// Tauri app support: the import, drift, the WebView2 step, both Windows modes and update ownership,
// against the Plume Notes example and a simulated Windows machine.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use li_engine::tauri::conf::read_snapshot;
use li_engine::tauri::edit::set_key;
use li_engine::tauri::import::{self, DriftKind};
use li_engine::tauri::nsis::SimNsis;
use li_engine::tauri::registry::{publisher_of, registry_writes};
use li_engine::tauri::webview2;
use li_engine::{
	DirPayload, EngineError, InstallRequest, Machine, Project, RunOptions, SimMachine, install,
	preview, read_receipt, repair, uninstall, update,
};
use li_model::tauri::{InstallMode, UpdateMode, Webview2Mode};
use li_model::{Operation, Receipt, Scope};

fn example_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/tauri-notes")
}

fn copy_tree(from: &Path, to: &Path) {
	std::fs::create_dir_all(to).unwrap();
	for entry in std::fs::read_dir(from).unwrap() {
		let entry = entry.unwrap();
		let dest = to.join(entry.file_name());
		if entry.file_type().unwrap().is_dir() {
			copy_tree(&entry.path(), &dest);
		} else {
			std::fs::copy(entry.path(), dest).unwrap();
		}
	}
}

/// A scratch copy of the example: `installer/` and `src-tauri/`.
struct Scratch {
	dir: tempfile::TempDir,
}

impl Scratch {
	fn new() -> Self {
		let dir = tempfile::tempdir().unwrap();
		copy_tree(&example_root(), dir.path());
		Scratch { dir }
	}

	fn installer(&self) -> PathBuf {
		self.dir.path().join("installer")
	}

	fn conf(&self) -> PathBuf {
		self.dir.path().join("src-tauri/tauri.conf.json")
	}

	fn edit_installer(&self, f: impl FnOnce(String) -> String) {
		let p = self.installer().join("installer.toml");
		let text = std::fs::read_to_string(&p).unwrap();
		std::fs::write(p, f(text)).unwrap();
	}

	fn edit_conf(&self, f: impl FnOnce(&mut serde_json::Value)) {
		let mut v: serde_json::Value =
			serde_json::from_str(&std::fs::read_to_string(self.conf()).unwrap()).unwrap();
		f(&mut v);
		std::fs::write(self.conf(), serde_json::to_string_pretty(&v).unwrap()).unwrap();
	}

	fn load(&self) -> Project {
		Project::load_installer_dir(&self.installer()).unwrap()
	}
}

fn windows_machine() -> SimMachine {
	SimMachine::new().with_os("windows")
}

/// A machine that already has WebView2 and runs programs elevated, which a `both` setup needs.
fn admin_machine() -> SimMachine {
	windows_machine().with_webview2("131.0.2903.51").elevated()
}

fn request(scope: Scope) -> InstallRequest {
	InstallRequest {
		target: "windows-x86_64".into(),
		install_dir: "/sim/home/AppData/Local/Plume Notes".into(),
		scope,
		components: None,
		version: None,
		feed: None,
		channel: None,
	}
}

fn plan(project: &Project, scope: Scope, m: &SimMachine) -> Vec<Operation> {
	let payload = DirPayload::new(&project.dir).unwrap();
	preview(project, &payload, &request(scope), m).unwrap().ops
}

fn run_install(
	project: &Project,
	m: &mut SimMachine,
	scope: Scope,
) -> Result<Receipt, EngineError> {
	let payload = DirPayload::new(&project.dir).unwrap();
	install(
		m,
		project,
		&payload,
		&request(scope),
		&mut RunOptions::default(),
	)
}

fn key_prefix(op: &Operation, prefix: &str) -> bool {
	matches!(op, Operation::RegistrySet { key, .. } if key.starts_with(prefix))
}

// Parsing.

#[test]
fn the_config_and_its_windows_overlay_become_a_snapshot() {
	let snap = read_snapshot(&example_root().join("src-tauri/tauri.conf.json")).unwrap();
	assert_eq!(snap.product_name, "Plume Notes");
	assert_eq!(snap.identifier, "ca.liminalhq.plumenotes");
	assert_eq!(snap.app_version, "0.4.2");
	assert_eq!(snap.binary_stem(), "plume-notes");
	assert_eq!(snap.targets, vec!["nsis", "deb", "app"]);
	assert_eq!(snap.deep_link_schemes, vec!["plume"]);
	assert_eq!(snap.file_associations[0].ext, vec!["plume"]);
	assert_eq!(snap.external_bin, vec!["binaries/plume-sync"]);
	// The base says downloadBootstrapper; the Windows overlay wins on Windows.
	assert_eq!(snap.windows.webview2.mode, Webview2Mode::EmbedBootstrapper);
	assert_eq!(
		snap.windows.webview2.min_version.as_deref(),
		Some("110.0.1587.40")
	);
	assert_eq!(
		snap.windows.install_mode,
		li_model::tauri::NsisInstallMode::Both
	);
	let u = snap.updater.as_ref().unwrap();
	assert_eq!(u.endpoints, vec!["https://example.org/plume/latest.json"]);
	assert!(u.create_artifacts);
	assert_eq!(
		snap.sources
			.iter()
			.map(|s| s.path.as_str())
			.collect::<Vec<_>>(),
		vec!["tauri.conf.json", "tauri.windows.conf.json"]
	);
}

#[test]
fn a_config_without_a_product_name_is_refused() {
	let s = Scratch::new();
	s.edit_conf(|v| {
		v.as_object_mut().unwrap().remove("productName");
	});
	let err = read_snapshot(&s.conf()).unwrap_err().to_string();
	assert!(err.contains("productName"), "{err}");
}

// Import and drift.

#[test]
fn the_committed_example_is_what_an_import_writes() {
	let s = Scratch::new();
	let before_snapshot = std::fs::read_to_string(s.installer().join("tauri.json")).unwrap();
	let before_components = std::fs::read_to_string(s.installer().join("components.toml")).unwrap();
	let report = import::import(&s.installer(), &s.conf()).unwrap();
	assert!(!report.first_import);
	assert!(report.changes.is_empty(), "{:?}", report.changes);
	assert_eq!(
		std::fs::read_to_string(s.installer().join("tauri.json")).unwrap(),
		before_snapshot
	);
	assert_eq!(
		std::fs::read_to_string(s.installer().join("components.toml")).unwrap(),
		before_components
	);
}

