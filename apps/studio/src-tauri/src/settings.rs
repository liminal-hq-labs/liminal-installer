// The Studio's settings file: a versioned JSON document in the app's config directory, with typed fields and validation.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::agent::Agent;
use crate::tools;

/// Where every window hears about a change to the settings.
pub const SETTINGS_EVENT: &str = "studio://settings-changed";

/// The version this build writes. A file with a newer one is kept as it is, apart from the fields this build knows.
pub const VERSION: u32 = 1;

/// The themes the design tokens define, and the one that follows the system.
const THEMES: [&str; 4] = ["afterglow", "afterglow-dark", "afterglow-light", "system"];
const DENSITIES: [&str; 2] = ["comfortable", "compact"];
const MOTIONS: [&str; 3] = ["system", "on", "off"];

/// The most recent projects kept, and the bounds of the limit a person may choose.
pub const RECENT_LIMIT: std::ops::RangeInclusive<u32> = 5..=20;
const TEXT_SIZE: std::ops::RangeInclusive<u32> = 90..=130;
const FONT_SIZE: std::ops::RangeInclusive<u32> = 10..=24;
const TAB_WIDTH: std::ops::RangeInclusive<u32> = 1..=8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
	/// The file format version. Managed by the Studio; a patch cannot change it.
	pub version: u32,

	// General
	/// Whether the conventional menu bar shows under the title bar.
	pub menu_bar: bool,
	pub reopen_last_project: bool,
	pub confirm_discard: bool,
	pub recent_limit: u32,
	/// Newest first, at most `recent_limit` long.
	pub recent_projects: Vec<String>,
	pub last_project: Option<String>,

	// Appearance
	/// One of the theme ids in the design tokens, or `system`.
	pub theme: String,
	pub density: String,
	pub reduce_motion: String,
	/// Interface text size, as a percentage.
	pub text_size: u32,

	// Editor
	pub editor_font_size: u32,
	pub tab_width: u32,
	pub wrap_lines: bool,
	pub line_numbers: bool,
	pub format_on_save: bool,

	// Preview
	/// The full id of the target a dry run starts on; empty for the first one.
	pub preview_target: String,
	pub preview_auto_run: bool,
	pub show_skipped_steps: bool,

	// Agents
	pub mcp_enabled: bool,
	/// 0 picks a free port.
	pub mcp_port: u32,

	// Projects
	pub projects_folder: String,
	pub runner_path: String,
	pub docker_path: String,

	/// Keys this build does not know, kept so a newer or older Studio finds its own settings intact.
	#[serde(flatten)]
	pub extra: Map<String, Value>,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			version: VERSION,
			menu_bar: false,
			reopen_last_project: false,
			confirm_discard: true,
			recent_limit: 10,
			recent_projects: Vec::new(),
			last_project: None,
			theme: "afterglow".into(),
			density: "comfortable".into(),
			reduce_motion: "system".into(),
			text_size: 100,
			editor_font_size: 13,
			tab_width: 4,
			wrap_lines: true,
			line_numbers: true,
			format_on_save: false,
			preview_target: String::new(),
			preview_auto_run: false,
			show_skipped_steps: true,
			// On while developing; a release build serves nothing until it is asked to.
			mcp_enabled: cfg!(debug_assertions),
			mcp_port: 0,
			projects_folder: String::new(),
			runner_path: String::new(),
			docker_path: String::new(),
			extra: Map::new(),
		}
	}
}

fn within(name: &str, value: u32, range: &std::ops::RangeInclusive<u32>) -> Result<(), String> {
	if range.contains(&value) {
		Ok(())
	} else {
		Err(format!(
			"{name} must be between {} and {}",
			range.start(),
			range.end()
		))
	}
}

fn one_of(name: &str, value: &str, allowed: &[&str]) -> Result<(), String> {
	if allowed.contains(&value) {
		Ok(())
	} else {
		Err(format!("unknown {name} `{value}`"))
	}
}

fn clean_path(name: &str, value: &str) -> Result<(), String> {
	if value.contains('\0') || value.contains('\n') {
		Err(format!("{name} is not a valid path"))
	} else {
		Ok(())
	}
}

