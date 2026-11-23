// The Tauri settings a project keeps in `installer.toml`, and how each WebView2 mode behaves.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_model::tauri::{
	InstallMode, UpdateMode, Updates, Webview2Mode, Webview2Override, Webview2Setting,
};

use crate::tauri::edit::{quote, set_key, string_array};

/// A change to the project's Tauri settings. A field left `None` is not touched.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsChange {
	/// How Windows installs: `native` or `wrap`.
	pub windows: Option<InstallMode>,
	/// The `[product]` fields the project keeps over the import.
	pub overrides: Option<Vec<String>>,
	/// Replaces `[tauri.webview2]`: a field left `None` there drops that key.
	pub webview2: Option<Webview2Override>,
	/// Replaces `[updates]`.
	pub updates: Option<Updates>,
}

fn install_mode(m: InstallMode) -> &'static str {
	match m {
		InstallMode::Native => "native",
		InstallMode::Wrap => "wrap",
	}
}

fn opt_str(v: &Option<String>) -> Option<String> {
	v.as_deref().filter(|s| !s.is_empty()).map(quote)
}

/// The text of `installer.toml` with the change applied. Comments and layout stay.
pub fn apply(text: &str, change: &SettingsChange) -> String {
	let mut out = text.to_owned();
	if let Some(m) = change.windows {
		out = set_key(&out, "tauri", "windows", Some(&quote(install_mode(m))));
	}
	if let Some(o) = &change.overrides {
		let value = (!o.is_empty()).then(|| string_array(o));
		out = set_key(&out, "tauri", "override", value.as_deref());
	}
	if let Some(w) = &change.webview2 {
		let s = "tauri.webview2";
		out = set_key(&out, s, "mode", w.mode.map(|m| quote(m.name())).as_deref());
		out = set_key(
			&out,
			s,
			"silent",
			w.silent.map(|b| b.to_string()).as_deref(),
		);
		out = set_key(&out, s, "path", opt_str(&w.path).as_deref());
		out = set_key(&out, s, "min_version", opt_str(&w.min_version).as_deref());
	}
	if let Some(u) = &change.updates {
		out = set_key(&out, "updates", "mode", Some(&quote(u.mode.as_str())));
		out = set_key(&out, "updates", "feed", opt_str(&u.feed).as_deref());
		out = set_key(&out, "updates", "channel", opt_str(&u.channel).as_deref());
		out = set_key(
			&out,
			"updates",
			"check_interval_hours",
			u.check_interval_hours.map(|h| h.to_string()).as_deref(),
		);
	}
	out
}

/// What a WebView2 mode does, for the Studio to show beside what it costs.
#[derive(Debug, Clone, PartialEq)]
pub struct ModeNotes {
	pub mode: Webview2Mode,
	pub size_cost_mb: f32,
	pub needs_network: bool,
	/// The payload file the mode carries, when it carries one.
	pub carries: Option<&'static str>,
	/// Where the bootstrapper comes from, for the modes that fetch it.
	pub source: Option<&'static str>,
}

pub fn mode_notes(mode: Webview2Mode) -> ModeNotes {
	use crate::tauri::plan::{EMBEDDED_BOOTSTRAPPER, EMBEDDED_OFFLINE_INSTALLER};
	use li_model::tauri::{WEBVIEW2_BOOTSTRAPPER_URL, WEBVIEW2_OFFLINE_URL};
	ModeNotes {
		mode,
		size_cost_mb: mode.size_cost_mb(),
		needs_network: mode.needs_network(),
		carries: match mode {
			Webview2Mode::EmbedBootstrapper => Some(EMBEDDED_BOOTSTRAPPER),
			Webview2Mode::OfflineInstaller => Some(EMBEDDED_OFFLINE_INSTALLER),
			_ => None,
		},
		source: match mode {
			Webview2Mode::DownloadBootstrapper | Webview2Mode::EmbedBootstrapper => {
				Some(WEBVIEW2_BOOTSTRAPPER_URL)
			}
			Webview2Mode::OfflineInstaller => Some(WEBVIEW2_OFFLINE_URL),
			_ => None,
		},
	}
}

