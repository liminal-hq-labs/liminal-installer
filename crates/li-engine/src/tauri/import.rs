// `liminst import tauri`: read a Tauri config, fill the project, and say what drifted since.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Component as PathPart, Path, PathBuf};

use li_model::tauri::InstallMode;
use li_model::{InstallerFile, Target, TauriSnapshot, project::VersionSource};
use serde::Serialize;
use serde_json::Value;

use crate::error::{EngineError, Result};
use crate::project::Project;
use crate::tauri::conf::{read_snapshot, source_hashes};
use crate::tauri::edit::{self, get_key, quote, replace_block, set_key};
use crate::tauri::plan::is_windows;

const BEGIN: &str = "# >>> liminst import tauri (generated; the next import replaces this block)";
const END: &str = "# <<< liminst import tauri";

/// One value that differs between two snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldChange {
	/// A path such as `windows.webview2.mode` or `deep_link_schemes[0]`.
	pub field: String,
	pub from: Option<String>,
	pub to: Option<String>,
}

/// What an import did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportReport {
	pub snapshot_path: PathBuf,
	/// Whether there was an earlier snapshot to compare with.
	pub first_import: bool,
	/// What changed in the Tauri project since the earlier snapshot.
	pub changes: Vec<FieldChange>,
	/// `[product]` fields the import set, as `id`, `name`, `publisher`, `version`.
	pub filled: Vec<String>,
	/// `[product]` fields the project keeps, because `[tauri] override` names them.
	pub kept: Vec<String>,
	pub components: Vec<String>,
	pub warnings: Vec<String>,
}

fn flatten(v: &Value, path: &str, out: &mut Vec<(String, String)>) {
	match v {
		Value::Object(o) => {
			for (k, x) in o {
				let p = if path.is_empty() {
					k.clone()
				} else {
					format!("{path}.{k}")
				};
				flatten(x, &p, out);
			}
		}
		Value::Array(a) => {
			if a.is_empty() {
				out.push((path.to_owned(), "[]".to_owned()));
			}
			for (i, x) in a.iter().enumerate() {
				flatten(x, &format!("{path}[{i}]"), out);
			}
		}
		Value::String(s) => out.push((path.to_owned(), s.clone())),
		Value::Null => {}
		other => out.push((path.to_owned(), other.to_string())),
	}
}

fn flat(s: &TauriSnapshot) -> Vec<(String, String)> {
	let mut v = serde_json::to_value(s).unwrap_or(Value::Null);
	if let Some(o) = v.as_object_mut() {
		o.remove("sources");
		o.remove("version");
	}
	let mut out = Vec::new();
	flatten(&v, "", &mut out);
	out
}

/// The values that differ between two snapshots (the source hashes are not values).
pub fn diff(old: &TauriSnapshot, new: &TauriSnapshot) -> Vec<FieldChange> {
	let (a, b) = (flat(old), flat(new));
	let mut out = Vec::new();
	for (k, v) in &a {
		match b.iter().find(|(k2, _)| k2 == k) {
			Some((_, v2)) if v2 == v => {}
			Some((_, v2)) => out.push(FieldChange {
				field: k.clone(),
				from: Some(v.clone()),
				to: Some(v2.clone()),
			}),
			None => out.push(FieldChange {
				field: k.clone(),
				from: Some(v.clone()),
				to: None,
			}),
		}
	}
	for (k, v) in &b {
		if !a.iter().any(|(k2, _)| k2 == k) {
			out.push(FieldChange {
				field: k.clone(),
				from: None,
				to: Some(v.clone()),
			});
		}
	}
	out
}

/// How a source file differs from the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftKind {
	/// The file's hash is not the one the snapshot recorded.
	Changed,
	/// An overlay exists that the snapshot did not read.
	Added,
	/// A file the snapshot read is gone.
	Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceDrift {
	pub file: String,
	pub kind: DriftKind,
}

/// The state of an import against the files it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriftReport {
	/// `[tauri] config` does not exist.
	pub config_missing: bool,
	/// `installer/tauri.json` does not exist.
	pub no_snapshot: bool,
	pub sources: Vec<SourceDrift>,
}

impl DriftReport {
	pub fn is_stale(&self) -> bool {
		!self.sources.is_empty()
	}
}

/// Where `[tauri] config` points, as a path.
pub fn config_path(project: &Project) -> Option<PathBuf> {
	project
		.installer
		.tauri
		.as_ref()
		.map(|t| project.dir.join(&t.config))
}

