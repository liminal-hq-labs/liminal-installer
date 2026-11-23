// Checks for a Tauri product: the import is current, the choices are possible, and the update
// settings do not fight.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_model::tauri::{InstallMode, UpdateMode, Webview2Mode};

use crate::check::{Diagnostic, error, warning};
use crate::payload::{DirPayload, expand_sources};
use crate::project::Project;
use crate::tauri::import::{DriftKind, drift};
use crate::tauri::plan::{WindowsTauri, effective_webview2, is_windows};

/// Appends the findings for `[tauri]` and `[updates]`.
pub fn check(project: &Project, out: &mut Vec<Diagnostic>) {
	let updates = project.installer.updates.as_ref();
	let mode = updates.map(|u| u.mode).unwrap_or_default();
	if let Some(u) = updates
		&& u.mode.installer_updates()
		&& u.feed.is_none()
		&& project.installer.signing.feed_key.is_none()
	{
		out.push(warning(
			"updates-no-feed",
			"installer.toml [updates]",
			format!(
				"mode `{}` has the installer apply updates from a feed, but the project names no `feed` and no `signing.feed_key`",
				u.mode.as_str()
			),
		));
	}
	let Some(section) = &project.installer.tauri else {
		if mode.app_updates() {
			out.push(error(
				"updates-needs-tauri",
				"installer.toml [updates]",
				format!(
					"mode `{}` leaves updates to the Tauri updater, which needs a [tauri] section to import its settings from",
					mode.as_str()
				),
			));
		}
		return;
	};
	let loc = "installer.toml [tauri]";

	if let Some(report) = drift(project) {
		if report.config_missing {
			out.push(error(
				"tauri-config-missing",
				loc,
				format!(
					"`config = \"{}\"` does not exist (paths are relative to installer/)",
					section.config
				),
			));
		} else if report.no_snapshot {
			out.push(error(
				"tauri-not-imported",
				loc,
				"the project names a Tauri config but has no installer/tauri.json; run `liminst import tauri`",
			));
		}
		for s in &report.sources {
			let what = match s.kind {
				DriftKind::Changed => "changed since the import",
				DriftKind::Added => "is new since the import",
				DriftKind::Removed => "is gone since the import",
			};
			out.push(error(
				"tauri-drift",
				loc,
				format!(
					"{} {what}; run `liminst import tauri {}` to accept it",
					s.file, section.config
				),
			));
		}
	}

	for (name, m) in [("macos", section.macos), ("linux", section.linux)] {
		if m == InstallMode::Wrap {
			out.push(warning(
				"tauri-wrap-unsupported",
				loc,
				format!("`{name} = \"wrap\"` has no effect: {name} installs are always native"),
			));
		}
	}

	let Some(snap) = &project.tauri_snapshot else {
		return;
	};
	for key in &section.overrides {
		if !matches!(key.as_str(), "id" | "name" | "publisher" | "version") {
			out.push(warning(
				"tauri-bad-override",
				loc,
				format!(
					"`override` names `{key}`; the product fields are id, name, publisher and version"
				),
			));
		}
	}
	let wv = effective_webview2(section, snap);
	if wv.mode == Webview2Mode::Skip {
		out.push(warning(
			"tauri-webview2-skip",
			loc,
			"WebView2 mode is `skip`: the app will not start on a machine without the runtime",
		));
	}
	if wv.mode == Webview2Mode::FixedRuntime && wv.path.is_none() {
		out.push(error(
			"tauri-webview2-path",
			loc,
			"WebView2 mode `fixedRuntime` needs a `path` to the runtime folder",
		));
	}
	if section.windows == InstallMode::Native
		&& (snap.windows.has_installer_hooks || snap.windows.has_custom_template)
	{
		out.push(warning(
			"tauri-native-hooks",
			loc,
			"the app ships NSIS installerHooks or a custom template, which the native Windows install cannot reproduce; set `windows = \"wrap\"`",
		));
	}

	if let Ok(dir) = DirPayload::new(&project.dir) {
		for t in project
			.installer
			.targets
			.iter()
			.filter(|t| is_windows(&t.id))
		{
			let Some(w) = WindowsTauri::of(project, &t.id, project.installer.defaults.scope) else {
				continue;
			};
			if w.wrap() && expand_sources(&dir, &w.setup_pattern()).is_empty() {
				out.push(error(
					"tauri-wrap-installer",
					format!("target {}", t.id),
					format!(
						"`windows = \"wrap\"` needs the Tauri setup at `{}`, which is not in the project",
						w.setup_pattern()
					),
				));
			}
			let embedded = match wv.mode {
				Webview2Mode::EmbedBootstrapper => Some(crate::tauri::plan::EMBEDDED_BOOTSTRAPPER),
				Webview2Mode::OfflineInstaller => {
					Some(crate::tauri::plan::EMBEDDED_OFFLINE_INSTALLER)
				}
				_ => None,
			};
			if let Some(path) = embedded
				&& expand_sources(&dir, path).is_empty()
			{
				let url = if wv.mode == Webview2Mode::OfflineInstaller {
					li_model::tauri::WEBVIEW2_OFFLINE_URL
				} else {
					li_model::tauri::WEBVIEW2_BOOTSTRAPPER_URL
				};
				out.push(error(
					"tauri-webview2-installer",
					format!("target {}", t.id),
					format!(
						"WebView2 mode `{}` carries `{path}` in the payload, which is not in the project (download it from {url})",
						wv.mode.name()
					),
				));
			}
		}
	}

	// Who updates the app.
	let updater = snap.updater.as_ref().filter(|u| !u.endpoints.is_empty());
	if mode.app_updates() && updater.is_none() {
		out.push(warning(
			"updates-no-updater",
			"installer.toml [updates]",
			format!(
				"mode `{}` expects the app to update itself, but tauri.conf.json configures no updater endpoints",
				mode.as_str()
			),
		));
	}
	if matches!(mode, UpdateMode::Liminal | UpdateMode::Off) && updater.is_some() {
		out.push(warning(
			"updates-conflict",
			"installer.toml [updates]",
			format!(
				"the app has the Tauri updater on, but mode `{}` says the installer owns updates: two updaters will replace each other's files. Set `mode = \"both\"` (the newer installed version wins) or `mode = \"tauri\"`, or turn the updater off in tauri.conf.json",
				mode.as_str()
			),
		));
	}
	if mode.app_updates()
		&& updater.is_some()
		&& section.windows == InstallMode::Native
		&& project.installer.targets.iter().any(|t| is_windows(&t.id))
	{
		out.push(warning(
			"updates-native-updater",
			"installer.toml [updates]",
			"with a native Windows install the Tauri updater runs Tauri's own setup over the files the engine placed; the engine catches the receipt up on the next launch or repair, and uninstall leaves the updater's uninstall.exe behind",
		));
	}
}