impl Settings {
	/// Checks every field against its allowed values.
	pub fn validate(&self) -> Result<(), String> {
		one_of("theme", &self.theme, &THEMES)?;
		one_of("density", &self.density, &DENSITIES)?;
		one_of("motion setting", &self.reduce_motion, &MOTIONS)?;
		within(
			"the recent projects limit",
			self.recent_limit,
			&RECENT_LIMIT,
		)?;
		within("text size", self.text_size, &TEXT_SIZE)?;
		within("font size", self.editor_font_size, &FONT_SIZE)?;
		within("tab width", self.tab_width, &TAB_WIDTH)?;
		within("the port", self.mcp_port, &(0..=65535))?;
		clean_path("the projects folder", &self.projects_folder)?;
		clean_path("the runner path", &self.runner_path)?;
		clean_path("the Docker path", &self.docker_path)?;
		clean_path("the preview target", &self.preview_target)?;
		if self.recent_projects.iter().any(|p| p.is_empty()) {
			return Err("a recent project has no path".into());
		}
		Ok(())
	}

	/// Puts the list fields within their limits.
	fn normalize(&mut self) {
		let mut seen = Vec::<String>::new();
		for p in self.recent_projects.drain(..) {
			if !seen.contains(&p) {
				seen.push(p);
			}
		}
		seen.truncate(self.recent_limit as usize);
		self.recent_projects = seen;
	}

	/// The keys of the typed fields.
	fn known_keys() -> Vec<String> {
		match serde_json::to_value(Settings::default()) {
			Ok(Value::Object(o)) => o.keys().cloned().collect(),
			_ => Vec::new(),
		}
	}

	/// Merges the fields in `patch` over these settings. A field that is unknown, of the wrong
	/// type or outside its allowed values refuses the whole patch and changes nothing.
	pub fn merged(&self, patch: &Value) -> Result<Settings, String> {
		let Value::Object(fields) = patch else {
			return Err("settings must be an object".into());
		};
		let known = Self::known_keys();
		let mut current = serde_json::to_value(self).map_err(|e| e.to_string())?;
		let object = current
			.as_object_mut()
			.expect("settings serialize to an object");
		for (key, value) in fields {
			if key == "version" {
				return Err("the version is managed by the Studio".into());
			}
			if !known.contains(key) {
				return Err(format!("unknown setting `{key}`"));
			}
			object.insert(key.clone(), value.clone());
		}
		let mut next: Settings =
			serde_json::from_value(current).map_err(|e| format!("invalid setting: {e}"))?;
		next.validate()?;
		next.normalize();
		Ok(next)
	}
}

/// Brings a file written by an older Studio up to the current version. The first file had only `menuBar` and
/// `theme`, with no version; every field it had keeps its meaning, so only the version is added.
fn migrate(value: &mut Value) {
	if let Value::Object(o) = value {
		let version = o.get("version").and_then(Value::as_u64).unwrap_or(0);
		if version < u64::from(VERSION) {
			o.insert("version".into(), VERSION.into());
		}
	}
}

/// Settings from a parsed file. A field that is of the wrong type or out of range takes its default and the others
/// stay, and keys this build does not know are kept.
fn from_value(mut value: Value) -> Settings {
	migrate(&mut value);
	let Value::Object(fields) = value else {
		return Settings::default();
	};
	let known = Settings::known_keys();
	let mut settings = Settings::default();
	for (key, field) in &fields {
		if key == "version" {
			continue;
		}
		if known.contains(key) {
			let mut patch = Map::new();
			patch.insert(key.clone(), field.clone());
			if let Ok(next) = settings.merged(&Value::Object(patch)) {
				settings = next;
			}
		} else {
			settings.extra.insert(key.clone(), field.clone());
		}
	}
	if let Some(v) = fields.get("version").and_then(Value::as_u64) {
		settings.version = u32::try_from(v).unwrap_or(VERSION).max(VERSION);
	}
	settings
}

/// Reads the file, taking the defaults for a missing or unreadable one, so a damaged file never blocks the app.
pub fn load(path: &Path) -> Settings {
	fs::read_to_string(path)
		.ok()
		.and_then(|text| serde_json::from_str::<Value>(&text).ok())
		.map(from_value)
		.unwrap_or_default()
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
	if let Some(dir) = path.parent() {
		fs::create_dir_all(dir).map_err(|e| e.to_string())?;
	}
	let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
	fs::write(path, text + "\n").map_err(|e| e.to_string())
}

pub fn file(app: &AppHandle) -> Result<PathBuf, String> {
	Ok(app
		.path()
		.app_config_dir()
		.map_err(|e| e.to_string())?
		.join("settings.json"))
}