#[test]
fn import_fills_the_product_and_keeps_what_the_project_overrides() {
	let s = Scratch::new();
	s.edit_installer(|t| {
		let t = set_key(&t, "product", "name", Some("\"Plume (house edition)\""));
		let t = set_key(&t, "product", "id", Some("\"old.id\""));
		set_key(&t, "tauri", "override", Some("[\"name\"]"))
	});
	let report = import::import(&s.installer(), &s.conf()).unwrap();
	assert_eq!(report.filled, vec!["id"]);
	assert_eq!(report.kept, vec!["name"]);
	let p = s.load();
	assert_eq!(p.installer.product.id, "ca.liminalhq.plumenotes");
	assert_eq!(p.installer.product.name, "Plume (house edition)");
}

#[test]
fn a_version_read_from_the_build_stays_that_way() {
	let s = Scratch::new();
	s.edit_installer(|t| set_key(&t, "product", "version", Some("{ from = \"git-tag\" }")));
	let report = import::import(&s.installer(), &s.conf()).unwrap();
	assert!(report.kept.contains(&"version".to_owned()));
	assert!(!report.filled.contains(&"version".to_owned()));
	assert!(matches!(
		s.load().installer.product.version,
		li_model::project::VersionSource::From { .. }
	));
}

#[test]
fn import_needs_a_project_to_fill() {
	let dir = tempfile::tempdir().unwrap();
	let err = import::import(
		dir.path(),
		&example_root().join("src-tauri/tauri.conf.json"),
	)
	.unwrap_err()
	.to_string();
	assert!(err.contains("liminst init"), "{err}");
}

#[test]
fn a_current_import_has_no_drift() {
	let s = Scratch::new();
	let report = import::drift(&s.load()).unwrap();
	assert!(!report.is_stale() && !report.config_missing && !report.no_snapshot);
	assert!(
		li_engine::check(&s.load())
			.iter()
			.all(|d| !d.code.starts_with("tauri-"))
	);
}

#[test]
fn a_changed_config_is_drift_that_check_reports_until_it_is_imported_again() {
	let s = Scratch::new();
	s.edit_conf(|v| v["version"] = "0.5.0".into());
	let p = s.load();
	let report = import::drift(&p).unwrap();
	assert_eq!(report.sources.len(), 1);
	assert_eq!(report.sources[0].file, "tauri.conf.json");
	assert_eq!(report.sources[0].kind, DriftKind::Changed);
	let found = li_engine::check(&p);
	let d = found.iter().find(|d| d.code == "tauri-drift").unwrap();
	assert_eq!(d.severity, li_engine::Severity::Error);
	assert!(
		d.message
			.contains("tauri.conf.json changed since the import")
	);
	// What a re-import would change, before it is accepted.
	let pending = import::pending_changes(&p).unwrap();
	assert_eq!(pending.len(), 1);
	assert_eq!(pending[0].field, "app_version");
	assert_eq!(pending[0].from.as_deref(), Some("0.4.2"));
	assert_eq!(pending[0].to.as_deref(), Some("0.5.0"));

	let report = import::reimport(&p).unwrap();
	assert_eq!(report.changes.len(), 1);
	assert_eq!(report.filled, vec!["version"]);
	let p = s.load();
	assert_eq!(
		p.installer.product.version,
		li_model::project::VersionSource::Literal("0.5.0".into())
	);
	assert!(!import::drift(&p).unwrap().is_stale());
}

#[test]
fn a_new_or_removed_overlay_is_drift_too() {
	let s = Scratch::new();
	std::fs::write(
		s.installer().join("../src-tauri/tauri.linux.conf.json"),
		"{\"bundle\": {}}",
	)
	.unwrap();
	let r = import::drift(&s.load()).unwrap();
	assert_eq!(
		r.sources
			.iter()
			.map(|d| (d.file.as_str(), d.kind))
			.collect::<Vec<_>>(),
		vec![("tauri.linux.conf.json", DriftKind::Added)]
	);
	std::fs::remove_file(s.installer().join("../src-tauri/tauri.windows.conf.json")).unwrap();
	let r = import::drift(&s.load()).unwrap();
	assert!(
		r.sources
			.iter()
			.any(|d| d.file == "tauri.windows.conf.json" && d.kind == DriftKind::Removed)
	);
}

#[test]
fn a_missing_config_or_snapshot_is_an_error() {
	let s = Scratch::new();
	std::fs::remove_file(s.installer().join("tauri.json")).unwrap();
	let codes: Vec<_> = li_engine::check(&s.load()).iter().map(|d| d.code).collect();
	assert!(codes.contains(&"tauri-not-imported"), "{codes:?}");
	std::fs::remove_file(s.conf()).unwrap();
	let codes: Vec<_> = li_engine::check(&s.load()).iter().map(|d| d.code).collect();
	assert!(codes.contains(&"tauri-config-missing"), "{codes:?}");
}

// Plans.

#[test]
fn the_native_plan_runs_webview2_then_copies_and_writes_what_the_installer_would() {
	let s = Scratch::new();
	let ops = plan(&s.load(), Scope::User, &windows_machine());
	let at = |f: &dyn Fn(&Operation) -> bool| ops.iter().position(f).unwrap();
	let prereq = at(&|o| matches!(o, Operation::PrereqWebview2 { .. }));
	let first_copy =
		at(&|o| matches!(o, Operation::Copy { to, .. } if to == "{install}/plume-notes.exe"));
	assert!(prereq < first_copy);
	let Operation::PrereqWebview2 {
		mode,
		min_version,
		installer,
		found,
		..
	} = &ops[prereq]
	else {
		unreachable!()
	};
	assert_eq!(*mode, Webview2Mode::EmbedBootstrapper);
	assert_eq!(min_version.as_deref(), Some("110.0.1587.40"));
	assert!(
		installer
			.as_deref()
			.unwrap()
			.ends_with("MicrosoftEdgeWebview2Setup.exe")
	);
	assert_eq!(*found, None);
	// The bootstrapper is carried in the payload and staged before the step.
	assert!(ops.iter().any(|o| matches!(o, Operation::Copy { from, .. } if from == "dist/webview2/MicrosoftEdgeWebview2Setup.exe")));
	// The registrations are registry values, not the generic file-type operations.
	assert!(
		!ops.iter()
			.any(|o| matches!(o, Operation::FileType { .. } | Operation::UrlScheme { .. }))
	);
	assert!(
		ops.iter()
			.any(|o| key_prefix(o, r"HKCU\Software\Classes\plume\shell\open\command"))
	);
	assert!(ops.iter().any(|o| key_prefix(
		o,
		r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Plume Notes"
	)));
	assert!(!ops.iter().any(|o| matches!(o, Operation::TauriWrap { .. })));
	// `both` also writes the choice, as the setup does.
	assert!(
		ops.iter()
			.any(|o| matches!(o, Operation::RegistrySet { name, .. } if name == "CurrentUser"))
	);
	assert!(matches!(ops.last(), Some(Operation::ReceiptWrite { .. })));
}

