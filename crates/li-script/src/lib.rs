// Sandboxed Lua 5.4 scripts that hook into an installer flow.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lua 5.4 scripts in a sandbox.
//!
//! A script sees only the safe parts of the standard library (`string`, `table`, `math`, `utf8`
//! and the pure base functions). There is no `io`, `os`, `package`, `debug`, `coroutine`,
//! `load`, `loadfile`, `dofile`, `require`, `pcall` or `xpcall`; the last three are absent so
//! that a script cannot catch the error that stops it.
//!
//! # Capabilities
//!
//! A script names what it needs, either in a header comment at the top of the file
//!
//! ```lua
//! -- capabilities: env, plan, fs.write
//! ```
//!
//! or in a `capabilities` list on the table the script returns. Everything else is refused.
//!
//! | Capability | Unlocks |
//! |---|---|
//! | `env` | `li.env.get(name)`, `li.env.os()`, `li.env.arch()` |
//! | `fs.read` | `li.fs.read(path)`, `li.fs.exists(path)`, `li.fs.list(path)` |
//! | `fs.write` | file-writing operations through `li.plan.add` |
//! | `ui` | `li.ui.note(text)`, `li.ui.dialog{title=, text=}` |
//! | `plan` | `li.plan.add(op)` and `plan:add(op)` |
//! | `net` | `li.net.get(url)` |
//!
//! File-writing operations are `mkdir`, `copy`, `write_file`, `write_ini` and `write_json`; they
//! need both `plan` and `fs.write`. Scripts never write files directly.
//!
//! # Hooks
//!
//! `detect(page)`, `validate(page)`, `on_commit(plan)` and `on_uninstall(plan)`, defined either as
//! globals or as fields of the returned table. See [`Script`].
//!
//! # Limits
//!
//! Each hook call may run [`Limits::instructions`] Lua instructions and the state may hold
//! [`Limits::memory`] bytes. A script that exceeds either stops with [`ScriptError::Limit`].
//! Long-running work inside a single library call, such as a pathological pattern match, is not
//! interrupted by the instruction count.

pub mod check;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use li_model::{Operation, Plan};
use mlua::{HookTriggers, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value, VmState};
use thiserror::Error;

/// Every capability a script may name.
pub const CAPABILITIES: &[&str] = &["env", "fs.read", "fs.write", "net", "ui", "plan"];

#[derive(Debug, Error)]
pub enum ScriptError {
	#[error("script error: {0}")]
	Lua(String),
	#[error("capability `{0}` not granted")]
	Denied(String),
	#[error("script stopped: {0}")]
	Limit(String),
	#[error("unknown capability `{0}`")]
	UnknownCapability(String),
	#[error("bad plan operation: {0}")]
	BadOperation(String),
	#[error("script has no `{0}` hook")]
	MissingHook(String),
}

/// Read-only file access for `li.fs`. The host decides what paths mean.
pub trait ScriptFs: Send + Sync {
	fn read(&self, path: &str) -> Result<Vec<u8>, String>;
	fn exists(&self, path: &str) -> bool;
	fn list(&self, path: &str) -> Result<Vec<String>, String>;
}

/// A file system that has nothing in it.
pub struct NoFs;

impl ScriptFs for NoFs {
	fn read(&self, path: &str) -> Result<Vec<u8>, String> {
		Err(format!("no such file `{path}`"))
	}
	fn exists(&self, _: &str) -> bool {
		false
	}
	fn list(&self, path: &str) -> Result<Vec<String>, String> {
		Err(format!("no such directory `{path}`"))
	}
}

/// The host's side of `li.net`.
pub trait ScriptNet: Send + Sync {
	fn get(&self, url: &str) -> Result<String, String>;
}

/// Execution limits for one script.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
	/// Lua instructions per hook call (and per chunk load).
	pub instructions: u64,
	/// Bytes the Lua state may allocate.
	pub memory: usize,
}

impl Default for Limits {
	fn default() -> Self {
		Limits {
			instructions: 5_000_000,
			memory: 16 * 1024 * 1024,
		}
	}
}