/// What a write needs from the machine, beyond the values themselves: a folder that exists and tools that run.
/// Only a field the patch names is looked at, so an unrelated change is never refused for an old path.
pub fn check_machine(next: &Settings, patch: &Value) -> Result<(), String> {
	let names = |key: &str| patch.get(key).is_some();
	if names("projectsFolder") && !next.projects_folder.is_empty() {
		let dir = Path::new(&next.projects_folder);
		if !dir.is_dir() {
			return Err(format!("`{}` is not a folder", next.projects_folder));
		}
	}
	if names("runnerPath") && !next.runner_path.is_empty() {
		tools::check_version(Path::new(&next.runner_path))
			.map_err(|e| format!("the runner did not answer `--version`: {e}"))?;
	}
	if names("dockerPath") && !next.docker_path.is_empty() {
		tools::check_version(Path::new(&next.docker_path))
			.map_err(|e| format!("Docker did not answer `--version`: {e}"))?;
	}
	Ok(())
}

#[tauri::command]
pub fn read_settings(app: AppHandle) -> Result<Settings, String> {
	Ok(load(&file(&app)?))
}

/// Merges `patch` into the saved settings, saves them and tells every window. A patch that cannot be applied,
/// including an agent port that is taken, is refused whole with the reason.
#[tauri::command]
pub fn write_settings(
	app: AppHandle,
	agent: State<Agent>,
	patch: Value,
) -> Result<Settings, String> {
	let path = file(&app)?;
	let current = load(&path);
	let next = current.merged(&patch)?;
	check_machine(&next, &patch)?;
	apply_agent(&agent, &current, &next)?;
	if let Err(e) = save(&path, &next) {
		let _ = apply_agent(&agent, &next, &current);
		return Err(e);
	}
	app.emit(SETTINGS_EVENT, &next).map_err(|e| e.to_string())?;
	Ok(next)
}

/// Puts every setting back to its default and keeps the keys this build does not know.
#[tauri::command]
pub fn reset_settings(app: AppHandle, agent: State<Agent>) -> Result<Settings, String> {
	let path = file(&app)?;
	let current = load(&path);
	let next = Settings {
		extra: current.extra.clone(),
		..Settings::default()
	};
	apply_agent(&agent, &current, &next)?;
	save(&path, &next)?;
	app.emit(SETTINGS_EVENT, &next).map_err(|e| e.to_string())?;
	Ok(next)
}

