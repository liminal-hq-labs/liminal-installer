// The real backend: checks, dry runs and script tests through the installer engine.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use li_engine::{
	CondContext, DirPayload, InstallRequest, Machine, ProjectScripts, SimMachine, State, cond,
};
use li_model::{Layout, Operation, Plan, Step, StepType, Target};
use li_render_tui::{Look, MapBindings, Screen, dump_with_controls};
use li_script::{Limits, Page, Script, ScriptContext, ScriptError};
use serde_json::{Value, json};

use crate::backend::{
	BackendError, HookInfo, Preview, PreviewStep, Problem, Screenshot, ScriptReport, Severity,
	StudioBackend,
};
use crate::checks::check_project;
use crate::mock::HOOKS;
use crate::project::Project;

/// Cells a step's screen is drawn at.
const COLS: u16 = 80;
const ROWS: u16 = 24;

/// Codes the Studio's own checks already report, in their own wording.
const COVERED_BY_STUDIO: &[&str] = &[
	"duplicate-target",
	"duplicate-component",
	"unknown-target",
	"unknown-theme",
	"missing-source",
	"missing-script",
	"bad-flow",
	"unknown-layout",
];

/// The backend that runs the real engine.
///
/// `check` is the Studio's own checks plus the engine's (conditions, components named in
/// conditions, hooks, built-in actions), not the CLI's `DefaultChecker`. The engine reads the
/// project from disk, which the Studio keeps current by writing every change through.
#[derive(Debug, Default, Clone, Copy)]
pub struct EngineBackend;

fn engine_project(project: &Project) -> Result<li_engine::Project, BackendError> {
	let root = project.root.canonicalize().unwrap_or(project.root.clone());
	let dir = match (root.file_name(), root.parent()) {
		(Some(n), Some(parent)) if n == "installer" => parent,
		_ => {
			return Err(BackendError(
				"the engine needs the project's files in a folder named `installer`".into(),
			));
		}
	};
	li_engine::Project::load(dir).map_err(|e| BackendError(e.to_string()))
}

/// The scope and step an engine location such as `flow install, step welcome` refers to.
fn scope_of(location: &str) -> (String, Option<String>) {
	if let Some(rest) = location.strip_prefix("flow ") {
		let mut parts = rest.split(", step ");
		let id = parts.next().unwrap_or(rest);
		return (format!("flow:{id}"), parts.next().map(str::to_owned));
	}
	if let Some((kind, id)) = location.split_once(' ')
		&& matches!(kind, "target" | "component" | "theme")
	{
		return (format!("{kind}:{id}"), None);
	}
	(location.to_owned(), None)
}

fn engine_problems(eng: &li_engine::Project) -> Vec<Problem> {
	li_engine::check(eng)
		.into_iter()
		.filter(|d| !COVERED_BY_STUDIO.contains(&d.code))
		.map(|d| {
			let (scope, step) = scope_of(&d.location);
			let p = match d.severity {
				li_engine::Severity::Error => Problem::error(scope, d.message),
				li_engine::Severity::Warning => Problem::warning(scope, d.message),
			};
			match step {
				Some(s) => p.at_step(s),
				None => p,
			}
		})
		.collect()
}

fn xml(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
}

/// Draws the layout as the `tui` renderer would, as a text dump set in an SVG.
fn screenshot(layout: &Layout, look: Look, state: &State, slug: &str) -> Screenshot {
	let screen = Screen::new(layout, look);
	let mut b = MapBindings::new();
	seed(&layout.root, &mut b, slug);
	for (k, v) in state {
		b.0.insert(k.clone(), v.clone());
	}
	let text = dump_with_controls(&screen, &b, COLS, ROWS);
	let lines: Vec<&str> = text.lines().collect();
	let (cell_w, cell_h) = (9usize, 18usize);
	let width = usize::from(COLS) * cell_w + 32;
	let height = lines.len() * cell_h + 32;
	let (bg, fg) = match look {
		Look::Retro => ("#0000aa", "#ffffff"),
		Look::Plain => ("#0c0c10", "#e0e0e0"),
	};
	let mut body = String::new();
	for (i, line) in lines.iter().enumerate() {
		body.push_str(&format!(
			"<text x=\"16\" y=\"{}\" xml:space=\"preserve\">{}</text>",
			24 + i * cell_h,
			xml(line)
		));
	}
	Screenshot {
		mime: "image/svg+xml".into(),
		encoding: "utf8".into(),
		data: format!(
			"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\"><rect width=\"{width}\" height=\"{height}\" fill=\"{bg}\"/><g font-family=\"monospace\" font-size=\"15\" fill=\"{fg}\">{body}</g></svg>"
		),
	}
}