/// What the host lends a script.
#[derive(Clone)]
pub struct ScriptContext {
	/// Values for `li.env.get`; `os` and `arch` are also exposed as functions.
	pub env: BTreeMap<String, String>,
	pub fs: Arc<dyn ScriptFs>,
	pub net: Option<Arc<dyn ScriptNet>>,
}

impl Default for ScriptContext {
	fn default() -> Self {
		ScriptContext {
			env: BTreeMap::new(),
			fs: Arc::new(NoFs),
			net: None,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKind {
	Note,
	Dialog,
}

/// A note or dialog a script asked for. The renderer decides how to show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiItem {
	pub kind: UiKind,
	pub title: Option<String>,
	pub text: String,
}

/// The fields of a page a hook may read and, for `detect`, change.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
	pub id: String,
	pub fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Default)]
struct Shared {
	caps: Mutex<BTreeSet<String>>,
	ui: Mutex<Vec<UiItem>>,
	ops: Mutex<Vec<serde_json::Value>>,
	used: AtomicU64,
}

impl Shared {
	fn require(&self, cap: &str) -> mlua::Result<()> {
		if self.caps.lock().expect("caps lock").contains(cap) {
			Ok(())
		} else {
			Err(mlua::Error::runtime(format!(
				"capability `{cap}` not granted"
			)))
		}
	}
}

/// A loaded script.
pub struct Script {
	lua: Lua,
	env: Table,
	returned: Option<Table>,
	shared: Arc<Shared>,
	limits: Limits,
}

fn header_capabilities(source: &str) -> Vec<String> {
	let mut caps = Vec::new();
	for line in source.lines() {
		let line = line.trim();
		if line.is_empty() {
			continue;
		}
		let Some(rest) = line.strip_prefix("--") else {
			break;
		};
		let rest = rest.trim_start_matches('-').trim();
		if let Some(list) = rest.strip_prefix("capabilities:") {
			caps.extend(
				list.split([',', ' '])
					.map(str::trim)
					.filter(|s| !s.is_empty())
					.map(str::to_owned),
			);
		}
	}
	caps
}

fn classify(err: mlua::Error) -> ScriptError {
	let text = err.to_string();
	if let Some(at) = text.find("capability `") {
		let rest = &text[at + "capability `".len()..];
		if let Some(end) = rest.find('`') {
			return ScriptError::Denied(rest[..end].to_owned());
		}
	}
	if text.contains("li-script: instruction limit")
		|| matches!(err, mlua::Error::MemoryError(_))
		|| text.contains("not enough memory")
	{
		return ScriptError::Limit(if text.contains("memory") {
			"memory limit exceeded".into()
		} else {
			"instruction limit exceeded".into()
		});
	}
	ScriptError::Lua(text)
}

const SAFE_BASE: &[&str] = &[
	"assert",
	"error",
	"ipairs",
	"next",
	"pairs",
	"rawequal",
	"rawget",
	"rawlen",
	"rawset",
	"select",
	"setmetatable",
	"getmetatable",
	"tonumber",
	"tostring",
	"type",
	"_VERSION",
];

