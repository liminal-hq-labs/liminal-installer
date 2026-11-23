// The Studio's view of a Tauri project: what was imported, what drifted, and the choices a person
// makes about WebView2, the Windows install mode and updates.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! One state for the Tauri screen and the MCP tools: [`TauriView`]. Importing and changing
//! settings write `installer.toml`, `components.toml` and `tauri.json` as one undoable change.

use std::path::Path;

use li_engine::tauri::import::{self, DriftKind};
use li_engine::tauri::plan::effective_webview2;
use li_engine::tauri::settings::{self, SettingsChange};
use li_model::tauri::{
	InstallMode, UpdateMode, Updates, Webview2Mode, Webview2Override, Webview2Setting,
};
use li_model::{TauriSnapshot, project::VersionSource};
use serde::{Deserialize, Serialize};

use crate::backend::{Problem, Severity};
use crate::error::{ErrorKind, StudioError};
use crate::history::Source;
use crate::project::{Doc, DocKey, TauriFiles};
use crate::studio::Studio;

#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TauriFieldChange {
	/// A path such as `windows.webview2.mode`.
	pub field: String,
	pub from: Option<String>,
	pub to: Option<String>,
}

impl From<import::FieldChange> for TauriFieldChange {
	fn from(c: import::FieldChange) -> Self {
		TauriFieldChange {
			field: c.field,
			from: c.from,
			to: c.to,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TauriDrift {
	pub file: String,
	/// `changed`, `added` or `removed`.
	#[ts(type = "'changed' | 'added' | 'removed'")]
	pub kind: &'static str,
}

/// One `[product]` field: what the import says, what the project says and which one wins.
#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TauriIdentityRow {
	/// `id`, `name`, `publisher` or `version`.
	pub field: String,
	pub imported: Option<String>,
	pub project: Option<String>,
	/// The project keeps its own value (`[tauri] override`).
	pub overridden: bool,
	/// The two differ, so the next import would change the project (unless it is overridden).
	pub differs: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TauriModeInfo {
	pub mode: Webview2Mode,
	/// What the mode adds to the installer, in megabytes.
	pub size_cost_mb: f32,
	pub needs_network: bool,
	/// The payload file the mode carries.
	pub carries: Option<String>,
	/// Where it comes from.
	pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TauriWebview2 {
	/// What `tauri.conf.json` says.
	pub imported: Webview2Setting,
	/// The project's `[tauri.webview2]`, when it has one.
	#[serde(rename = "override")]
	#[ts(rename = "override")]
	pub overridden: Option<Webview2Override>,
	/// What the plan uses.
	pub effective: Webview2Setting,
	pub modes: Vec<TauriModeInfo>,
	/// What the step does on a machine with no runtime: `skip`, `fixed`, `download`, `bootstrapper`
	/// or `offline`.
	pub if_missing: String,
	/// What it does on a machine that has one: `skip`, `fixed`, `found` or `found-checked`.
	pub if_present: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TauriUpdates {
	/// The project has an `[updates]` section.
	pub configured: bool,
	pub mode: UpdateMode,
	pub feed: Option<String>,
	pub channel: Option<String>,
	pub check_interval_hours: Option<u32>,
	/// The engine applies updates from the feed.
	pub installer_applies: bool,
	/// The installed app updates itself.
	pub app_applies: bool,
	/// `tauri.conf.json` configures an updater with endpoints.
	pub app_has_updater: bool,
	pub endpoints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TauriView {
	/// The project has a `[tauri]` section.
	pub configured: bool,
	pub config: Option<String>,
	/// The file `config` names exists.
	pub config_found: bool,
	/// `installer/tauri.json` exists.
	pub imported: bool,
	/// A source file differs from the snapshot.
	pub stale: bool,
	pub drift: Vec<TauriDrift>,
	/// What importing again would change in the snapshot's values.
	pub pending: Vec<TauriFieldChange>,
	pub snapshot: Option<TauriSnapshot>,
	pub windows: InstallMode,
	pub macos: InstallMode,
	pub linux: InstallMode,
	/// The app has NSIS hooks or a custom template, which only `wrap` carries out.
	pub needs_wrap: bool,
	pub identity: Vec<TauriIdentityRow>,
	pub webview2: Option<TauriWebview2>,
	pub updates: TauriUpdates,
	/// The Tauri and updates findings from the engine's checks.
	pub problems: Vec<Problem>,
}

/// What an import did, with the state after it.
#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TauriImportResult {
	pub view: TauriView,
	pub first_import: bool,
	pub changes: Vec<TauriFieldChange>,
	pub filled: Vec<String>,
	pub kept: Vec<String>,
	pub components: Vec<String>,
	pub warnings: Vec<String>,
}

/// A change to the project's Tauri settings; a field left out is not touched.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct TauriSettingsChange {
	#[serde(default)]
	#[ts(optional)]
	pub windows: Option<InstallMode>,
	#[serde(default)]
	#[ts(optional)]
	pub overrides: Option<Vec<String>>,
	/// Replaces `[tauri.webview2]`; send `{}` to drop the override.
	#[serde(default)]
	#[ts(optional)]
	pub webview2: Option<Webview2Override>,
	/// Replaces `[updates]`.
	#[serde(default)]
	#[ts(optional)]
	pub updates: Option<Updates>,
}

impl From<TauriSettingsChange> for SettingsChange {
	fn from(c: TauriSettingsChange) -> Self {
		SettingsChange {
			windows: c.windows,
			overrides: c.overrides,
			webview2: c.webview2,
			updates: c.updates,
		}
	}
}

fn engine_err(e: li_engine::EngineError) -> StudioError {
	StudioError::new(ErrorKind::InvalidInput, e)
}

fn load(root: &Path) -> Result<li_engine::Project, StudioError> {
	li_engine::Project::load_installer_dir(root).map_err(engine_err)
}

fn version_text(v: &VersionSource) -> Option<String> {
	match v {
		VersionSource::Literal(v) => Some(v.clone()),
		VersionSource::From { .. } => None,
	}
}

fn identity(project: &li_engine::Project) -> Vec<TauriIdentityRow> {
	let (Some(section), Some(snap)) = (&project.installer.tauri, &project.tauri_snapshot) else {
		return Vec::new();
	};
	let p = &project.installer.product;
	let rows = [
		("id", Some(snap.identifier.clone()), Some(p.id.clone())),
		(
			"name",
			Some(snap.product_name.clone()),
			Some(p.name.clone()),
		),
		(
			"publisher",
			snap.publisher.clone(),
			Some(p.publisher.clone()),
		),
		(
			"version",
			Some(snap.app_version.clone()).filter(|v| !v.is_empty()),
			version_text(&p.version),
		),
	];
	rows.into_iter()
		.map(|(field, imported, project)| {
			let overridden = section.overrides.iter().any(|o| o == field)
				|| (field == "version" && version_text(&p.version).is_none());
			let differs = imported.is_some() && imported != project;
			TauriIdentityRow {
				field: field.to_owned(),
				imported,
				project,
				overridden,
				differs,
			}
		})
		.collect()
}

fn webview2_view(project: &li_engine::Project) -> Option<TauriWebview2> {
	let (section, snap) = (
		project.installer.tauri.as_ref()?,
		project.tauri_snapshot.as_ref()?,
	);
	let effective = effective_webview2(section, snap);
	Some(TauriWebview2 {
		imported: snap.windows.webview2.clone(),
		overridden: section.webview2.clone(),
		modes: Webview2Mode::ALL
			.into_iter()
			.map(|m| {
				let n = settings::mode_notes(m);
				TauriModeInfo {
					mode: m,
					size_cost_mb: n.size_cost_mb,
					needs_network: n.needs_network,
					carries: n.carries.map(str::to_owned),
					source: n.source.map(str::to_owned),
				}
			})
			.collect(),
		if_missing: settings::outcome(&effective, false).to_owned(),
		if_present: settings::outcome(&effective, true).to_owned(),
		effective,
	})
}

fn updates_view(project: &li_engine::Project) -> TauriUpdates {
	let configured = project.installer.updates.is_some();
	let u = project.installer.updates.clone().unwrap_or_default();
	let endpoints = project
		.tauri_snapshot
		.as_ref()
		.and_then(|s| s.updater.as_ref())
		.map(|u| u.endpoints.clone())
		.unwrap_or_default();
	TauriUpdates {
		configured,
		mode: u.mode,
		feed: u.feed,
		channel: u.channel,
		check_interval_hours: u.check_interval_hours,
		installer_applies: u.mode.installer_updates(),
		app_applies: u.mode.app_updates(),
		app_has_updater: !endpoints.is_empty(),
		endpoints,
	}
}

fn view_of(project: &li_engine::Project) -> Result<TauriView, StudioError> {
	let section = project.installer.tauri.as_ref();
	let report = import::drift(project);
	let drift: Vec<TauriDrift> = report
		.as_ref()
		.map(|r| {
			r.sources
				.iter()
				.map(|s| TauriDrift {
					file: s.file.clone(),
					kind: match s.kind {
						DriftKind::Changed => "changed",
						DriftKind::Added => "added",
						DriftKind::Removed => "removed",
					},
				})
				.collect()
		})
		.unwrap_or_default();
	let snap = project.tauri_snapshot.as_ref();
	let problems = li_engine::check(project)
		.into_iter()
		.filter(|d| d.code.starts_with("tauri-") || d.code.starts_with("updates-"))
		.map(|d| {
			let scope = if d.code.starts_with("updates-") {
				"updates"
			} else {
				"tauri"
			};
			let msg = format!("{} ({})", d.message, d.code);
			match d.severity {
				li_engine::Severity::Error => Problem::error(scope, msg),
				li_engine::Severity::Warning => Problem::warning(scope, msg),
			}
		})
		.collect();
	Ok(TauriView {
		configured: section.is_some(),
		config: section.map(|s| s.config.clone()),
		config_found: report.as_ref().is_some_and(|r| !r.config_missing),
		imported: snap.is_some(),
		stale: report.as_ref().is_some_and(|r| r.is_stale()),
		drift,
		pending: import::pending_changes(project)
			.unwrap_or_default()
			.into_iter()
			.map(Into::into)
			.collect(),
		snapshot: snap.cloned(),
		windows: section.map(|s| s.windows).unwrap_or_default(),
		macos: section.map(|s| s.macos).unwrap_or_default(),
		linux: section.map(|s| s.linux).unwrap_or_default(),
		needs_wrap: snap
			.is_some_and(|s| s.windows.has_installer_hooks || s.windows.has_custom_template),
		identity: identity(project),
		webview2: webview2_view(project),
		updates: updates_view(project),
		problems,
	})
}

fn read_files(root: &Path) -> Result<TauriFiles, StudioError> {
	let read = |name: &str| std::fs::read_to_string(root.join(name));
	Ok(TauriFiles {
		installer_toml: read("installer.toml")
			.map_err(|e| StudioError::io(root.join("installer.toml"), e))?,
		components_toml: read("components.toml").unwrap_or_default(),
		snapshot_json: read(TauriSnapshot::PATH).ok(),
	})
}

impl Studio {
	/// The Tauri screen's state: the import, its drift, WebView2, the install mode and updates.
	pub fn tauri_view(&self) -> Result<TauriView, StudioError> {
		self.with_project(|p| view_of(&load(&p.root)?))
	}

	/// Imports a `tauri.conf.json` (default: the one `[tauri] config` names; a relative path is
	/// read from `installer/`) as one undoable change.
	pub fn tauri_import(
		&self,
		config: Option<&str>,
		label: &str,
		source: Source,
	) -> Result<TauriImportResult, StudioError> {
		let root = self.with_project(|p| Ok(p.root.clone()))?;
		let config = match config {
			Some(c) if Path::new(c).is_absolute() => Path::new(c).to_owned(),
			Some(c) => root.join(c),
			None => {
				let project = load(&root)?;
				import::config_path(&project).ok_or_else(|| {
					StudioError::invalid(
						"tauri config",
						"name the tauri.conf.json to import; the project has no [tauri] section yet",
					)
				})?
			}
		};
		let (files, report) = import::prepare(&root, &config).map_err(engine_err)?;
		let doc = Doc::Tauri(TauriFiles {
			installer_toml: files.installer_toml,
			components_toml: files.components_toml,
			snapshot_json: Some(files.snapshot_json),
		});
		self.apply(DocKey::Tauri, Some(doc), label, source)?;
		Ok(TauriImportResult {
			view: self.tauri_view()?,
			first_import: report.first_import,
			changes: report.changes.into_iter().map(Into::into).collect(),
			filled: report.filled,
			kept: report.kept,
			components: report.components,
			warnings: report.warnings,
		})
	}

	/// Changes the Tauri settings in `installer.toml` as one undoable change.
	pub fn tauri_set(
		&self,
		change: TauriSettingsChange,
		label: &str,
		source: Source,
	) -> Result<TauriView, StudioError> {
		let root = self.with_project(|p| Ok(p.root.clone()))?;
		let mut files = read_files(&root)?;
		let has_tauri = toml::from_str::<li_model::InstallerFile>(&files.installer_toml)
			.map(|f| f.tauri.is_some())
			.unwrap_or(false);
		if !has_tauri && (change.windows.is_some() || change.webview2.is_some()) {
			return Err(StudioError::invalid(
				"tauri settings",
				"the project has no [tauri] section; import a tauri.conf.json first",
			));
		}
		files.installer_toml = settings::apply(&files.installer_toml, &change.into());
		self.apply(DocKey::Tauri, Some(Doc::Tauri(files)), label, source)?;
		self.tauri_view()
	}
}

/// The problems from a view, errors first, for callers that want a count.
pub fn error_count(view: &TauriView) -> usize {
	view.problems
		.iter()
		.filter(|p| p.severity == Severity::Error)
		.count()
}

#[cfg(test)]
mod tests {
	use std::fs;

	use serde_json::json;

	use super::*;
	use crate::mcp::call_tool;
	use crate::studio::testutil::open_example;

	fn read(root: &Path, name: &str) -> String {
		fs::read_to_string(root.join(name)).unwrap()
	}

	#[test]
	fn the_example_is_imported_and_current() {
		let (_d, s, _root) = open_example("tauri-notes");
		let v = s.tauri_view().unwrap();
		assert!(v.configured && v.config_found && v.imported && !v.stale);
		assert_eq!(v.config.as_deref(), Some("../src-tauri/tauri.conf.json"));
		assert_eq!(v.windows, InstallMode::Native);
		assert!(v.drift.is_empty() && v.pending.is_empty());
		let w = v.webview2.unwrap();
		assert_eq!(w.imported.mode, Webview2Mode::EmbedBootstrapper);
		assert_eq!(w.effective.mode, Webview2Mode::EmbedBootstrapper);
		assert!(w.overridden.is_none());
		assert_eq!(w.modes.len(), 5);
		let offline = w
			.modes
			.iter()
			.find(|m| m.mode == Webview2Mode::OfflineInstaller);
		assert_eq!(offline.unwrap().size_cost_mb, 127.0);
		assert_eq!(
			(w.if_missing.as_str(), w.if_present.as_str()),
			("bootstrapper", "found-checked")
		);
		assert_eq!(v.updates.mode, UpdateMode::Both);
		assert!(v.updates.installer_applies && v.updates.app_applies && v.updates.app_has_updater);
		assert_eq!(v.identity.len(), 4);
		assert!(v.identity.iter().all(|r| !r.differs && !r.overridden));
		assert!(
			v.problems.iter().all(|p| p.severity == Severity::Warning),
			"{:?}",
			v.problems
		);
	}

	#[test]
	fn a_project_that_is_not_tauri_has_an_empty_state() {
		let (_d, s, _root) = open_example("neutral");
		let v = s.tauri_view().unwrap();
		assert!(!v.configured && !v.imported && v.webview2.is_none() && v.identity.is_empty());
		assert_eq!(v.updates.mode, UpdateMode::Liminal);
		assert!(!v.updates.configured);
	}

	#[test]
	fn drift_shows_the_file_and_what_an_import_would_change() {
		let (d, s, _root) = open_example("tauri-notes");
		let conf = d.path().join("src-tauri/tauri.conf.json");
		let text = fs::read_to_string(&conf).unwrap().replace("0.4.2", "0.5.0");
		fs::write(&conf, text).unwrap();
		let v = s.tauri_view().unwrap();
		assert!(v.stale);
		assert_eq!(
			v.drift,
			vec![TauriDrift {
				file: "tauri.conf.json".into(),
				kind: "changed"
			}]
		);
		assert_eq!(v.pending.len(), 1);
		assert_eq!(v.pending[0].field, "app_version");
		assert!(
			v.problems
				.iter()
				.any(|p| p.message.contains("tauri-drift") && p.severity == Severity::Error)
		);
		assert!(error_count(&v) >= 1);
	}

	#[test]
	fn importing_is_one_undoable_change() {
		let (d, s, root) = open_example("tauri-notes");
		let conf = d.path().join("src-tauri/tauri.conf.json");
		let text = fs::read_to_string(&conf).unwrap().replace("0.4.2", "0.5.0");
		fs::write(&conf, text).unwrap();
		let before = (
			read(&root, "installer.toml"),
			read(&root, "components.toml"),
			read(&root, "tauri.json"),
		);
		let r = s
			.tauri_import(None, "Import tauri.conf.json", Source::Ui)
			.unwrap();
		assert!(!r.first_import);
		assert_eq!(r.filled, vec!["version"]);
		assert_eq!(r.changes.len(), 1);
		assert!(!r.view.stale && r.view.pending.is_empty());
		assert!(read(&root, "installer.toml").contains("version   = \"0.5.0\""));
		assert!(read(&root, "tauri.json").contains("0.5.0"));
		assert_eq!(s.history().entries.len(), 1);
		assert_eq!(s.history().entries[0].doc, DocKey::Tauri);

		s.undo(Source::Ui).unwrap();
		assert_eq!(
			(
				read(&root, "installer.toml"),
				read(&root, "components.toml"),
				read(&root, "tauri.json")
			),
			before
		);
		assert!(s.tauri_view().unwrap().stale);
		s.redo(Source::Ui).unwrap();
		assert!(!s.tauri_view().unwrap().stale);
	}

	#[test]
	fn a_first_import_names_the_config_and_creates_the_snapshot() {
		let (d, s, root) = open_example("tauri-notes");
		fs::remove_file(root.join("tauri.json")).unwrap();
		// Re-open so the project reflects the missing snapshot.
		s.open(d.path()).unwrap();
		let v = s.tauri_view().unwrap();
		assert!(v.configured && !v.imported);
		assert!(
			v.problems
				.iter()
				.any(|p| p.message.contains("tauri-not-imported"))
		);
		let abs = d.path().join("src-tauri/tauri.conf.json");
		let r = s
			.tauri_import(Some(abs.to_str().unwrap()), "Import", Source::Mcp)
			.unwrap();
		assert!(r.first_import && r.view.imported && !r.view.stale);
		assert!(root.join("tauri.json").is_file());
		// Undo removes the snapshot again.
		s.undo(Source::Mcp).unwrap();
		assert!(!root.join("tauri.json").exists());
	}

	#[test]
	fn a_project_with_no_tauri_section_needs_a_path_to_import() {
		let (_d, s, _root) = open_example("neutral");
		let err = s.tauri_import(None, "Import", Source::Ui).unwrap_err();
		assert!(err.to_string().contains("[tauri]"), "{err}");
	}

	#[test]
	fn settings_change_the_mode_webview2_and_updates() {
		let (_d, s, root) = open_example("tauri-notes");
		let before = read(&root, "installer.toml");
		let v = s
			.tauri_set(
				TauriSettingsChange {
					windows: Some(InstallMode::Wrap),
					webview2: Some(Webview2Override {
						mode: Some(Webview2Mode::OfflineInstaller),
						..Default::default()
					}),
					updates: Some(Updates {
						mode: UpdateMode::Liminal,
						feed: Some("https://example.org/f.json".into()),
						channel: Some("beta".into()),
						check_interval_hours: None,
					}),
					..Default::default()
				},
				"Change Tauri settings",
				Source::Ui,
			)
			.unwrap();
		assert_eq!(v.windows, InstallMode::Wrap);
		let w = v.webview2.unwrap();
		assert_eq!(w.effective.mode, Webview2Mode::OfflineInstaller);
		// The override keeps the import's minimum version and says the import is untouched.
		assert_eq!(w.effective.min_version.as_deref(), Some("110.0.1587.40"));
		assert_eq!(w.imported.mode, Webview2Mode::EmbedBootstrapper);
		assert_eq!(w.if_missing, "offline");
		assert_eq!(v.updates.mode, UpdateMode::Liminal);
		assert_eq!(v.updates.channel.as_deref(), Some("beta"));
		assert!(!v.updates.app_applies);
		// The app has an updater but the installer owns updates: the conflict is reported.
		assert!(
			v.problems
				.iter()
				.any(|p| p.scope == "updates" && p.message.contains("updates-conflict"))
		);
		// The comment-preserving edit left the rest of the file alone.
		let after = read(&root, "installer.toml");
		assert!(after.starts_with("# Tauri example project"));
		assert!(after.contains("[[target]]"));

		s.undo(Source::Ui).unwrap();
		assert_eq!(read(&root, "installer.toml"), before);
		assert_eq!(s.tauri_view().unwrap().windows, InstallMode::Native);
	}

	#[test]
	fn settings_that_need_a_tauri_section_are_refused_without_one() {
		let (_d, s, _root) = open_example("neutral");
		let err = s
			.tauri_set(
				TauriSettingsChange {
					windows: Some(InstallMode::Wrap),
					..Default::default()
				},
				"x",
				Source::Ui,
			)
			.unwrap_err();
		assert!(err.to_string().contains("no [tauri] section"), "{err}");
		// Updates alone make sense for any product.
		let v = s
			.tauri_set(
				TauriSettingsChange {
					updates: Some(Updates {
						mode: UpdateMode::Off,
						..Default::default()
					}),
					..Default::default()
				},
				"Turn updates off",
				Source::Ui,
			)
			.unwrap();
		assert_eq!(v.updates.mode, UpdateMode::Off);
		assert!(v.updates.configured && !v.updates.installer_applies);
	}

	#[test]
	fn the_mcp_tools_read_import_and_change() {
		let (d, s, root) = open_example("tauri-notes");
		let v = call_tool(&s, "get_tauri", &json!({})).unwrap();
		assert_eq!(v["windows"], "native");
		assert_eq!(v["webview2"]["effective"]["mode"], "embedBootstrapper");
		assert_eq!(v["updates"]["mode"], "both");

		let v = call_tool(
			&s,
			"update_tauri_settings",
			&json!({"windows": "wrap", "webview2": {"mode": "skip"}, "label": "Agent: wrap, skip"}),
		)
		.unwrap();
		assert_eq!(v["windows"], "wrap");
		assert_eq!(v["webview2"]["effective"]["mode"], "skip");
		assert_eq!(s.history().entries[0].label, "Agent: wrap, skip");
		assert_eq!(s.history().entries[0].source, Source::Mcp);

		let err =
			call_tool(&s, "update_tauri_settings", &json!({"windows": "sideways"})).unwrap_err();
		assert_eq!(err.payload().kind, ErrorKind::InvalidInput);

		fs::write(
			d.path().join("src-tauri/tauri.conf.json"),
			fs::read_to_string(d.path().join("src-tauri/tauri.conf.json"))
				.unwrap()
				.replace("Plume Notes", "Plume Notes Pro"),
		)
		.unwrap();
		let r = call_tool(&s, "import_tauri", &json!({})).unwrap();
		assert_eq!(r["first_import"], false);
		assert!(
			r["changes"]
				.as_array()
				.unwrap()
				.iter()
				.any(|c| c["field"] == "product_name")
		);
		assert!(read(&root, "tauri.json").contains("Plume Notes Pro"));
	}

	#[test]
	fn the_dry_run_shows_the_webview2_step_and_both_modes_plans() {
		use li_model::Operation;
		let (d, _s, root) = open_example("tauri-notes");
		let s = Studio::new(Box::new(crate::EngineBackend));
		s.open(d.path()).unwrap();
		let target = "windows-x86_64:canvas";
		let native = s.run_preview(target).unwrap();
		assert!(native.plan.ops.iter().any(|o| matches!(
			o,
			Operation::PrereqWebview2 {
				mode: Webview2Mode::EmbedBootstrapper,
				..
			}
		)));
		assert!(
			native
				.plan
				.ops
				.iter()
				.any(|o| matches!(o, Operation::RegistrySet { name, .. } if name == "DisplayName"))
		);
		assert!(
			!native
				.plan
				.ops
				.iter()
				.any(|o| matches!(o, Operation::TauriWrap { .. }))
		);
		s.tauri_set(
			TauriSettingsChange {
				windows: Some(InstallMode::Wrap),
				..Default::default()
			},
			"Wrap",
			Source::Ui,
		)
		.unwrap();
		let wrap = s.run_preview(target).unwrap();
		assert!(
			wrap.plan
				.ops
				.iter()
				.any(|o| matches!(o, Operation::TauriWrap { .. }))
		);
		assert!(
			!wrap
				.plan
				.ops
				.iter()
				.any(|o| matches!(o, Operation::RegistrySet { .. }))
		);
		assert!(root.join("installer.toml").is_file());
		// The checks the Studio runs include the Tauri findings.
		assert!(
			s.check_project()
				.unwrap()
				.iter()
				.all(|p| p.severity != Severity::Error),
			"{:?}",
			s.check_project().unwrap()
		);
	}
}