/// What the step does at install time with a setting, for a machine with and without the
/// runtime: each line is a short English sentence key the Studio maps to its catalogue.
pub fn outcome(setting: &Webview2Setting, present: bool) -> &'static str {
	use Webview2Mode::*;
	match (setting.mode, present) {
		(Skip, _) => "skip",
		(FixedRuntime, _) => "fixed",
		(_, true) if setting.min_version.is_none() => "found",
		(_, true) => "found-checked",
		(DownloadBootstrapper, false) => "download",
		(EmbedBootstrapper, false) => "bootstrapper",
		(OfflineInstaller, false) => "offline",
	}
}

/// Whether `[updates]` is the legacy default (absent): the installer owns updates.
pub fn default_updates() -> Updates {
	Updates {
		mode: UpdateMode::Liminal,
		..Updates::default()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const DOC: &str = "[product]\nid = \"a\"\nname = \"A\"\npublisher = \"P\"\nversion = \"1\"\n\n[tauri]\nconfig = \"../c.json\"\nwindows = \"native\"\n\n[updates]\nmode = \"tauri\"\nfeed = \"f\"\n";

	#[test]
	fn windows_mode_and_updates_are_edited_in_place() {
		let out = apply(
			DOC,
			&SettingsChange {
				windows: Some(InstallMode::Wrap),
				updates: Some(Updates {
					mode: UpdateMode::Both,
					feed: None,
					channel: Some("beta".into()),
					check_interval_hours: Some(12),
				}),
				..Default::default()
			},
		);
		assert!(out.contains("windows = \"wrap\""), "{out}");
		assert!(out.contains("mode = \"both\""));
		assert!(!out.contains("feed"));
		assert!(out.contains("channel = \"beta\""));
		assert!(out.contains("check_interval_hours = 12"));
		let parsed: li_model::InstallerFile = toml::from_str(&out).unwrap();
		assert_eq!(parsed.tauri.unwrap().windows, InstallMode::Wrap);
		assert_eq!(parsed.updates.unwrap().mode, UpdateMode::Both);
	}

	#[test]
	fn a_webview2_override_replaces_the_section() {
		let out = apply(
			DOC,
			&SettingsChange {
				webview2: Some(Webview2Override {
					mode: Some(Webview2Mode::OfflineInstaller),
					min_version: Some("110.0.0.0".into()),
					..Default::default()
				}),
				..Default::default()
			},
		);
		let parsed: li_model::InstallerFile = toml::from_str(&out).unwrap();
		let o = parsed.tauri.unwrap().webview2.unwrap();
		assert_eq!(o.mode, Some(Webview2Mode::OfflineInstaller));
		assert_eq!(o.min_version.as_deref(), Some("110.0.0.0"));
		// Clearing the override drops each key.
		let out = apply(
			&out,
			&SettingsChange {
				webview2: Some(Webview2Override::default()),
				..Default::default()
			},
		);
		let parsed: li_model::InstallerFile = toml::from_str(&out).unwrap();
		assert_eq!(
			parsed.tauri.unwrap().webview2.unwrap(),
			Webview2Override::default()
		);
	}

	#[test]
	fn overrides_are_written_and_cleared() {
		let out = apply(
			DOC,
			&SettingsChange {
				overrides: Some(vec!["name".into(), "version".into()]),
				..Default::default()
			},
		);
		assert!(out.contains("override = [\"name\", \"version\"]"), "{out}");
		let out = apply(
			&out,
			&SettingsChange {
				overrides: Some(vec![]),
				..Default::default()
			},
		);
		assert!(!out.contains("override"));
	}

	#[test]
	fn each_mode_says_what_it_costs_and_where_it_comes_from() {
		assert_eq!(
			mode_notes(Webview2Mode::OfflineInstaller).size_cost_mb,
			127.0
		);
		assert!(!mode_notes(Webview2Mode::OfflineInstaller).needs_network);
		assert!(mode_notes(Webview2Mode::DownloadBootstrapper).needs_network);
		assert_eq!(mode_notes(Webview2Mode::FixedRuntime).size_cost_mb, 180.0);
		assert_eq!(outcome(&Webview2Setting::default(), true), "found");
		assert_eq!(outcome(&Webview2Setting::default(), false), "download");
	}
}