/// Compares the hash of each file the snapshot was read from, and any overlay beside the config,
/// with what the snapshot recorded.
pub fn drift(project: &Project) -> Option<DriftReport> {
	let config = config_path(project)?;
	let Some(snap) = &project.tauri_snapshot else {
		return Some(DriftReport {
			config_missing: !config.is_file(),
			no_snapshot: true,
			sources: Vec::new(),
		});
	};
	if !config.is_file() {
		return Some(DriftReport {
			config_missing: true,
			no_snapshot: false,
			sources: Vec::new(),
		});
	}
	let now = source_hashes(&config).unwrap_or_default();
	let mut sources = Vec::new();
	for s in &snap.sources {
		match now.iter().find(|n| n.path == s.path) {
			Some(n) if n.sha256 == s.sha256 => {}
			Some(_) => sources.push(SourceDrift {
				file: s.path.clone(),
				kind: DriftKind::Changed,
			}),
			None => sources.push(SourceDrift {
				file: s.path.clone(),
				kind: DriftKind::Removed,
			}),
		}
	}
	for n in &now {
		if !snap.sources.iter().any(|s| s.path == n.path) {
			sources.push(SourceDrift {
				file: n.path.clone(),
				kind: DriftKind::Added,
			});
		}
	}
	Some(DriftReport {
		config_missing: false,
		no_snapshot: false,
		sources,
	})
}

/// The values the config says now, for showing what a re-import would change.
pub fn pending_changes(project: &Project) -> Result<Vec<FieldChange>> {
	let (Some(config), Some(old)) = (config_path(project), &project.tauri_snapshot) else {
		return Ok(Vec::new());
	};
	if !config.is_file() {
		return Ok(Vec::new());
	}
	Ok(diff(old, &read_snapshot(&config)?))
}

/// `to` as seen from the folder `from`, with `/` separators.
pub fn relative_path(from: &Path, to: &Path) -> String {
	let from: Vec<PathPart> = from.components().collect();
	let to_parts: Vec<PathPart> = to.components().collect();
	let common = from
		.iter()
		.zip(&to_parts)
		.take_while(|(a, b)| a == b)
		.count();
	let mut parts: Vec<String> = from[common..].iter().map(|_| "..".to_owned()).collect();
	parts.extend(
		to_parts[common..]
			.iter()
			.map(|c| c.as_os_str().to_string_lossy().into_owned()),
	);
	parts.join("/")
}

fn exe(snap: &TauriSnapshot, windows: bool) -> String {
	format!(
		"{}{}",
		snap.binary_stem(),
		if windows { ".exe" } else { "" }
	)
}

fn toml_inline(fields: &[(&str, String)]) -> String {
	format!(
		"{{ {} }}",
		fields
			.iter()
			.map(|(k, v)| format!("{k} = {v}"))
			.collect::<Vec<_>>()
			.join(", ")
	)
}

fn component_text(
	id: &str,
	snap: &TauriSnapshot,
	windows: bool,
	targets: Option<&[String]>,
) -> String {
	let bin = exe(snap, windows);
	let mut files = vec![toml_inline(&[
		("from", quote(&format!("dist/{{target}}/{bin}"))),
		("to", quote("{install}/")),
	])];
	for r in &snap.resources {
		let (from, to) = if r.from.contains(['*', '?']) {
			let name = r.from.rsplit('/').next().unwrap_or(&r.from);
			(
				format!("dist/{{target}}/{}{name}", r.to),
				format!("{{install}}/{}", r.to),
			)
		} else {
			(
				format!("dist/{{target}}/{}", r.to),
				format!("{{install}}/{}", r.to),
			)
		};
		files.push(toml_inline(&[("from", quote(&from)), ("to", quote(&to))]));
	}
	for side in &snap.external_bin {
		let name = side.rsplit('/').next().unwrap_or(side);
		let name = if windows {
			format!("{name}.exe")
		} else {
			name.to_owned()
		};
		files.push(toml_inline(&[
			("from", quote(&format!("dist/{{target}}/{name}"))),
			("to", quote("{install}/")),
		]));
	}
	let target = format!("{{install}}/{bin}");
	let mut shortcuts = vec![toml_inline(&[
		("name", quote(&snap.product_name)),
		("target", quote(&target)),
		("in", quote("start_menu")),
	])];
	if windows {
		shortcuts.push(toml_inline(&[
			("name", quote(&snap.product_name)),
			("target", quote(&target)),
			("in", quote("desktop")),
		]));
	}
	let mut s = String::new();
	s.push_str("[[component]]\n");
	s.push_str(&format!("id       = {}\n", quote(id)));
	s.push_str(&format!("name     = {}\n", quote(&snap.product_name)));
	s.push_str("required = true\norigin   = \"tauri\"\n");
	if let Some(t) = targets {
		s.push_str(&format!("targets  = {}\n", edit::string_array(t)));
	}
	let array = |name: &str, items: &[String], s: &mut String| {
		s.push_str(&format!("{name} = [\n"));
		for i in items {
			s.push_str(&format!("\t{i},\n"));
		}
		s.push_str("]\n");
	};
	array("files", &files, &mut s);
	array("shortcut", &shortcuts, &mut s);
	let types: Vec<String> = snap
		.file_associations
		.iter()
		.flat_map(|a| {
			let target = target.clone();
			a.ext.iter().map(move |e| {
				toml_inline(&[
					(
						"extension",
						quote(&format!(".{}", e.trim_start_matches('.'))),
					),
					("name", quote(&a.name)),
					("open_with", quote(&target)),
				])
			})
		})
		.collect();
	if !types.is_empty() {
		array("file_types", &types, &mut s);
	}
	if !snap.deep_link_schemes.is_empty() {
		s.push_str(&format!(
			"url_schemes = {}\n",
			edit::string_array(&snap.deep_link_schemes)
		));
	}
	s
}