/// Sample values so meters and paths show something.
fn seed(node: &li_model::Node, b: &mut MapBindings, slug: &str) {
	let node = node.for_renderer(li_render_tui::RENDERER_ID);
	if let Some(bind) = &node.bind {
		match node.kind.as_str() {
			"meter" | "progress" => b.0.insert(bind.clone(), json!(0.5)).map(drop),
			"path" => {
				b.0.insert(bind.clone(), json!(format!("~/.local/share/{slug}")))
					.map(drop)
			}
			_ => None,
		};
	}
	for c in &node.children {
		seed(c, b, slug);
	}
}

fn slug_of(eng: &li_engine::Project) -> String {
	eng.installer
		.product
		.id
		.rsplit('.')
		.next()
		.unwrap_or("app")
		.to_owned()
}

fn script_env(target: &Target, install: &Path, version: &str) -> BTreeMap<String, String> {
	let sim = SimMachine::new();
	BTreeMap::from([
		("os".to_owned(), sim.os().to_owned()),
		("arch".to_owned(), sim.arch().to_owned()),
		("target".to_owned(), target.id.clone()),
		("install".to_owned(), install.to_string_lossy().into_owned()),
		("version".to_owned(), version.to_owned()),
	])
}

/// The flow's steps as one target walks them: a step whose `when` is false is listed as
/// skipped and passed over, a branch takes its first condition that holds, and a page's
/// `detect` hook runs against the sample state.
#[allow(clippy::too_many_arguments)]
fn walk_install(
	eng: &li_engine::Project,
	target: &Target,
	selected: &[String],
	scripts: Option<&ProjectScripts>,
	sim: &SimMachine,
	install: &Path,
	problems: &mut Vec<Problem>,
) -> Vec<PreviewStep> {
	let Some(flow) = eng.flows.get("install") else {
		return Vec::new();
	};
	let look = if target.renderer == "tui-retro" {
		Look::Retro
	} else {
		Look::Plain
	};
	let slug = slug_of(eng);
	let mut state = State::new();
	state.insert("components".into(), json!(selected));
	state.insert(
		"install.path".into(),
		json!(install.to_string_lossy().into_owned()),
	);
	let (os, arch) = target.id.split_once('-').unwrap_or((&target.id, ""));
	let target_value = json!({
		"id": target.id,
		"renderer": target.renderer,
		"os": os,
		"arch": arch,
	});
	let system = li_engine::system_info(sim, install);
	let holds = |expr: &str, state: &State, step: &Step, problems: &mut Vec<Problem>| {
		let ctx = CondContext {
			target: target_value.clone(),
			components: state.get("components").cloned().unwrap_or(json!([])),
			state: Value::Object(state.clone()),
			system: system.clone(),
		};
		match cond::eval_bool(expr, &ctx) {
			Ok(v) => v,
			Err(e) => {
				problems.push(
					Problem::error("flow:install", format!("condition `{expr}`: {e}"))
						.at_step(&step.id),
				);
				false
			}
		}
	};

	let mut steps = Vec::new();
	let mut seen = BTreeSet::new();
	let mut cursor = flow.start_id().map(str::to_owned);
	while let Some(id) = cursor {
		if !seen.insert(id.clone()) {
			break;
		}
		let Some(step) = flow.step(&id) else { break };
		let skipped = match &step.when {
			Some(w) => !holds(w, &state, step, problems),
			None => false,
		};
		let mut screenshot_of = None;
		let mut cursor_next = step.next.clone();
		if !skipped {
			if let (Some(spec), Some(scripts)) = (&step.on_enter, scripts)
				&& let Some((path, "detect")) = spec.split_once('#')
				&& let Some(script) = scripts.get(path)
			{
				let mut page = Page {
					id: step.id.clone(),
					fields: state.clone(),
				};
				match script.detect(&mut page) {
					Ok(()) => state = page.fields,
					Err(e) => problems.push(
						Problem::error(format!("script:{path}"), format!("detect: {e}"))
							.at_step(&step.id),
					),
				}
			}
			if step.kind == StepType::Branch {
				for b in &step.branches {
					if holds(&b.when, &state, step, problems) {
						cursor_next = Some(b.next.clone());
						break;
					}
				}
			}
			if let Some(layout) = step
				.layout
				.as_deref()
				.and_then(|l| eng.layout_for(target, l))
			{
				screenshot_of = Some(screenshot(layout, look, &state, &slug));
			}
		}
		steps.push(PreviewStep {
			id: step.id.clone(),
			kind: format!("{:?}", step.kind).to_lowercase(),
			layout: step.layout.clone(),
			skipped,
			screenshot: screenshot_of,
		});
		if step.kind == StepType::Finish && !skipped {
			break;
		}
		cursor = cursor_next;
	}
	steps
}

