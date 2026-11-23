// The Studio's settings file: a small JSON document in the app's config directory.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

/// Where every window hears about a change to the settings.
pub const SETTINGS_EVENT: &str = "studio://settings-changed";

/// The themes the design tokens define.
const THEMES: [&str; 3] = ["afterglow", "afterglow-dark", "afterglow-light"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
	/// Whether the conventional menu bar shows under the title bar.
	pub menu_bar: bool,
	/// One of the theme ids in the design tokens.
	pub theme: String,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			menu_bar: false,
			theme: "afterglow".into(),
		}
	}
}

impl Settings {
	/// Merges the fields in `patch` over these settings. A field that is unknown, of the wrong
	/// type or outside its allowed values refuses the whole patch and changes nothing.
	pub fn merged(&self, patch: &Value) -> Result<Settings, String> {
		let Value::Object(fields) = patch else {
			return Err("settings must be an object".into());
		};
		let mut current = serde_json::to_value(self).map_err(|e| e.to_string())?;
		let object = current
			.as_object_mut()
			.expect("settings serialize to an object");
		for (key, value) in fields {
			if !object.contains_key(key) {
				return Err(format!("unknown setting `{key}`"));
			}
			object.insert(key.clone(), value.clone());
		}
		let next: Settings =
			serde_json::from_value(current).map_err(|e| format!("invalid setting: {e}"))?;
		if !THEMES.contains(&next.theme.as_str()) {
			return Err(format!("unknown theme `{}`", next.theme));
		}
		Ok(next)
	}
}

/// Reads the file, taking the defaults for a missing or unreadable one, so a damaged file never blocks the app.
pub fn load(path: &Path) -> Settings {
	fs::read_to_string(path)
		.ok()
		.and_then(|text| serde_json::from_str(&text).ok())
		.unwrap_or_default()
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
	if let Some(dir) = path.parent() {
		fs::create_dir_all(dir).map_err(|e| e.to_string())?;
	}
	let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
	fs::write(path, text + "\n").map_err(|e| e.to_string())
}

fn file(app: &AppHandle) -> Result<PathBuf, String> {
	Ok(app
		.path()
		.app_config_dir()
		.map_err(|e| e.to_string())?
		.join("settings.json"))
}

#[tauri::command]
pub fn read_settings(app: AppHandle) -> Result<Settings, String> {
	Ok(load(&file(&app)?))
}

/// Merges `patch` into the saved settings, saves them and tells every window.
#[tauri::command]
pub fn write_settings(app: AppHandle, patch: Value) -> Result<Settings, String> {
	let path = file(&app)?;
	let next = load(&path).merged(&patch)?;
	save(&path, &next)?;
	app.emit(SETTINGS_EVENT, &next).map_err(|e| e.to_string())?;
	Ok(next)
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn defaults_hide_the_menu_bar() {
		let s = Settings::default();
		assert!(!s.menu_bar);
		assert_eq!(s.theme, "afterglow");
	}

	#[test]
	fn a_patch_changes_only_the_fields_it_names() {
		let next = Settings::default()
			.merged(&json!({ "menuBar": true }))
			.unwrap();
		assert!(next.menu_bar);
		assert_eq!(next.theme, "afterglow");
	}

	#[test]
	fn a_bad_patch_is_refused_whole() {
		let base = Settings::default();
		assert!(base.merged(&json!({ "menuBar": "yes" })).is_err());
		assert!(base.merged(&json!({ "theme": "neon" })).is_err());
		assert!(base.merged(&json!({ "nope": 1 })).is_err());
		assert!(base.merged(&json!({ "menuBar": true, "nope": 1 })).is_err());
		assert!(base.merged(&json!([1])).is_err());
	}

	#[test]
	fn saves_and_loads_through_a_file() {
		let dir = std::env::temp_dir().join(format!("li-studio-settings-{}", std::process::id()));
		let path = dir.join("nested").join("settings.json");
		let s = Settings {
			menu_bar: true,
			theme: "afterglow-light".into(),
		};
		save(&path, &s).unwrap();
		assert_eq!(load(&path), s);
		fs::write(&path, "{ not json").unwrap();
		assert_eq!(load(&path), Settings::default());
		assert_eq!(load(&dir.join("missing.json")), Settings::default());
		let _ = fs::remove_dir_all(&dir);
	}
}