impl Script {
	/// Loads and runs the script's top level under the sandbox.
	pub fn load(
		name: &str,
		source: &str,
		ctx: ScriptContext,
		limits: Limits,
	) -> Result<Script, ScriptError> {
		let mut caps = BTreeSet::new();
		for cap in header_capabilities(source) {
			if !CAPABILITIES.contains(&cap.as_str()) {
				return Err(ScriptError::UnknownCapability(cap));
			}
			caps.insert(cap);
		}
		let shared = Arc::new(Shared {
			caps: Mutex::new(caps),
			..Shared::default()
		});
		let lua = Lua::new_with(
			StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8,
			LuaOptions::default(),
		)
		.map_err(classify)?;
		lua.set_memory_limit(limits.memory).map_err(classify)?;
		let counter = shared.clone();
		let step = 1000u32;
		let max = limits.instructions;
		lua.set_global_hook(
			HookTriggers::new().every_nth_instruction(step),
			move |_, _| {
				let used = counter.used.fetch_add(u64::from(step), Ordering::Relaxed);
				if used >= max {
					Err(mlua::Error::runtime(
						"li-script: instruction limit exceeded",
					))
				} else {
					Ok(VmState::Continue)
				}
			},
		)
		.map_err(classify)?;

		let globals = lua.globals();
		let env = lua.create_table().map_err(classify)?;
		for key in SAFE_BASE {
			let v: Value = globals.get(*key).map_err(classify)?;
			if !v.is_nil() {
				env.set(*key, v).map_err(classify)?;
			}
		}
		let string: Table = globals.get("string").map_err(classify)?;
		string.set("dump", Value::Nil).map_err(classify)?;
		for lib in ["string", "table", "math", "utf8"] {
			let v: Value = globals.get(lib).map_err(classify)?;
			env.set(lib, v).map_err(classify)?;
		}
		env.set("_G", env.clone()).map_err(classify)?;
		env.set("li", build_li(&lua, &shared, &ctx).map_err(classify)?)
			.map_err(classify)?;

		shared.used.store(0, Ordering::Relaxed);
		let value: Value = lua
			.load(source)
			.set_name(name)
			.set_environment(env.clone())
			.eval()
			.map_err(classify)?;
		let returned = match value {
			Value::Table(t) => {
				if let Ok(list) = t.get::<Table>("capabilities") {
					for item in list.sequence_values::<String>() {
						let cap = item.map_err(classify)?;
						if !CAPABILITIES.contains(&cap.as_str()) {
							return Err(ScriptError::UnknownCapability(cap));
						}
						shared.caps.lock().expect("caps lock").insert(cap);
					}
				}
				Some(t)
			}
			_ => None,
		};
		Ok(Script {
			lua,
			env,
			returned,
			shared,
			limits,
		})
	}

	/// The capabilities this script holds.
	pub fn capabilities(&self) -> BTreeSet<String> {
		self.shared.caps.lock().expect("caps lock").clone()
	}

	pub fn limits(&self) -> Limits {
		self.limits
	}

	fn hook(&self, name: &str) -> Option<mlua::Function> {
		if let Some(t) = &self.returned
			&& let Ok(f) = t.get::<mlua::Function>(name)
		{
			return Some(f);
		}
		self.env.get::<mlua::Function>(name).ok()
	}

	pub fn has_hook(&self, name: &str) -> bool {
		self.hook(name).is_some()
	}

	fn call(&self, name: &str, arg: Value) -> Result<Value, ScriptError> {
		let f = self
			.hook(name)
			.ok_or_else(|| ScriptError::MissingHook(name.to_owned()))?;
		self.shared.used.store(0, Ordering::Relaxed);
		f.call::<Value>(arg).map_err(classify)
	}

	fn page_table(&self, page: &Page) -> Result<Table, ScriptError> {
		let t = self.lua.create_table().map_err(classify)?;
		t.set("id", page.id.as_str()).map_err(classify)?;
		let fields = self
			.lua
			.to_value(&serde_json::Value::Object(page.fields.clone()))
			.map_err(classify)?;
		let fields = match fields {
			Value::Table(f) => f,
			_ => self.lua.create_table().map_err(classify)?,
		};
		t.set("fields", fields).map_err(classify)?;
		Ok(t)
	}

	/// Runs `detect(page)`, which may change the page's fields. A script without the hook leaves
	/// the page alone.
	pub fn detect(&self, page: &mut Page) -> Result<(), ScriptError> {
		if !self.has_hook("detect") {
			return Ok(());
		}
		let t = self.page_table(page)?;
		self.call("detect", Value::Table(t.clone()))?;
		let fields: Value = t.get("fields").map_err(classify)?;
		let json: serde_json::Value = self.lua.from_value(fields).map_err(classify)?;
		if let serde_json::Value::Object(map) = json {
			page.fields = map;
		} else {
			page.fields.clear();
		}
		Ok(())
	}