#[test]
fn a_machine_scope_install_writes_hklm() {
	let s = Scratch::new();
	let ops = plan(&s.load(), Scope::Machine, &windows_machine());
	assert!(ops.iter().any(|o| key_prefix(o, "HKLM\\")));
	assert!(!ops.iter().any(|o| key_prefix(o, "HKCU\\")));
	assert!(
		ops.iter()
			.any(|o| matches!(o, Operation::RegistrySet { name, .. } if name == "AllUsers"))
	);
}

#[test]
fn the_wrap_plan_carries_the_tauri_setup_and_leaves_the_rest_to_it() {
	let s = Scratch::new();
	s.edit_installer(|t| set_key(&t, "tauri", "windows", Some("\"wrap\"")));
	let p = s.load();
	assert_eq!(
		p.installer.tauri.as_ref().unwrap().windows,
		InstallMode::Wrap
	);
	let ops = plan(&p, Scope::User, &windows_machine());
	let wrap = ops.iter().find_map(|o| match o {
		Operation::TauriWrap {
			installer,
			product_name,
			hive,
			nsis_mode,
			external,
			..
		} => Some((
			installer.clone(),
			product_name.clone(),
			hive.clone(),
			nsis_mode.clone(),
			external.clone(),
		)),
		_ => None,
	});
	let (installer, name, hive, mode, external) = wrap.unwrap();
	assert!(installer.ends_with("Plume Notes-setup.exe"));
	assert_eq!(
		(name.as_str(), hive.as_str(), mode.as_str()),
		("Plume Notes", "HKCU", "both")
	);
	assert!(external.is_none());
	// Tauri's setup makes the files, shortcuts and registrations.
	assert!(!ops.iter().any(|o| matches!(
		o,
		Operation::Shortcut { .. } | Operation::RegistrySet { .. } | Operation::FileType { .. }
	)));
	assert!(
		!ops.iter()
			.any(|o| matches!(o, Operation::Copy { to, .. } if to.starts_with("{install}")))
	);
	// The setup comes after WebView2 so the runtime is there first.
	let prereq = ops
		.iter()
		.position(|o| matches!(o, Operation::PrereqWebview2 { .. }))
		.unwrap();
	let wrap_at = ops
		.iter()
		.position(|o| matches!(o, Operation::TauriWrap { .. }))
		.unwrap();
	assert!(prereq < wrap_at);
	// The other targets are untouched by `wrap`.
	let payload = DirPayload::new(&p.dir).unwrap();
	let linux = InstallRequest {
		target: "linux-x86_64".into(),
		install_dir: "/sim/opt/plume".into(),
		..request(Scope::User)
	};
	let lops = preview(&p, &payload, &linux, &SimMachine::new())
		.unwrap()
		.ops;
	assert!(lops.iter().any(|o| matches!(o, Operation::FileType { .. })));
	assert!(!lops.iter().any(|o| matches!(
		o,
		Operation::PrereqWebview2 { .. } | Operation::TauriWrap { .. }
	)));
}

#[test]
fn per_machine_and_per_user_tauri_builds_pick_the_hive_for_wrap() {
	let s = Scratch::new();
	s.edit_installer(|t| set_key(&t, "tauri", "windows", Some("\"wrap\"")));
	s.edit_conf(|v| {
		v["bundle"]["windows"]["nsis"]["installMode"] = "perMachine".into();
	});
	import::import(&s.installer(), &s.conf()).unwrap();
	let ops = plan(&s.load(), Scope::User, &windows_machine());
	assert!(ops.iter().any(|o| matches!(o, Operation::TauriWrap { hive, nsis_mode, .. } if hive == "HKLM" && nsis_mode == "perMachine")));
}

#[test]
fn a_dry_run_on_a_machine_with_the_runtime_finds_it() {
	let s = Scratch::new();
	let m = windows_machine().with_webview2("131.0.2903.51");
	let ops = plan(&s.load(), Scope::User, &m);
	let found = ops.iter().find_map(|o| match o {
		Operation::PrereqWebview2 { found, ran, .. } => Some((found.clone(), *ran)),
		_ => None,
	});
	assert_eq!(found, Some((Some("131.0.2903.51".into()), false)));
	// An older runtime than the minimum is not "found" for the plan's purposes.
	let old = windows_machine().with_webview2("99.0.0.0");
	let ops = plan(&s.load(), Scope::User, &old);
	assert!(
		ops.iter()
			.any(|o| matches!(o, Operation::PrereqWebview2 { found: None, .. }))
	);
}

#[test]
fn skip_plans_no_step_and_check_warns() {
	let s = Scratch::new();
	s.edit_installer(|t| set_key(&t, "tauri.webview2", "mode", Some("\"skip\"")));
	let p = s.load();
	let ops = plan(&p, Scope::User, &windows_machine());
	assert!(
		!ops.iter()
			.any(|o| matches!(o, Operation::PrereqWebview2 { .. }))
	);
	assert!(
		li_engine::check(&p)
			.iter()
			.any(|d| d.code == "tauri-webview2-skip")
	);
}