/// Starts, stops or moves the agent server when its settings changed.
pub fn apply_agent(agent: &Agent, from: &Settings, to: &Settings) -> Result<(), String> {
	if from.mcp_enabled != to.mcp_enabled || from.mcp_port != to.mcp_port {
		agent.apply(to.mcp_enabled, to.mcp_port)?;
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	fn temp(name: &str) -> PathBuf {
		std::env::temp_dir().join(format!("li-studio-settings-{}-{name}", std::process::id()))
	}

	#[test]
	fn defaults_are_the_documented_ones() {
		let s = Settings::default();
		assert_eq!(s.version, VERSION);
		assert!(!s.menu_bar);
		assert!(s.confirm_discard);
		assert_eq!((s.recent_limit, s.text_size, s.tab_width), (10, 100, 4));
		assert_eq!(s.theme, "afterglow");
		assert_eq!(s.mcp_enabled, cfg!(debug_assertions));
		assert_eq!(s.mcp_port, 0);
		assert!(s.validate().is_ok());
	}

	#[test]
	fn a_patch_changes_only_the_fields_it_names() {
		let next = Settings::default()
			.merged(&json!({ "menuBar": true, "tabWidth": 2 }))
			.unwrap();
		assert!(next.menu_bar);
		assert_eq!(next.tab_width, 2);
		assert_eq!(next.theme, "afterglow");
	}

	#[test]
	fn a_bad_patch_is_refused_whole() {
		let base = Settings::default();
		for patch in [
			json!({ "menuBar": "yes" }),
			json!({ "theme": "neon" }),
			json!({ "density": "tiny" }),
			json!({ "reduceMotion": "maybe" }),
			json!({ "textSize": 89 }),
			json!({ "textSize": 131 }),
			json!({ "editorFontSize": 9 }),
			json!({ "tabWidth": 0 }),
			json!({ "recentLimit": 4 }),
			json!({ "recentLimit": 21 }),
			json!({ "mcpPort": 70000 }),
			json!({ "mcpPort": -1 }),
			json!({ "runnerPath": "a\nb" }),
			json!({ "recentProjects": [""] }),
			json!({ "nope": 1 }),
			json!({ "version": 9 }),
			json!({ "menuBar": true, "nope": 1 }),
			json!([1]),
		] {
			assert!(base.merged(&patch).is_err(), "{patch}");
		}
	}

	#[test]
	fn the_theme_may_follow_the_system() {
		let s = Settings::default();
		assert_eq!(
			s.merged(&json!({ "theme": "system" })).unwrap().theme,
			"system"
		);
	}

	#[test]
	fn recent_projects_are_deduplicated_and_cut_to_the_limit() {
		let s = Settings::default();
		let many: Vec<String> = (0..30).map(|i| format!("/p/{i}")).collect();
		let next = s.merged(&json!({ "recentProjects": many })).unwrap();
		assert_eq!(next.recent_projects.len(), 10);
		let dup = s
			.merged(&json!({ "recentProjects": ["/a", "/b", "/a"] }))
			.unwrap();
		assert_eq!(dup.recent_projects, ["/a", "/b"]);
		let lowered = next.merged(&json!({ "recentLimit": 5 })).unwrap();
		assert_eq!(lowered.recent_projects.len(), 5);
	}

	#[test]
	fn the_old_two_key_file_is_migrated_and_keeps_its_values() {
		let s = from_value(json!({ "menuBar": true, "theme": "afterglow-light" }));
		assert_eq!(s.version, VERSION);
		assert!(s.menu_bar);
		assert_eq!(s.theme, "afterglow-light");
		assert_eq!(
			Settings {
				menu_bar: false,
				theme: "afterglow".into(),
				..s
			},
			Settings::default()
		);
	}

	#[test]
	fn a_bad_field_in_a_file_falls_back_alone_and_unknown_keys_are_kept() {
		let s = from_value(json!({
			"menuBar": true,
			"theme": "neon",
			"tabWidth": "wide",
			"wrapLines": false,
			"futureThing": { "a": 1 }
		}));
		assert!(s.menu_bar);
		assert!(!s.wrap_lines);
		assert_eq!(s.theme, "afterglow");
		assert_eq!(s.tab_width, 4);
		assert_eq!(s.extra.get("futureThing"), Some(&json!({ "a": 1 })));
		let text = serde_json::to_string(&s).unwrap();
		assert!(text.contains("\"futureThing\""));
	}

	#[test]
	fn a_newer_version_is_not_lowered() {
		let s = from_value(json!({ "version": 7, "menuBar": true }));
		assert_eq!(s.version, 7);
		assert!(s.menu_bar);
	}

	#[test]
	fn saves_and_loads_through_a_file() {
		let dir = temp("file");
		let path = dir.join("nested").join("settings.json");
		let mut s = Settings {
			menu_bar: true,
			theme: "afterglow-light".into(),
			..Settings::default()
		};
		s.extra.insert("other".into(), json!(3));
		save(&path, &s).unwrap();
		assert_eq!(load(&path), s);
		fs::write(&path, "{ not json").unwrap();
		assert_eq!(load(&path), Settings::default());
		fs::write(&path, "[1]").unwrap();
		assert_eq!(load(&path), Settings::default());
		assert_eq!(load(&dir.join("missing.json")), Settings::default());
		let _ = fs::remove_dir_all(&dir);
	}

	#[test]
	fn machine_checks_look_only_at_the_fields_in_the_patch() {
		let dir = temp("machine");
		fs::create_dir_all(&dir).unwrap();
		let mut s = Settings {
			runner_path: "/definitely/not/here".into(),
			..Settings::default()
		};
		// The old runner path is not in the patch, so an unrelated change goes through.
		assert!(check_machine(&s, &json!({ "tabWidth": 2 })).is_ok());
		assert!(check_machine(&s, &json!({ "runnerPath": s.runner_path })).is_err());
		s.runner_path.clear();
		s.projects_folder = dir.to_string_lossy().into_owned();
		assert!(check_machine(&s, &json!({ "projectsFolder": s.projects_folder })).is_ok());
		s.projects_folder = dir.join("missing").to_string_lossy().into_owned();
		let refused = check_machine(&s, &json!({ "projectsFolder": s.projects_folder }));
		assert!(refused.unwrap_err().contains("not a folder"));
		let _ = fs::remove_dir_all(&dir);
	}
}