	/// Runs `validate(page)` and returns the problems it lists (a string or a list of strings).
	/// Changes the hook makes to the page are discarded.
	pub fn validate(&self, page: &Page) -> Result<Vec<String>, ScriptError> {
		if !self.has_hook("validate") {
			return Ok(Vec::new());
		}
		let t = self.page_table(page)?;
		let result = self.call("validate", Value::Table(t))?;
		match result {
			Value::Nil => Ok(Vec::new()),
			Value::String(s) => Ok(vec![s.to_string_lossy().to_string()]),
			Value::Table(list) => list
				.sequence_values::<String>()
				.collect::<Result<_, _>>()
				.map_err(classify),
			other => Err(ScriptError::Lua(format!(
				"validate returned a {}, expected a list of strings",
				other.type_name()
			))),
		}
	}

	/// Runs `on_commit(plan)` and returns the operations the script added.
	pub fn on_commit(&self, plan: &Plan) -> Result<Vec<Operation>, ScriptError> {
		self.plan_hook("on_commit", plan)
	}

	/// Runs `on_uninstall(plan)` and returns the operations the script added.
	pub fn on_uninstall(&self, plan: &Plan) -> Result<Vec<Operation>, ScriptError> {
		self.plan_hook("on_uninstall", plan)
	}

	fn plan_hook(&self, name: &str, plan: &Plan) -> Result<Vec<Operation>, ScriptError> {
		if !self.has_hook(name) {
			return Ok(Vec::new());
		}
		self.shared.ops.lock().expect("ops lock").clear();
		let t = self.lua.create_table().map_err(classify)?;
		let ops = self.lua.to_value(&plan.ops).map_err(classify)?;
		t.set("ops", ops).map_err(classify)?;
		let shared = self.shared.clone();
		let add = self
			.lua
			.create_function(move |lua, (_self, op): (Value, Value)| add_op(lua, &shared, op))
			.map_err(classify)?;
		t.set("add", add).map_err(classify)?;
		self.call(name, Value::Table(t))?;
		let raw = std::mem::take(&mut *self.shared.ops.lock().expect("ops lock"));
		raw.into_iter()
			.map(|v| {
				serde_json::from_value::<Operation>(v)
					.map_err(|e| ScriptError::BadOperation(e.to_string()))
			})
			.collect()
	}

	/// Notes and dialogs collected since the last call.
	pub fn take_ui(&self) -> Vec<UiItem> {
		std::mem::take(&mut *self.shared.ui.lock().expect("ui lock"))
	}
}

const FILE_OPS: &[&str] = &["mkdir", "copy", "write_file", "write_ini", "write_json"];

fn add_op(lua: &Lua, shared: &Shared, op: Value) -> mlua::Result<()> {
	shared.require("plan")?;
	let json: serde_json::Value = lua.from_value(op)?;
	let kind = json
		.get("op")
		.and_then(|v| v.as_str())
		.ok_or_else(|| mlua::Error::runtime("plan operation needs an `op` field"))?;
	if FILE_OPS.contains(&kind) {
		shared.require("fs.write")?;
	}
	shared.ops.lock().expect("ops lock").push(json);
	Ok(())
}