#[test]
fn every_mode_stages_what_it_needs() {
	let s = Scratch::new();
	for (mode, copies, source) in [
		("downloadBootstrapper", false, true),
		("embedBootstrapper", true, false),
		("offlineInstaller", true, false),
	] {
		s.edit_installer(|t| set_key(&t, "tauri.webview2", "mode", Some(&format!("\"{mode}\""))));
		let ops = plan(&s.load(), Scope::User, &windows_machine());
		let step = ops
			.iter()
			.find_map(|o| match o {
				Operation::PrereqWebview2 {
					source, installer, ..
				} => Some((source.clone(), installer.clone())),
				_ => None,
			})
			.unwrap();
		assert_eq!(step.0.is_some(), source, "{mode}");
		assert!(step.1.is_some(), "{mode}");
		let staged = ops.iter().any(
			|o| matches!(o, Operation::Copy { from, .. } if from.starts_with("dist/webview2/")),
		);
		assert_eq!(staged, copies, "{mode}");
		if mode == "downloadBootstrapper" {
			assert_eq!(
				step.0.as_deref(),
				Some("https://go.microsoft.com/fwlink/p/?LinkId=2124703")
			);
		}
	}
	assert_eq!(
		li_model::tauri::WEBVIEW2_OFFLINE_URL,
		"https://go.microsoft.com/fwlink/?linkid=2124701"
	);
}

#[test]
fn a_fixed_runtime_is_copied_with_the_app() {
	let s = Scratch::new();
	let folder = s
		.installer()
		.join("dist/windows-x86_64/Microsoft.WebView2.FixedVersionRuntime.131");
	std::fs::create_dir_all(&folder).unwrap();
	std::fs::write(folder.join("msedgewebview2.exe"), "runtime").unwrap();
	s.edit_installer(|t| {
		let t = set_key(&t, "tauri.webview2", "mode", Some("\"fixedRuntime\""));
		set_key(
			&t,
			"tauri.webview2",
			"path",
			Some("\"./Microsoft.WebView2.FixedVersionRuntime.131\""),
		)
	});
	let p = s.load();
	let ops = plan(&p, Scope::User, &windows_machine());
	assert!(ops.iter().any(|o| matches!(o, Operation::Copy { to, .. } if to == "{install}/Microsoft.WebView2.FixedVersionRuntime.131/msedgewebview2.exe")));
	let mut m = windows_machine();
	let receipt = run_install(&p, &mut m, Scope::User).unwrap();
	assert!(receipt.operations.iter().any(|o| matches!(o, Operation::PrereqWebview2 { path: Some(p), ran: false, .. } if p.ends_with("FixedVersionRuntime.131"))));
	// No installer ran.
	assert!(m.runs().is_empty());
}

// Detection and the step, run against the simulated machine.

#[test]
fn a_present_runtime_means_no_installer_runs() {
	let s = Scratch::new();
	let mut m = windows_machine().with_webview2("131.0.2903.51");
	let receipt = run_install(&s.load(), &mut m, Scope::User).unwrap();
	assert!(m.runs().is_empty());
	assert!(receipt.operations.iter().any(|o| matches!(o, Operation::PrereqWebview2 { found: Some(v), ran: false, .. } if v == "131.0.2903.51")));
}

#[test]
fn a_missing_runtime_is_installed_silently_per_user_and_recorded() {
	let s = Scratch::new();
	let mut m = windows_machine();
	let receipt = run_install(&s.load(), &mut m, Scope::User).unwrap();
	let runs = m.runs();
	assert_eq!(runs.len(), 1);
	assert!(runs[0].0.ends_with("MicrosoftEdgeWebview2Setup.exe"));
	assert_eq!(runs[0].1, vec!["/silent", "/install"]);
	assert_eq!(webview2::detect(&m).map(|d| d.hive), Some("HKCU"));
	assert!(receipt.operations.iter().any(|o| matches!(
		o,
		Operation::PrereqWebview2 {
			ran: true,
			found: None,
			..
		}
	)));
}

#[test]
fn an_elevated_install_puts_the_runtime_per_machine() {
	let s = Scratch::new();
	let mut m = windows_machine().elevated();
	run_install(&s.load(), &mut m, Scope::Machine).unwrap();
	assert_eq!(webview2::detect(&m).map(|d| d.hive), Some("HKLM"));
}

#[test]
fn the_download_mode_fetches_the_bootstrapper_with_curl() {
	let s = Scratch::new();
	s.edit_installer(|t| {
		set_key(
			&t,
			"tauri.webview2",
			"mode",
			Some("\"downloadBootstrapper\""),
		)
	});
	let mut m = windows_machine();
	run_install(&s.load(), &mut m, Scope::User).unwrap();
	let runs = m.runs();
	assert_eq!(runs.len(), 2);
	assert_eq!(runs[0].0, Path::new("curl"));
	assert!(
		runs[0]
			.1
			.contains(&"https://go.microsoft.com/fwlink/p/?LinkId=2124703".to_owned())
	);
	assert!(runs[1].0.ends_with("MicrosoftEdgeWebview2Setup.exe"));
	// The download was scratch: nothing of it is left in temp.
	assert!(m.snapshot().keys().all(|p| {
		!p.to_string_lossy()
			.contains("MicrosoftEdgeWebview2Setup.exe")
	}));
}

#[test]
fn no_network_fails_the_step_and_rolls_everything_back() {
	let s = Scratch::new();
	s.edit_installer(|t| {
		set_key(
			&t,
			"tauri.webview2",
			"mode",
			Some("\"downloadBootstrapper\""),
		)
	});
	let mut m = windows_machine().offline();
	let before = (m.snapshot(), m.registry());
	let err = run_install(&s.load(), &mut m, Scope::User).unwrap_err();
	assert!(matches!(err, EngineError::RolledBack(_)), "{err}");
	assert!(err.to_string().contains("WebView2"), "{err}");
	assert_eq!((m.snapshot(), m.registry()), before);
}

#[test]
fn an_older_runtime_than_the_minimum_is_installed_again() {
	let s = Scratch::new();
	let mut m = windows_machine().with_webview2("99.0.0.0");
	let receipt = run_install(&s.load(), &mut m, Scope::User).unwrap();
	assert_eq!(m.runs().len(), 1);
	assert!(receipt.operations.iter().any(
		|o| matches!(o, Operation::PrereqWebview2 { ran: true, found: Some(v), .. } if v == "99.0.0.0")
	));
}