impl StudioBackend for EngineBackend {
	fn check(&self, project: &Project) -> Result<Vec<Problem>, BackendError> {
		let mut out = check_project(project);
		out.extend(engine_problems(&engine_project(project)?));
		Ok(out)
	}

	fn run_preview(&self, project: &Project, target: &str) -> Result<Preview, BackendError> {
		let eng = engine_project(project)?;
		let t = eng
			.installer
			.targets
			.iter()
			.find(|t| t.full_id() == target)
			.ok_or_else(|| BackendError(format!("unknown target `{target}`")))?
			.clone();
		if !eng.flows.contains_key("install") {
			return Err(BackendError("the project has no install flow".into()));
		}
		let mut problems: Vec<Problem> = check_project(project)
			.into_iter()
			.filter(|p| {
				p.scope.starts_with("flow:install")
					|| p.scope == format!("target:{target}")
					|| p.scope.starts_with("layout:")
			})
			.collect();
		problems.extend(
			engine_problems(&eng)
				.into_iter()
				.filter(|p| p.scope.starts_with("flow:install") || p.scope.starts_with("target:")),
		);

		let sim = SimMachine::new();
		let install = PathBuf::from(format!("/opt/{}", slug_of(&eng)));
		let version = eng.literal_version().unwrap_or("0.0.0").to_owned();
		let selected =
			li_engine::select_components(&eng, &t.id, None).unwrap_or_else(|_| Vec::new());
		let scripts =
			match ProjectScripts::load(&eng, script_env(&t, &install, &version), sim.script_fs()) {
				Ok(s) => Some(s),
				Err(e) => {
					problems.push(Problem::error("script", e.to_string()));
					None
				}
			};
		let plan = match DirPayload::new(&eng.dir) {
			Ok(payload) => {
				let req = InstallRequest {
					target: t.id.clone(),
					install_dir: install.clone(),
					scope: eng.installer.defaults.scope,
					components: None,
					version: None,
					feed: None,
					channel: None,
				};
				li_engine::preview(&eng, &payload, &req, &sim).unwrap_or_else(|e| {
					problems.push(Problem::error(format!("target:{target}"), e.to_string()));
					Plan::default()
				})
			}
			Err(e) => {
				problems.push(Problem::error(format!("target:{target}"), e.to_string()));
				Plan::default()
			}
		};
		let steps = walk_install(
			&eng,
			&t,
			&selected,
			scripts.as_ref(),
			&sim,
			&install,
			&mut problems,
		);
		Ok(Preview {
			target: target.to_owned(),
			plan,
			problems,
			steps,
		})
	}