/// The generated components: one for Windows targets and one for the rest when the project has
/// both kinds, otherwise one for everything.
pub fn generated_components(targets: &[Target], snap: &TauriSnapshot) -> Vec<(String, String)> {
	let win: Vec<String> = targets
		.iter()
		.filter(|t| is_windows(&t.id))
		.map(|t| t.id.clone())
		.collect();
	let other: Vec<String> = targets
		.iter()
		.filter(|t| !is_windows(&t.id))
		.map(|t| t.id.clone())
		.collect();
	let dedup = |mut v: Vec<String>| {
		v.sort();
		v.dedup();
		v
	};
	let (win, other) = (dedup(win), dedup(other));
	match (win.is_empty(), other.is_empty()) {
		(false, false) => vec![
			(
				"tauri-app".to_owned(),
				component_text("tauri-app", snap, false, Some(&other)),
			),
			(
				"tauri-app-windows".to_owned(),
				component_text("tauri-app-windows", snap, true, Some(&win)),
			),
		],
		(false, true) => vec![(
			"tauri-app".to_owned(),
			component_text("tauri-app", snap, true, None),
		)],
		_ => vec![(
			"tauri-app".to_owned(),
			component_text("tauri-app", snap, false, None),
		)],
	}
}

fn write_text(path: &Path, text: &str) -> Result<()> {
	std::fs::write(path, text).map_err(|e| EngineError::io(path, e))
}

fn read_text(path: &Path) -> Result<String> {
	std::fs::read_to_string(path).map_err(|e| EngineError::io(path, e))
}

/// A snapshot as pretty JSON, indented with tabs.
pub fn snapshot_json(snap: &TauriSnapshot) -> Result<String> {
	use serde::Serialize as _;
	let mut buf = Vec::new();
	let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
	snap.serialize(&mut ser)
		.map_err(|e| EngineError::Other(e.to_string()))?;
	Ok(String::from_utf8_lossy(&buf).into_owned() + "\n")
}

/// The three files an import writes, as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFiles {
	pub installer_toml: String,
	pub components_toml: String,
	pub snapshot_json: String,
}

/// Imports the Tauri project whose config is at `config` into the project in `installer_dir`
/// (the folder with `installer.toml`):
///
/// 1. writes `tauri.json`, the snapshot, with a hash of each source file;
/// 2. sets `[tauri] config` (and `windows = "native"` the first time) and, unless `[tauri]
///    override` names the field, `[product]` `id`, `name`, `publisher` and a literal `version`;
/// 3. adds `[updates] mode = "tauri"` when the app has an updater and the project has no
///    `[updates]`;
/// 4. replaces the generated block in `components.toml` (components with `origin = "tauri"`).
pub fn import(installer_dir: &Path, config: &Path) -> Result<ImportReport> {
	let (files, report) = prepare(installer_dir, config)?;
	write_text(&installer_dir.join("installer.toml"), &files.installer_toml)?;
	write_text(
		&installer_dir.join("components.toml"),
		&files.components_toml,
	)?;
	write_text(&report.snapshot_path, &files.snapshot_json)?;
	Ok(report)
}