#[test]
fn the_offline_installer_needs_no_network() {
	let s = Scratch::new();
	s.edit_installer(|t| set_key(&t, "tauri.webview2", "mode", Some("\"offlineInstaller\"")));
	let mut m = windows_machine().offline();
	run_install(&s.load(), &mut m, Scope::User).unwrap();
	assert!(webview2::detect(&m).is_some());
}

// Native Windows install.

#[test]
fn a_native_install_writes_the_values_and_uninstall_removes_every_one() {
	let s = Scratch::new();
	let p = s.load();
	let mut m = windows_machine().with_webview2("131.0.2903.51");
	let before = m.snapshot();
	let registry_before = m.registry();
	let receipt = run_install(&p, &mut m, Scope::User).unwrap();
	let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Plume Notes";
	assert_eq!(
		m.registry_value(key, "DisplayName").as_deref(),
		Some("Plume Notes")
	);
	assert_eq!(
		m.registry_value(key, "DisplayVersion").as_deref(),
		Some("0.4.2")
	);
	assert_eq!(
		m.registry_value(key, "Publisher").as_deref(),
		Some("Liminal HQ")
	);
	// Placeholders are resolved in what the machine holds.
	assert_eq!(
		m.registry_value(key, "InstallLocation").as_deref(),
		Some("\"/sim/home/AppData/Local/Plume Notes\"")
	);
	assert_eq!(
		m.registry_value(r"HKCU\Software\Classes\plume\shell\open\command", "")
			.as_deref(),
		Some("\"/sim/home/AppData/Local/Plume Notes\\plume-notes.exe\" \"%1\"")
	);
	assert_eq!(
		m.registry_value(r"HKCU\Software\Liminal HQ\Plume Notes", "")
			.as_deref(),
		Some("/sim/home/AppData/Local/Plume Notes")
	);
	// The receipt keeps the placeholder form.
	assert!(receipt.operations.iter().any(|o| matches!(o, Operation::RegistrySet { name, value, .. } if name == "InstallLocation" && value == "\"{install}\"")));

	uninstall(
		&mut m,
		Some(&p),
		&request(Scope::User).install_dir,
		&mut RunOptions::default(),
	)
	.unwrap();
	assert_eq!(m.snapshot(), before);
	assert_eq!(m.registry(), registry_before);
}

#[test]
fn repair_restores_a_deleted_registry_value() {
	let s = Scratch::new();
	let p = s.load();
	let mut m = windows_machine().with_webview2("131.0.2903.51");
	run_install(&p, &mut m, Scope::User).unwrap();
	let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Plume Notes";
	m.remove_registry_value(key, "DisplayVersion");
	let payload = DirPayload::new(&p.dir).unwrap();
	let report = repair(
		&mut m,
		&payload,
		&request(Scope::User).install_dir,
		&mut RunOptions::default(),
	)
	.unwrap();
	assert_eq!(report.restored.len(), 1);
	assert_eq!(
		m.registry_value(key, "DisplayVersion").as_deref(),
		Some("0.4.2")
	);
}

// Wrap on Windows.

fn wrap_scratch() -> (Scratch, Project) {
	let s = Scratch::new();
	s.edit_installer(|t| set_key(&t, "tauri", "windows", Some("\"wrap\"")));
	let p = s.load();
	(s, p)
}

fn nsis(p: &Project, m: &SimMachine) {
	SimNsis::new(p.tauri_snapshot.clone().unwrap()).install_on(m);
}

#[test]
fn a_wrapped_install_drives_the_setup_and_records_its_uninstall_key() {
	let (_s, p) = wrap_scratch();
	let mut m = admin_machine();
	nsis(&p, &m);
	let dir = request(Scope::Machine).install_dir;
	let receipt = run_install(&p, &mut m, Scope::Machine).unwrap();

	let runs = m.runs();
	assert_eq!(runs.len(), 1);
	assert!(runs[0].0.ends_with("Plume Notes-setup.exe"));
	// `both` always names the scope, and /D= comes last and unquoted.
	assert_eq!(
		runs[0].1,
		vec![
			"/S".to_owned(),
			"/allusers".to_owned(),
			format!("/D={}", dir.display())
		]
	);

	let ext = receipt
		.external()
		.expect("the receipt records the setup's entry");
	assert_eq!(ext.kind, "nsis");
	assert_eq!(ext.hive, "HKLM");
	assert_eq!(
		ext.uninstall_key,
		r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Plume Notes"
	);
	assert_eq!(ext.version, "0.4.2");
	assert_eq!(ext.main_binary.as_deref(), Some("plume-notes.exe"));
	assert_eq!(Path::new(&ext.uninstaller), dir.join("uninstall.exe"));
	assert_eq!(Path::new(&ext.install_location), dir);
	assert_eq!(
		ext.publisher_key.as_deref(),
		Some(r"Software\Liminal HQ\Plume Notes")
	);
	assert!(m.snapshot().contains_key(&dir.join("plume-notes.exe")));
	assert!(
		m.snapshot()
			.contains_key(&dir.join(".liminal/receipt.json"))
	);
	assert_eq!(read_receipt(&m, &dir).unwrap(), receipt);
}

#[test]
fn a_per_user_choice_passes_currentuser() {
	let (_s, p) = wrap_scratch();
	let mut m = admin_machine();
	nsis(&p, &m);
	let receipt = run_install(&p, &mut m, Scope::User).unwrap();
	assert!(m.runs()[0].1.contains(&"/currentuser".to_owned()));
	assert_eq!(receipt.external().unwrap().hive, "HKCU");
}

#[test]
fn a_wrapped_uninstall_runs_the_products_own_uninstaller_and_cleans_its_residue() {
	let (_s, p) = wrap_scratch();
	let mut m = admin_machine();
	nsis(&p, &m);
	let before = m.snapshot();
	let dir = request(Scope::User).install_dir;
	run_install(&p, &mut m, Scope::User).unwrap();
	uninstall(&mut m, Some(&p), &dir, &mut RunOptions::default()).unwrap();
	let runs = m.runs();
	let last = runs.last().unwrap();
	assert!(last.0.ends_with("uninstall.exe"));
	assert_eq!(
		last.1,
		vec!["/S".to_owned(), format!("_?={}", dir.display())]
	);
	// Neither uninstall.exe, the install directory nor the product key is left.
	assert_eq!(m.snapshot(), before);
	assert!(
		m.registry_value(r"HKCU\Software\Liminal HQ\Plume Notes", "")
			.is_none()
	);
	assert!(
		m.registry_value(
			r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Plume Notes",
			"DisplayName"
		)
		.is_none()
	);
}