fn build_li(lua: &Lua, shared: &Arc<Shared>, ctx: &ScriptContext) -> mlua::Result<Table> {
	let li = lua.create_table()?;

	let env = lua.create_table()?;
	{
		let s = shared.clone();
		let vars = ctx.env.clone();
		env.set(
			"get",
			lua.create_function(move |_, name: String| {
				s.require("env")?;
				Ok(vars.get(&name).cloned())
			})?,
		)?;
		for (fname, key) in [("os", "os"), ("arch", "arch")] {
			let s = shared.clone();
			let vars = ctx.env.clone();
			env.set(
				fname,
				lua.create_function(move |_, ()| {
					s.require("env")?;
					Ok(vars.get(key).cloned())
				})?,
			)?;
		}
	}
	li.set("env", env)?;

	let fs = lua.create_table()?;
	{
		let (s, host) = (shared.clone(), ctx.fs.clone());
		fs.set(
			"read",
			lua.create_function(move |lua, path: String| {
				s.require("fs.read")?;
				let bytes = host.read(&path).map_err(mlua::Error::runtime)?;
				lua.create_string(&bytes)
			})?,
		)?;
		let (s, host) = (shared.clone(), ctx.fs.clone());
		fs.set(
			"exists",
			lua.create_function(move |_, path: String| {
				s.require("fs.read")?;
				Ok(host.exists(&path))
			})?,
		)?;
		let (s, host) = (shared.clone(), ctx.fs.clone());
		fs.set(
			"list",
			lua.create_function(move |_, path: String| {
				s.require("fs.read")?;
				host.list(&path).map_err(mlua::Error::runtime)
			})?,
		)?;
	}
	li.set("fs", fs)?;

	let ui = lua.create_table()?;
	{
		let s = shared.clone();
		ui.set(
			"note",
			lua.create_function(move |_, text: String| {
				s.require("ui")?;
				s.ui.lock().expect("ui lock").push(UiItem {
					kind: UiKind::Note,
					title: None,
					text,
				});
				Ok(())
			})?,
		)?;
		let s = shared.clone();
		ui.set(
			"dialog",
			lua.create_function(move |_, arg: Value| {
				s.require("ui")?;
				let (title, text) = match arg {
					Value::String(t) => (None, t.to_string_lossy().to_string()),
					Value::Table(t) => (
						t.get("title")?,
						t.get::<Option<String>>("text")?.unwrap_or_default(),
					),
					_ => return Err(mlua::Error::runtime("dialog expects a string or a table")),
				};
				s.ui.lock().expect("ui lock").push(UiItem {
					kind: UiKind::Dialog,
					title,
					text,
				});
				Ok(())
			})?,
		)?;
	}
	li.set("ui", ui)?;

	let plan = lua.create_table()?;
	{
		let s = shared.clone();
		plan.set(
			"add",
			lua.create_function(move |lua, op: Value| add_op(lua, &s, op))?,
		)?;
	}
	li.set("plan", plan)?;

	let net = lua.create_table()?;
	{
		let (s, host) = (shared.clone(), ctx.net.clone());
		net.set(
			"get",
			lua.create_function(move |_, url: String| {
				s.require("net")?;
				match &host {
					Some(h) => h.get(&url).map_err(mlua::Error::runtime),
					None => Err(mlua::Error::runtime("no network is available")),
				}
			})?,
		)?;
	}
	li.set("net", net)?;
	Ok(li)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn load(src: &str) -> Result<Script, ScriptError> {
		Script::load("test.lua", src, ScriptContext::default(), Limits::default())
	}

	fn must_fail(src: &str, hook: &str) -> ScriptError {
		let s = load(src).unwrap();
		match hook {
			"detect" => s.detect(&mut Page::default()).unwrap_err(),
			_ => s.on_commit(&Plan::default()).unwrap_err(),
		}
	}

	#[test]
	fn dangerous_globals_are_absent() {
		let s = load(
			"function detect(page)
				page.fields.has = tostring(io) .. tostring(os) .. tostring(package) .. tostring(debug)
					.. tostring(loadfile) .. tostring(dofile) .. tostring(load) .. tostring(require)
					.. tostring(pcall) .. tostring(coroutine) .. tostring(string.dump)
			end",
		)
		.unwrap();
		let mut page = Page::default();
		s.detect(&mut page).unwrap();
		assert_eq!(page.fields["has"], "nilnilnilnilnilnilnilnilnilnilnil");
	}

	#[test]
	fn unknown_capability_is_refused() {
		assert!(matches!(
			load("-- capabilities: teleport\n"),
			Err(ScriptError::UnknownCapability(_))
		));
	}

	#[test]
	fn denied_without_capability() {
		for body in [
			"li.net.get('http://x')",
			"li.env.get('a')",
			"li.fs.exists('a')",
			"li.ui.note('a')",
			"li.plan.add({op='mkdir', path='a'})",
		] {
			let err = must_fail(&format!("function on_commit(plan) {body} end"), "on_commit");
			assert!(matches!(err, ScriptError::Denied(_)), "{body}: {err}");
		}
	}

	#[test]
	fn file_ops_need_fs_write() {
		let src = "-- capabilities: plan\nfunction on_commit(plan) plan:add({op='mkdir', path='{install}/a'}) end";
		let err = must_fail(src, "on_commit");
		assert!(matches!(err, ScriptError::Denied(c) if c == "fs.write"));
		let src = "-- capabilities: plan, fs.write\nfunction on_commit(plan) plan:add({op='mkdir', path='{install}/a'}) li.plan.add({op='shortcut', name='n', target='t', place='desktop'}) end";
		let ops = load(src).unwrap().on_commit(&Plan::default()).unwrap();
		assert_eq!(ops.len(), 2);
		assert!(matches!(ops[0], Operation::Mkdir { .. }));
	}

	#[test]
	fn capabilities_from_returned_table() {
		let src = "return { capabilities = {'ui'}, on_commit = function(plan) li.ui.note('hi') li.ui.dialog{title='T', text='body'} end }";
		let s = load(src).unwrap();
		assert!(s.capabilities().contains("ui"));
		s.on_commit(&Plan::default()).unwrap();
		let ui = s.take_ui();
		assert_eq!(ui.len(), 2);
		assert_eq!(ui[1].kind, UiKind::Dialog);
		assert_eq!(ui[1].title.as_deref(), Some("T"));
		assert!(s.take_ui().is_empty());
	}

	#[test]
	fn net_is_refused_without_net_even_when_a_host_exists() {
		struct Net;
		impl ScriptNet for Net {
			fn get(&self, _: &str) -> Result<String, String> {
				Ok("body".into())
			}
		}
		let ctx = ScriptContext {
			net: Some(Arc::new(Net)),
			..ScriptContext::default()
		};
		let denied = Script::load(
			"t",
			"function detect(p) p.fields.x = li.net.get('u') end",
			ctx.clone(),
			Limits::default(),
		)
		.unwrap();
		assert!(matches!(
			denied.detect(&mut Page::default()),
			Err(ScriptError::Denied(_))
		));
		let ok = Script::load(
			"t",
			"-- capabilities: net\nfunction detect(p) p.fields.x = li.net.get('u') end",
			ctx,
			Limits::default(),
		)
		.unwrap();
		let mut page = Page::default();
		ok.detect(&mut page).unwrap();
		assert_eq!(page.fields["x"], "body");
	}

	#[test]
	fn detect_and_validate_round_trip_fields() {
		let src = "-- capabilities: env, fs.read
			function detect(page)
				page.fields.os = li.env.get('os')
				page.fields.count = (page.fields.count or 0) + 1
			end
			function validate(page)
				if page.fields.path == '' then return {'path is empty'} end
			end";
		let mut env = BTreeMap::new();
		env.insert("os".to_owned(), "linux".to_owned());
		let s = Script::load(
			"t",
			src,
			ScriptContext {
				env,
				..ScriptContext::default()
			},
			Limits::default(),
		)
		.unwrap();
		let mut page = Page::default();
		page.fields.insert("path".into(), "".into());
		page.fields.insert("count".into(), 1.into());
		s.detect(&mut page).unwrap();
		assert_eq!(page.fields["os"], "linux");
		assert_eq!(page.fields["count"], 2);
		assert_eq!(s.validate(&page).unwrap(), vec!["path is empty"]);
		page.fields.insert("path".into(), "/x".into());
		assert!(s.validate(&page).unwrap().is_empty());
	}

	#[test]
	fn runaway_scripts_are_stopped() {
		let err = must_fail("function detect(p) while true do end end", "detect");
		assert!(matches!(err, ScriptError::Limit(_)), "{err}");
		let err = load("while true do end").err().expect("load must fail");
		assert!(matches!(err, ScriptError::Limit(_)), "{err}");
	}

	#[test]
	fn memory_hogs_are_stopped() {
		let src = "function detect(p) local t = {} for i = 1, 1e9 do t[i] = string.rep('x', 1000) .. i end end";
		let err = must_fail(src, "detect");
		assert!(matches!(err, ScriptError::Limit(_)), "{err}");
	}

	#[test]
	fn limits_reset_for_each_call() {
		let src = "function detect(p) local n = 0 for i = 1, 100000 do n = n + i end end";
		let s = Script::load(
			"t",
			src,
			ScriptContext::default(),
			Limits {
				instructions: 2_000_000,
				memory: 1 << 24,
			},
		)
		.unwrap();
		for _ in 0..40 {
			s.detect(&mut Page::default()).unwrap();
		}
	}
}