	fn test_script(
		&self,
		project: &Project,
		path: &str,
		source: &str,
	) -> Result<ScriptReport, BackendError> {
		let scope = format!("script:{path}");
		let mut diagnostics = Vec::new();
		let sim = SimMachine::new();
		let env = BTreeMap::from([
			("os".to_owned(), sim.os().to_owned()),
			("arch".to_owned(), sim.arch().to_owned()),
			("install".to_owned(), "/opt/app".to_owned()),
			("version".to_owned(), "0.0.0".to_owned()),
		]);
		let _ = project;
		let ctx = ScriptContext {
			env,
			fs: sim.script_fs(),
			net: None,
		};
		let no_hooks = |diagnostics: Vec<Problem>| ScriptReport {
			ok: false,
			hooks: HOOKS
				.iter()
				.map(|(n, w)| HookInfo {
					name: (*n).into(),
					when: (*w).into(),
					present: false,
				})
				.collect(),
			capabilities: Vec::new(),
			diagnostics,
			output: Vec::new(),
		};
		let script = match Script::load(path, source, ctx, Limits::default()) {
			Ok(s) => s,
			Err(e) => {
				return Ok(no_hooks(vec![script_problem(&scope, None, &e)]));
			}
		};
		let hooks: Vec<HookInfo> = HOOKS
			.iter()
			.map(|(name, when)| HookInfo {
				name: (*name).into(),
				when: (*when).into(),
				present: script.has_hook(name),
			})
			.collect();
		if !hooks.iter().any(|h| h.present) {
			diagnostics.push(Problem::error(&scope, "the script defines no hook"));
		}
		let mut output = Vec::new();
		let mut page = Page {
			id: "test".into(),
			fields: json!({"install.path": "/opt/app", "components": ["main"]})
				.as_object()
				.cloned()
				.unwrap_or_default(),
		};
		let note = |script: &Script, output: &mut Vec<String>| {
			for item in script.take_ui() {
				output.push(format!(
					"  ui {}: {}",
					match item.kind {
						li_script::UiKind::Note => "note",
						li_script::UiKind::Dialog => "dialog",
					},
					item.text
				));
			}
		};

		if script.has_hook("detect") {
			let before = page.fields.clone();
			match script.detect(&mut page) {
				Ok(()) => {
					let changed: Vec<&String> = page
						.fields
						.iter()
						.filter(|(k, v)| before.get(*k) != Some(*v))
						.map(|(k, _)| k)
						.collect();
					output.push(format!(
						"detect: ok, set {}",
						if changed.is_empty() {
							"nothing".to_owned()
						} else {
							changed
								.iter()
								.map(|k| k.as_str())
								.collect::<Vec<_>>()
								.join(", ")
						}
					));
				}
				Err(e) => diagnostics.push(script_problem(&scope, Some("detect"), &e)),
			}
			note(&script, &mut output);
		}
		if script.has_hook("validate") {
			match script.validate(&page) {
				Ok(found) if found.is_empty() => output.push("validate: ok, no problems".into()),
				Ok(found) => {
					output.push(format!(
						"validate: {} problem(s) on the sample page",
						found.len()
					));
					output.extend(found.iter().map(|f| format!("  {f}")));
				}
				Err(e) => diagnostics.push(script_problem(&scope, Some("validate"), &e)),
			}
			note(&script, &mut output);
		}
		let sample = Plan {
			ops: vec![
				Operation::Mkdir {
					path: "{install}".into(),
				},
				Operation::ReceiptWrite {
					path: "{install}/.liminal/receipt.json".into(),
				},
			],
		};
		for (name, run) in [
			(
				"on_commit",
				Script::on_commit as fn(&Script, &Plan) -> Result<Vec<Operation>, ScriptError>,
			),
			("on_uninstall", Script::on_uninstall),
		] {
			if !script.has_hook(name) {
				continue;
			}
			match run(&script, &sample) {
				Ok(ops) => {
					output.push(format!("{name}: ok, adds {} operation(s)", ops.len()));
					output.extend(ops.iter().map(|op| format!("  plan.add {}", op_line(op))));
				}
				Err(e) => diagnostics.push(script_problem(&scope, Some(name), &e)),
			}
			note(&script, &mut output);
		}

		let capabilities = modules(&script);
		if capabilities.iter().any(|c| c == "li.net") {
			diagnostics.push(Problem::warning(
				&scope,
				"`net` is granted, but a preview never reaches the network",
			));
		}
		let ok = diagnostics.iter().all(|d| d.severity != Severity::Error);
		Ok(ScriptReport {
			ok,
			hooks,
			capabilities,
			diagnostics,
			output,
		})
	}
}

/// The `li.*` modules the script's granted capabilities open.
fn modules(script: &Script) -> Vec<String> {
	let mut out: Vec<String> = script
		.capabilities()
		.iter()
		.map(|c| format!("li.{}", c.split('.').next().unwrap_or(c)))
		.collect();
	out.sort();
	out.dedup();
	out
}