#[test]
fn exit_740_is_reported_as_needing_elevation_and_rolls_back() {
	let (_s, p) = wrap_scratch();
	let mut m = windows_machine().with_webview2("131.0.2903.51");
	nsis(&p, &m);
	let before = (m.snapshot(), m.registry());
	let err = run_install(&p, &mut m, Scope::Machine).unwrap_err();
	assert!(
		err.to_string().contains("needs elevation (exit 740)"),
		"{err}"
	);
	assert_eq!((m.snapshot(), m.registry()), before);
}

#[test]
fn exit_2_is_an_abort_and_other_codes_are_failures() {
	let (_s, p) = wrap_scratch();
	let mut m = admin_machine();
	m.on_run(|_, _, _| Some(2));
	let err = run_install(&p, &mut m, Scope::User).unwrap_err();
	assert!(err.to_string().contains("(exit 2)"), "{err}");
	let mut m = admin_machine();
	m.on_run(|_, _, _| Some(1603));
	let err = run_install(&p, &mut m, Scope::User).unwrap_err();
	assert!(err.to_string().contains("exit 1603"), "{err}");
}

#[test]
fn a_setup_that_writes_no_uninstall_entry_is_an_error() {
	let (_s, p) = wrap_scratch();
	let mut m = admin_machine();
	let err = run_install(&p, &mut m, Scope::User).unwrap_err();
	assert!(
		err.to_string().contains("no Add or Remove Programs entry"),
		"{err}"
	);
}

#[test]
fn a_failure_after_the_setup_ran_uninstalls_it() {
	let (_s, p) = wrap_scratch();
	let payload = DirPayload::new(&p.dir).unwrap();
	let run = |fault: Option<usize>| {
		let m = admin_machine();
		nsis(&p, &m);
		let before = m.snapshot();
		let mut fm = match fault {
			Some(n) => li_engine::FaultMachine::failing_at(m.clone(), n),
			None => li_engine::FaultMachine::counting(m.clone()),
		};
		let r = install(
			&mut fm,
			&p,
			&payload,
			&request(Scope::User),
			&mut RunOptions::default(),
		);
		(m, before, fm.calls(), r)
	};
	let (_, _, total, ok) = run(None);
	ok.unwrap();
	// The last write is the receipt's, after the setup ran: failing it rolls the setup back.
	let (m, before, _, r) = run(Some(total - 1));
	assert!(matches!(r, Err(EngineError::RolledBack(_))), "{r:?}");
	assert!(m.runs().iter().any(|(p, _)| p.ends_with("uninstall.exe")));
	assert_eq!(m.snapshot(), before);
}

#[test]
fn wrap_reads_the_setup_from_a_named_path_and_check_notices_when_it_is_missing() {
	let (s, _) = wrap_scratch();
	s.edit_installer(|t| {
		set_key(
			&t,
			"tauri.wrap",
			"installer",
			Some("\"dist/{target}/nowhere.exe\""),
		)
	});
	let found = li_engine::check(&s.load());
	assert!(found.iter().any(|d| d.code == "tauri-wrap-installer"));
}

#[test]
fn an_update_runs_the_new_setup_in_place_as_the_tauri_updater_does() {
	let (s, p) = wrap_scratch();
	let mut m = admin_machine();
	nsis(&p, &m);
	run_install(&p, &mut m, Scope::User).unwrap();
	// The same payload again changes nothing, so the setup does not run twice.
	run_install(&p, &mut m, Scope::User).unwrap();
	assert_eq!(m.runs().len(), 1, "{:?}", m.runs());
	// A new build of the setup is an update: run in place with /UPDATE.
	std::fs::write(
		s.installer()
			.join("dist/windows-x86_64/Plume Notes-setup.exe"),
		"a newer setup",
	)
	.unwrap();
	run_install(&s.load(), &mut m, Scope::User).unwrap();
	let runs = m.runs();
	assert_eq!(runs.len(), 2);
	assert!(!runs[0].1.contains(&"/UPDATE".to_owned()));
	assert_eq!(runs[1].1[..2], ["/S".to_owned(), "/UPDATE".to_owned()]);
	assert!(runs[1].1.last().unwrap().starts_with("/D="));
}

#[test]
fn a_blocked_downgrade_is_exit_2() {
	let (s, _) = wrap_scratch();
	s.edit_conf(|v| v["bundle"]["windows"]["allowDowngrades"] = false.into());
	import::import(&s.installer(), &s.conf()).unwrap();
	let p = s.load();
	let mut m = admin_machine();
	nsis(&p, &m);
	// A newer version is already there.
	m.set_registry_value(
		r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Plume Notes",
		"DisplayVersion",
		"9.0.0",
	);
	m.set_registry_value(
		r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Plume Notes",
		"DisplayName",
		"Plume Notes",
	);
	let err = run_install(&p, &mut m, Scope::User).unwrap_err();
	assert!(err.to_string().contains("(exit 2)"), "{err}");
}

// Native and wrap agree.

#[test]
fn the_native_registry_matches_what_the_setup_writes() {
	let (_s, p) = wrap_scratch();
	let snap = p.tauri_snapshot.clone().unwrap();
	let mut wrapped = admin_machine();
	nsis(&p, &wrapped);
	run_install(&p, &mut wrapped, Scope::User).unwrap();

	let native_scratch = Scratch::new();
	let native_project = native_scratch.load();
	let mut native = windows_machine().with_webview2("131.0.2903.51");
	run_install(&native_project, &mut native, Scope::User).unwrap();

	// Same install directory in both, so the values compare equal. The two differ only where
	// the uninstall command and the size are, which are ours and Tauri's own.
	let dir = request(Scope::User)
		.install_dir
		.to_string_lossy()
		.into_owned();
	for r in registry_writes(&snap, "HKCU", &dir, "", 0) {
		if r.name == "UninstallString" || r.name == "EstimatedSize" {
			continue;
		}
		assert_eq!(
			native.registry_value(&r.key, &r.name).as_deref(),
			wrapped.registry_value(&r.key, &r.name).as_deref(),
			"{} {}",
			r.key,
			r.name
		);
		assert!(
			native.registry_value(&r.key, &r.name).is_some(),
			"{} {}",
			r.key,
			r.name
		);
	}
	assert_eq!(publisher_of(&snap), "Liminal HQ");
}