/// Does everything [`import`] does except write: the new text of each file and the report. The
/// Studio writes the files itself so the change goes through its undo history.
pub fn prepare(installer_dir: &Path, config: &Path) -> Result<(ImportFiles, ImportReport)> {
	let installer_path = installer_dir.join("installer.toml");
	if !installer_path.is_file() {
		return Err(EngineError::Other(format!(
			"no installer.toml in {}; run `liminst init` first",
			installer_dir.display()
		)));
	}
	let config_abs = config
		.canonicalize()
		.map_err(|e| EngineError::io(config, e))?;
	let dir_abs = installer_dir
		.canonicalize()
		.map_err(|e| EngineError::io(installer_dir, e))?;
	let snap = read_snapshot(&config_abs)?;
	let rel = relative_path(&dir_abs, &config_abs);

	let mut text = read_text(&installer_path)?;
	let parsed: InstallerFile = toml::from_str(&text).map_err(|e| EngineError::Parse {
		path: installer_path.clone(),
		message: e.to_string(),
	})?;
	let overrides = parsed
		.tauri
		.as_ref()
		.map(|t| t.overrides.clone())
		.unwrap_or_default();
	let mut warnings = Vec::new();

	text = set_key(&text, "tauri", "config", Some(&quote(&rel)));
	if parsed.tauri.is_none() {
		text = set_key(&text, "tauri", "windows", Some("\"native\""));
	}
	let mut filled = Vec::new();
	let mut kept = Vec::new();
	let fields: [(&str, Option<String>); 4] = [
		("id", Some(snap.identifier.clone())),
		("name", Some(snap.product_name.clone())),
		("publisher", snap.publisher.clone()),
		(
			"version",
			Some(snap.app_version.clone()).filter(|v| !v.is_empty()),
		),
	];
	for (key, value) in fields {
		let Some(value) = value else { continue };
		if overrides.iter().any(|o| o == key) {
			kept.push(key.to_owned());
			continue;
		}
		if key == "version" && matches!(parsed.product.version, VersionSource::From { .. }) {
			// A version read from the build stays that way.
			kept.push(key.to_owned());
			continue;
		}
		let already = get_key(&text, "product", key).is_some_and(|v| v == quote(&value));
		text = set_key(&text, "product", key, Some(&quote(&value)));
		if !already {
			filled.push(key.to_owned());
		}
	}
	if parsed.updates.is_none()
		&& snap
			.updater
			.as_ref()
			.is_some_and(|u| !u.endpoints.is_empty())
	{
		text = set_key(&text, "updates", "mode", Some("\"tauri\""));
	}
	if is_windows_wrap_only(&snap, parsed.tauri.as_ref().map(|t| t.windows)) {
		warnings.push(
			"the app ships NSIS installerHooks or a custom template, which the native Windows install cannot reproduce; set `windows = \"wrap\"` in [tauri]"
				.to_owned(),
		);
	}
	let installer_text = text.clone();

	let targets = toml::from_str::<InstallerFile>(&text)
		.map(|f| f.targets)
		.unwrap_or_default();
	let generated = generated_components(&targets, &snap);
	let block = format!(
		"{BEGIN}\n{}\n{END}\n",
		generated
			.iter()
			.map(|(_, t)| t.trim_end().to_owned())
			.collect::<Vec<_>>()
			.join("\n\n")
	);
	let components_path = installer_dir.join("components.toml");
	let components_text = std::fs::read_to_string(&components_path).unwrap_or_default();
	// A hand-written component with a generated id would collide.
	for (id, _) in &generated {
		if components_text
			.lines()
			.any(|l| l.trim().replace(' ', "") == format!("id=\"{id}\""))
			&& !components_text.contains(BEGIN)
		{
			warnings.push(format!(
				"components.toml already has a component `{id}`; rename it, because the generated block uses that id"
			));
		}
	}
	let components_out = replace_block(&components_text, BEGIN, END, &block);

	let snapshot_path = installer_dir.join(TauriSnapshot::PATH);
	let old: Option<TauriSnapshot> = std::fs::read_to_string(&snapshot_path)
		.ok()
		.and_then(|t| serde_json::from_str(&t).ok());
	let changes = old.as_ref().map(|o| diff(o, &snap)).unwrap_or_default();
	let files = ImportFiles {
		installer_toml: installer_text,
		components_toml: components_out,
		snapshot_json: snapshot_json(&snap)?,
	};
	let report = ImportReport {
		snapshot_path,
		first_import: old.is_none(),
		changes,
		filled,
		kept,
		components: generated.into_iter().map(|(id, _)| id).collect(),
		warnings,
	};
	Ok((files, report))
}

fn is_windows_wrap_only(snap: &TauriSnapshot, mode: Option<InstallMode>) -> bool {
	(snap.windows.has_installer_hooks || snap.windows.has_custom_template)
		&& mode != Some(InstallMode::Wrap)
}

/// Imports again from the config the project already names.
pub fn reimport(project: &Project) -> Result<ImportReport> {
	let config = config_path(project)
		.ok_or_else(|| EngineError::Other("installer.toml has no [tauri] section".into()))?;
	import(&project.dir, &config)
}