fn script_problem(scope: &str, hook: Option<&str>, e: &ScriptError) -> Problem {
	let prefix = hook.map(|h| format!("{h}: ")).unwrap_or_default();
	match e {
		ScriptError::Denied(cap) => Problem::error(
			scope,
			format!(
				"{prefix}capability `{cap}` is denied; name it in a `-- capabilities:` header comment"
			),
		),
		other => Problem::error(scope, format!("{prefix}{other}")),
	}
}

fn op_line(op: &Operation) -> String {
	serde_json::to_string(op).unwrap_or_else(|_| format!("{op:?}"))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::project::{Doc, DocKey};

	fn open(name: &str) -> Project {
		Project::open(
			&Path::new(env!("CARGO_MANIFEST_DIR"))
				.join("../../examples")
				.join(name),
		)
		.unwrap()
	}

	fn step<'a>(p: &'a Preview, id: &str) -> &'a PreviewStep {
		p.steps.iter().find(|s| s.id == id).unwrap()
	}

	#[test]
	fn fizz_skips_the_sound_card_unless_the_renderer_is_retro() {
		let p = open("lt-fizz");
		let retro = EngineBackend.run_preview(&p, "dos-x86:tui-retro").unwrap();
		let modern = EngineBackend.run_preview(&p, "linux-x86_64:tui").unwrap();
		assert!(!step(&retro, "sound-card").skipped);
		assert!(step(&retro, "sound-card").screenshot.is_some());
		assert!(step(&modern, "sound-card").skipped);
		assert!(step(&modern, "sound-card").screenshot.is_none());
		assert!(!retro.plan.ops.is_empty());
		assert!(matches!(
			retro.plan.ops.last(),
			Some(Operation::ReceiptWrite { .. })
		));
		assert_eq!(retro.steps.last().unwrap().kind, "finish");
		let shot = step(&retro, "welcome").screenshot.as_ref().unwrap();
		assert_eq!(shot.mime, "image/svg+xml");
		assert!(shot.data.contains("<text"));
		let errors: Vec<_> = retro
			.problems
			.iter()
			.filter(|p| p.severity == Severity::Error)
			.collect();
		assert!(errors.is_empty(), "{errors:?}");
	}

	#[test]
	fn the_detect_hook_runs_for_the_sound_card_page() {
		let p = open("lt-fizz");
		let pv = EngineBackend.run_preview(&p, "dos-x86:tui-retro").unwrap();
		assert!(pv.problems.iter().all(|x| !x.message.starts_with("detect")));
		let broken = {
			let dir = tempfile::tempdir().unwrap();
			copy_dir(&examples().join("lt-fizz"), dir.path());
			let script = dir.path().join("installer/scripts/sound-setup.lua");
			let src = std::fs::read_to_string(&script).unwrap();
			std::fs::write(&script, src.replace("li.env.get", "li.net.get")).unwrap();
			let p = Project::open(&dir.path().join("installer")).unwrap();
			EngineBackend.run_preview(&p, "dos-x86:tui-retro").unwrap()
		};
		assert!(
			broken
				.problems
				.iter()
				.any(|x| x.message.starts_with("detect:") && x.message.contains("not granted"))
		);
	}

	#[test]
	fn planboard_previews_a_plan_for_each_target() {
		let p = open("planboard");
		for t in ["linux-x86_64:canvas", "linux-x86_64:tui"] {
			let pv = EngineBackend.run_preview(&p, t).unwrap();
			assert!(pv.plan.ops.len() > 3, "{t}");
			assert!(pv.steps.iter().all(|s| !s.skipped), "{t}");
			assert!(pv.steps.iter().any(|s| s.screenshot.is_some()));
		}
		assert!(EngineBackend.run_preview(&p, "nope:tui").is_err());
	}

	#[test]
	fn a_branch_takes_the_first_condition_that_holds() {
		let dir = tempfile::tempdir().unwrap();
		copy_dir(
			&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/planboard"),
			dir.path(),
		);
		let flow_path = dir.path().join("installer/flows/install.flow.json");
		let mut flow: Value =
			serde_json::from_str(&std::fs::read_to_string(&flow_path).unwrap()).unwrap();
		let steps = flow["steps"].as_array_mut().unwrap();
		let i = steps.iter().position(|s| s["id"] == "folder").unwrap();
		steps[i]["next"] = json!("route");
		steps.push(json!({
			"id": "route", "type": "branch",
			"branches": [{"when": "target.renderer == 'tui'", "next": "components"}],
			"next": "ready"
		}));
		std::fs::write(&flow_path, flow.to_string()).unwrap();
		let p = Project::open(&dir.path().join("installer")).unwrap();
		let tui = EngineBackend.run_preview(&p, "linux-x86_64:tui").unwrap();
		let canvas = EngineBackend
			.run_preview(&p, "linux-x86_64:canvas")
			.unwrap();
		let ids = |pv: &Preview| pv.steps.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
		assert!(ids(&tui).contains(&"components".to_owned()));
		assert!(!ids(&canvas).contains(&"components".to_owned()));
	}

	fn examples() -> PathBuf {
		Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
	}

	fn copy_dir(from: &Path, to: &Path) {
		for e in std::fs::read_dir(from).unwrap() {
			let e = e.unwrap();
			let dest = to.join(e.file_name());
			if e.file_type().unwrap().is_dir() {
				std::fs::create_dir_all(&dest).unwrap();
				copy_dir(&e.path(), &dest);
			} else {
				std::fs::copy(e.path(), dest).unwrap();
			}
		}
	}

	#[test]
	fn check_reports_engine_findings_the_studio_does_not() {
		let dir = tempfile::tempdir().unwrap();
		copy_dir(&examples().join("lt-fizz"), dir.path());
		let mut p = Project::open(&dir.path().join("installer")).unwrap();
		assert!(
			EngineBackend
				.check(&p)
				.unwrap()
				.iter()
				.all(|x| x.severity != Severity::Error)
		);
		let mut flow = p.flows["install"].clone();
		flow.steps[0].when = Some("target.renderer in [".into());
		p.put(&DocKey::Flow("install".into()), Some(Doc::Flow(flow)))
			.unwrap();
		let found = EngineBackend.check(&p).unwrap();
		assert!(
			found
				.iter()
				.any(|x| x.scope == "flow:install" && x.step.as_deref() == Some("welcome")),
			"{found:?}"
		);
	}

	#[test]
	fn the_example_script_passes_its_test() {
		let p = open("lt-fizz");
		let src = p.scripts["scripts/sound-setup.lua"].clone();
		let r = EngineBackend
			.test_script(&p, "scripts/sound-setup.lua", &src)
			.unwrap();
		assert!(r.ok, "{:?}", r.diagnostics);
		assert_eq!(
			r.hooks.iter().filter(|h| h.present).count(),
			2,
			"{:?}",
			r.hooks
		);
		assert!(r.capabilities.contains(&"li.env".to_owned()));
		assert!(r.output.iter().any(|l| l.starts_with("detect: ok")));
	}

	#[test]
	fn a_denied_capability_is_reported() {
		let p = open("lt-fizz");
		let src = "-- capabilities: ui\nfunction detect(page)\n\tpage.fields.x = li.env.get('HOME')\nend\n";
		let r = EngineBackend.test_script(&p, "scripts/x.lua", src).unwrap();
		assert!(!r.ok);
		assert!(
			r.diagnostics
				.iter()
				.any(|d| d.message.contains("capability `env` is denied")),
			"{:?}",
			r.diagnostics
		);
	}

	#[test]
	fn plan_writes_and_load_errors_are_reported() {
		let p = open("lt-fizz");
		let src = "-- capabilities: plan, fs.write\nfunction on_commit(plan)\n\tplan:add({op = 'mkdir', path = '{install}/saves'})\nend\n";
		let r = EngineBackend.test_script(&p, "scripts/x.lua", src).unwrap();
		assert!(r.ok, "{:?}", r.diagnostics);
		assert!(
			r.output
				.iter()
				.any(|l| l.contains("plan.add") && l.contains("saves"))
		);
		let denied = "-- capabilities: plan\nfunction on_commit(plan)\n\tplan:add({op = 'mkdir', path = '{install}/saves'})\nend\n";
		assert!(
			!EngineBackend
				.test_script(&p, "scripts/x.lua", denied)
				.unwrap()
				.ok
		);
		let unknown = EngineBackend
			.test_script(&p, "scripts/x.lua", "-- capabilities: teleport\n")
			.unwrap();
		assert!(!unknown.ok);
		let none = EngineBackend
			.test_script(&p, "scripts/x.lua", "-- nothing")
			.unwrap();
		assert!(!none.ok);
	}
}