// Updates.

#[test]
fn the_installer_owns_updates_by_default_and_a_tauri_only_product_refuses_them() {
	let s = Scratch::new();
	let p = s.load();
	assert_eq!(p.installer.updates.as_ref().unwrap().mode, UpdateMode::Both);
	let mut m = windows_machine().with_webview2("131.0.2903.51");
	let receipt = run_install(&p, &mut m, Scope::User).unwrap();
	assert_eq!(receipt.updates.as_ref().unwrap().mode, UpdateMode::Both);
	assert_eq!(
		receipt.feed.as_deref(),
		Some("https://example.org/plume/feed.json")
	);
	assert_eq!(receipt.channel.as_deref(), Some("stable"));
	assert_eq!(
		receipt.updates.as_ref().unwrap().check_interval_hours,
		Some(24)
	);

	s.edit_installer(|t| set_key(&t, "updates", "mode", Some("\"tauri\"")));
	let p = s.load();
	let mut m = windows_machine().with_webview2("131.0.2903.51");
	let receipt = run_install(&p, &mut m, Scope::User).unwrap();
	let feed = li_model::Feed {
		version: 1,
		publisher: "x".into(),
		timestamp: None,
		products: vec![li_model::FeedProduct {
			id: receipt.product.clone(),
			name: "Plume".into(),
			channels: [("stable".to_owned(), "9.0.0".to_owned())].into(),
			releases: vec![li_model::Release {
				version: "9.0.0".into(),
				artefacts: vec![li_model::Artefact {
					target: "windows-x86_64".into(),
					kind: "payload".into(),
					url: "x".into(),
					size: 0,
					sha256: String::new(),
				}],
			}],
			themes: vec![],
		}],
	};
	assert!(li_engine::find_update(&receipt, &feed).is_none());
	let err = update(
		&mut m,
		&p,
		&request(Scope::User).install_dir,
		&feed,
		b"",
		&mut RunOptions::default(),
	)
	.unwrap_err();
	assert!(
		matches!(err, EngineError::UpdatesNotOwned(UpdateMode::Tauri)),
		"{err}"
	);

	s.edit_installer(|t| set_key(&t, "updates", "mode", Some("\"liminal\"")));
	let mut m = windows_machine().with_webview2("131.0.2903.51");
	let receipt = run_install(&s.load(), &mut m, Scope::User).unwrap();
	assert!(li_engine::find_update(&receipt, &feed).is_some());
}

#[test]
fn the_check_interval_gates_feed_checks() {
	use li_engine::tauri::updates::{check_due, record_check};
	let mut m = SimMachine::new();
	let dir = Path::new("/sim/app");
	m.mkdir_all(&dir.join(".liminal"));
	assert!(check_due(&m, dir, None, 1000));
	assert!(check_due(&m, dir, Some(24), 1000));
	record_check(&mut m, dir, 1000).unwrap();
	assert!(!check_due(&m, dir, Some(24), 1000 + 23 * 3600));
	assert!(check_due(&m, dir, Some(24), 1000 + 24 * 3600));
	assert!(check_due(&m, dir, Some(0), 1001));
}

#[test]
fn uninstall_removes_the_time_of_the_last_check() {
	let s = Scratch::new();
	let p = s.load();
	let mut m = windows_machine().with_webview2("131.0.2903.51");
	let before = m.snapshot();
	run_install(&p, &mut m, Scope::User).unwrap();
	let dir = request(Scope::User).install_dir;
	li_engine::tauri::updates::record_check(&mut m, &dir, 5).unwrap();
	uninstall(&mut m, Some(&p), &dir, &mut RunOptions::default()).unwrap();
	assert_eq!(m.snapshot(), before);
}

#[test]
fn an_in_app_update_is_caught_up_by_the_receipt_and_never_reverted() {
	let s = Scratch::new();
	let p = s.load();
	let mut m = windows_machine().with_webview2("131.0.2903.51");
	run_install(&p, &mut m, Scope::User).unwrap();
	let dir = request(Scope::User).install_dir;
	// The Tauri updater runs the product's installer: new files, a new version in the same key.
	let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Plume Notes";
	m.add_file(&dir.join("plume-notes.exe"), b"plume 0.5.0", true);
	m.set_registry_value(key, "DisplayVersion", "0.5.0");

	let next = li_engine::reconcile_receipt(&mut m, &dir)
		.unwrap()
		.expect("the receipt moved");
	assert_eq!(next.installed, "0.5.0");
	assert!(next.operations.iter().all(|o| !matches!(
		o,
		Operation::Copy {
			sha256: Some(_),
			..
		}
	)));
	assert_eq!(read_receipt(&m, &dir).unwrap().installed, "0.5.0");
	// A repair now leaves the newer file alone.
	let payload = DirPayload::new(&p.dir).unwrap();
	let report = repair(&mut m, &payload, &dir, &mut RunOptions::default()).unwrap();
	assert!(report.restored.is_empty(), "{:?}", report.restored);
	assert_eq!(
		m.read(&dir.join("plume-notes.exe")).unwrap(),
		b"plume 0.5.0"
	);
	// Nothing to do a second time.
	assert!(
		li_engine::reconcile_receipt(&mut m, &dir)
			.unwrap()
			.is_none()
	);
}

#[test]
fn a_wrapped_receipt_refreshes_its_external_entry() {
	let (_s, p) = wrap_scratch();
	let mut m = admin_machine();
	nsis(&p, &m);
	let dir = request(Scope::User).install_dir;
	run_install(&p, &mut m, Scope::User).unwrap();
	let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Plume Notes";
	m.set_registry_value(key, "DisplayVersion", "0.5.0");
	let next = li_engine::reconcile_receipt(&mut m, &dir).unwrap().unwrap();
	assert_eq!(next.installed, "0.5.0");
	assert_eq!(next.external().unwrap().version, "0.5.0");
}

#[test]
fn a_tauri_only_product_never_reconciles_a_liminal_one() {
	let s = Scratch::new();
	s.edit_installer(|t| set_key(&t, "updates", "mode", Some("\"liminal\"")));
	let p = s.load();
	let mut m = windows_machine().with_webview2("131.0.2903.51");
	run_install(&p, &mut m, Scope::User).unwrap();
	let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Plume Notes";
	m.set_registry_value(key, "DisplayVersion", "0.5.0");
	assert!(
		li_engine::reconcile_receipt(&mut m, &request(Scope::User).install_dir)
			.unwrap()
			.is_none()
	);
}

#[test]
fn the_launch_of_a_wrapped_install_uses_its_main_binary() {
	let (_s, p) = wrap_scratch();
	let mut m = admin_machine();
	nsis(&p, &m);
	let dir = request(Scope::User).install_dir;
	run_install(&p, &mut m, Scope::User).unwrap();
	li_engine::launch(&mut m, &dir, None, true).unwrap();
	assert_eq!(m.launched()[0].0, dir.join("plume-notes.exe"));
}

// Conflicts among the settings.

fn codes(p: &Project) -> Vec<&'static str> {
	li_engine::check(p).into_iter().map(|d| d.code).collect()
}

#[test]
fn two_updaters_are_a_conflict_and_the_rule_names_the_way_out() {
	let s = Scratch::new();
	s.edit_installer(|t| set_key(&t, "updates", "mode", Some("\"liminal\"")));
	let found = li_engine::check(&s.load());
	let d = found.iter().find(|d| d.code == "updates-conflict").unwrap();
	assert!(d.message.contains("mode = \"both\""));
	s.edit_installer(|t| set_key(&t, "updates", "mode", Some("\"both\"")));
	assert!(!codes(&s.load()).contains(&"updates-conflict"));
	s.edit_installer(|t| set_key(&t, "updates", "mode", Some("\"tauri\"")));
	assert!(!codes(&s.load()).contains(&"updates-conflict"));
}

#[test]
fn tauri_updates_need_a_tauri_section_and_an_updater() {
	let s = Scratch::new();
	s.edit_installer(|t| {
		let t = set_key(&t, "tauri", "config", None);
		set_key(&t, "tauri", "windows", None)
	});
	// With the section's keys gone the project no longer parses as a Tauri project.
	assert!(Project::load_installer_dir(&s.installer()).is_err());

	let s = Scratch::new();
	s.edit_conf(|v| {
		v["plugins"].as_object_mut().unwrap().remove("updater");
	});
	import::import(&s.installer(), &s.conf()).unwrap();
	assert!(codes(&s.load()).contains(&"updates-no-updater"));
}

#[test]
fn native_with_hooks_warns_and_wrap_does_not() {
	let s = Scratch::new();
	s.edit_conf(|v| {
		v["bundle"]["windows"]["nsis"]["installerHooks"] = "./hooks.nsh".into();
	});
	let report = import::import(&s.installer(), &s.conf()).unwrap();
	assert!(report.warnings.iter().any(|w| w.contains("installerHooks")));
	assert!(codes(&s.load()).contains(&"tauri-native-hooks"));
	s.edit_installer(|t| set_key(&t, "tauri", "windows", Some("\"wrap\"")));
	assert!(!codes(&s.load()).contains(&"tauri-native-hooks"));
}

#[test]
fn the_import_summarizes_modes_in_the_snapshot_diff() {
	let s = Scratch::new();
	s.edit_conf(|v| {
		v["bundle"]["windows"]["webviewInstallMode"] =
			serde_json::json!({"type": "offlineInstaller"});
		v["bundle"]["fileAssociations"] = serde_json::json!([]);
	});
	let report = import::reimport(&s.load()).unwrap();
	let fields: Vec<_> = report.changes.iter().map(|c| c.field.as_str()).collect();
	// The overlay still sets embedBootstrapper, so the mode does not move; the associations go.
	assert!(!fields.contains(&"windows.webview2.mode"), "{fields:?}");
	assert!(
		fields.iter().any(|f| f.starts_with("file_associations")),
		"{fields:?}"
	);
}

// Packing: the `.lip` carries what the plan reads.

#[test]
fn the_packed_payload_carries_the_webview2_installer_and_installs_from_the_lip() {
	let s = Scratch::new();
	let p = s.load();
	let bytes = li_engine::pack_payload(&p, "windows-x86_64").unwrap();
	let lip = li_engine::LipPayload::from_bytes(&bytes).unwrap();
	use li_engine::Payload;
	let files = lip.list();
	assert!(files.contains(&"dist/webview2/MicrosoftEdgeWebview2Setup.exe".to_owned()));
	assert!(files.contains(&"dist/windows-x86_64/plume-notes.exe".to_owned()));
	assert!(!files.iter().any(|f| f.ends_with("-setup.exe")));
	let mut m = windows_machine();
	install(
		&mut m,
		&p,
		&lip,
		&request(Scope::User),
		&mut RunOptions::default(),
	)
	.unwrap();
	assert!(webview2::detect(&m).is_some());
}

#[test]
fn a_wrapped_payload_carries_the_setup_and_not_the_apps_files() {
	let (_s, p) = wrap_scratch();
	let bytes = li_engine::pack_payload(&p, "windows-x86_64").unwrap();
	let lip = li_engine::LipPayload::from_bytes(&bytes).unwrap();
	use li_engine::Payload;
	let files = lip.list();
	assert!(files.contains(&"dist/windows-x86_64/Plume Notes-setup.exe".to_owned()));
	assert!(!files.contains(&"dist/windows-x86_64/plume-notes.exe".to_owned()));
	let mut m = admin_machine();
	nsis(&p, &m);
	install(
		&mut m,
		&p,
		&lip,
		&request(Scope::User),
		&mut RunOptions::default(),
	)
	.unwrap();
	// The linux target of the same project is unchanged.
	let linux = li_engine::pack_payload(&p, "linux-x86_64").unwrap();
	let l = li_engine::LipPayload::from_bytes(&linux).unwrap();
	assert!(
		l.list()
			.contains(&"dist/linux-x86_64/plume-notes".to_owned())
	);
}
