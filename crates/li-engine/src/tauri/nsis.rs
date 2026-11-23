// Driving Tauri's NSIS installer: the switches, the exit codes, the uninstall entry it leaves and
// a simulated installer for dry runs and tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Everything here is from `docs/tauri-windows-spike.md`: `/S`, `/D=` (always last, unquoted),
//! `/currentuser` and `/allusers` for `installMode: both`, `/UPDATE` for an in-place upgrade,
//! `/NS` for no shortcuts; exit 0 success, 2 aborted, 740 needs elevation. The uninstaller takes
//! `/S` and, with `_?=<dir>`, runs in place so the caller can delete it.

use std::path::{Path, PathBuf};

use li_model::TauriSnapshot;
use li_model::tauri::{External, NsisInstallMode, Webview2Mode};

use crate::error::{EngineError, Result};
use crate::machine::{Machine, registry_entry_key};
use crate::sim::SimMachine;
use crate::tauri::registry::{RegWrite, product_key, publisher_of, registry_writes, uninstall_key};
use crate::tauri::webview2;

/// The `nsis_mode` strings a `tauri.wrap` operation carries.
pub fn mode_name(mode: NsisInstallMode) -> &'static str {
	match mode {
		NsisInstallMode::CurrentUser => "currentUser",
		NsisInstallMode::PerMachine => "perMachine",
		NsisInstallMode::Both => "both",
	}
}

/// What an exit code from the setup means, as a message; `None` for success.
pub fn explain_exit(code: i32) -> Option<String> {
	match code {
		0 => None,
		2 => Some(
			"the Tauri installer stopped without installing (exit 2): a downgrade it refuses, or WebView2 could not be installed"
				.to_owned(),
		),
		740 => Some(
			"the Tauri installer needs elevation (exit 740): run the installer as an administrator for a per-machine install"
				.to_owned(),
		),
		other => Some(format!("the Tauri installer failed (exit {other})")),
	}
}

/// The switches for a silent run. `/D=` is last, as NSIS requires.
pub fn setup_args(
	nsis_mode: &str,
	hive: &str,
	upgrading: bool,
	extra: &[String],
	install_dir: &str,
) -> Vec<String> {
	let mut args = vec!["/S".to_owned()];
	if upgrading {
		args.push("/UPDATE".to_owned());
	}
	if nsis_mode == "both" {
		args.push(
			if hive == "HKLM" {
				"/allusers"
			} else {
				"/currentuser"
			}
			.to_owned(),
		);
	}
	args.extend(extra.iter().cloned());
	args.push(format!("/D={install_dir}"));
	args
}

/// The unquoted form of a registry string: `"C:\x\uninstall.exe"` becomes `C:\x\uninstall.exe`.
fn unquote(v: &str) -> String {
	v.trim().trim_matches('"').to_owned()
}

fn within(key: &str, hive: &str) -> String {
	key.strip_prefix(&format!("{hive}\\"))
		.unwrap_or(key)
		.to_owned()
}

/// Reads the uninstall entry a setup wrote, from `hive` and then the other one.
pub fn find_external(
	m: &dyn Machine,
	product_name: &str,
	publisher: &str,
	hive: &str,
) -> Option<External> {
	let other = if hive == "HKLM" { "HKCU" } else { "HKLM" };
	[hive, other].into_iter().find_map(|h| {
		let key = uninstall_key(h, product_name);
		m.registry_value(&key, "DisplayName")?;
		let get = |name: &str| m.registry_value(&key, name);
		let uninstaller = unquote(&get("UninstallString")?);
		Some(External {
			kind: "nsis".to_owned(),
			hive: h.to_owned(),
			uninstall_key: within(&key, h),
			uninstaller,
			install_location: unquote(&get("InstallLocation").unwrap_or_default()),
			version: get("DisplayVersion").unwrap_or_default(),
			main_binary: get("MainBinaryName"),
			publisher_key: Some(within(&product_key(h, publisher, product_name), h)),
		})
	})
}

/// Runs the product's own uninstaller silently and in place, then removes what a silent
/// uninstall leaves: `uninstall.exe` and the publisher and product key.
pub fn run_uninstall(m: &mut dyn Machine, ext: &External) -> Result<()> {
	let uninstaller = Path::new(&ext.uninstaller);
	let args = vec!["/S".to_owned(), format!("_?={}", ext.install_location)];
	let code = m
		.run(uninstaller, &args)
		.map_err(|e| EngineError::io(uninstaller, e))?;
	if code != 0 {
		return Err(EngineError::Other(format!(
			"the product's uninstaller failed (exit {code})"
		)));
	}
	if m.kind(uninstaller).is_some() {
		m.remove_file(uninstaller)
			.map_err(|e| EngineError::io(uninstaller, e))?;
	}
	if let Some(key) = &ext.publisher_key {
		let full = format!("{}\\{key}", ext.hive);
		m.registry_set(&registry_entry_key(&full, ""), None)
			.map_err(|e| EngineError::Other(format!("registry `{full}`: {e}")))?;
	}
	Ok(())
}

/// A stand-in for the Tauri NSIS setup on a [`SimMachine`], following the spike's observations:
/// elevation, `/currentuser` and `/allusers`, the downgrade gate, the WebView2 section, the
/// files, the registry map, and an uninstaller that leaves the product key behind.
pub struct SimNsis {
	pub snapshot: TauriSnapshot,
}

impl SimNsis {
	pub fn new(snapshot: TauriSnapshot) -> Self {
		SimNsis { snapshot }
	}

	/// Makes `sim` answer for the product's setup (`*-setup.exe`) and `uninstall.exe`.
	pub fn install_on(self, sim: &SimMachine) {
		let snap = self.snapshot;
		sim.on_run(move |m, program, args| handle(&snap, m, program, args));
	}
}

fn handle(snap: &TauriSnapshot, sim: &SimMachine, program: &Path, args: &[String]) -> Option<i32> {
	let name = program.file_name()?.to_string_lossy().to_ascii_lowercase();
	if name == "uninstall.exe" {
		return Some(uninstall(snap, sim, program, args));
	}
	if name.ends_with("setup.exe") && !name.contains("webview2") {
		return Some(setup(snap, sim, args));
	}
	None
}

fn write_all(sim: &SimMachine, writes: &[RegWrite]) {
	for r in writes {
		sim.set_registry_value(&r.key, &r.name, &r.value);
	}
}

fn setup(snap: &TauriSnapshot, sim: &SimMachine, args: &[String]) -> i32 {
	let has = |a: &str| args.iter().any(|x| x == a);
	let dir = args
		.iter()
		.find_map(|a| a.strip_prefix("/D="))
		.map(str::to_owned)
		.unwrap_or_else(|| format!("/sim/home/AppData/Local/{}", snap.product_name));
	let elevated = sim.is_elevated();
	let hive = match snap.windows.install_mode {
		NsisInstallMode::CurrentUser => "HKCU",
		NsisInstallMode::PerMachine | NsisInstallMode::Both if !elevated => return 740,
		NsisInstallMode::PerMachine => "HKLM",
		NsisInstallMode::Both if has("/currentuser") => "HKCU",
		NsisInstallMode::Both => "HKLM",
	};
	let key = uninstall_key(hive, &snap.product_name);
	if !snap.windows.allow_downgrades
		&& let Some(old) = sim.registry_value(&key, "DisplayVersion")
		&& !webview2::at_least(&snap.app_version, &old)
	{
		return 2;
	}
	if !has("/UPDATE")
		&& webview2::detect(sim).is_none()
		&& let Some(code) = sim_webview2(snap, sim, elevated)
	{
		return code;
	}
	let install = PathBuf::from(&dir);
	sim.mkdir_all(&install);
	sim.add_file(
		&install.join(format!("{}.exe", snap.binary_stem())),
		format!("tauri app {}", snap.app_version).as_bytes(),
		true,
	);
	let uninstaller = install.join("uninstall.exe");
	sim.add_file(&uninstaller, b"uninstaller", true);
	let uninstall_string = format!("\"{}\"", uninstaller.display());
	write_all(
		sim,
		&registry_writes(snap, hive, &dir, &uninstall_string, 1024),
	);
	0
}

/// The setup's own WebView2 section, run when the runtime is missing. `Some(code)` stops it.
fn sim_webview2(snap: &TauriSnapshot, sim: &SimMachine, elevated: bool) -> Option<i32> {
	match snap.windows.webview2.mode {
		Webview2Mode::Skip | Webview2Mode::FixedRuntime => None,
		Webview2Mode::DownloadBootstrapper | Webview2Mode::EmbedBootstrapper
			if sim.is_offline() =>
		{
			Some(2)
		}
		_ => {
			let k = if elevated {
				webview2::HKLM_CLIENT_KEY
			} else {
				webview2::HKCU_CLIENT_KEY
			};
			sim.set_registry_value(k, webview2::VERSION_VALUE, webview2::SIM_INSTALLED_VERSION);
			None
		}
	}
}

fn uninstall(snap: &TauriSnapshot, sim: &SimMachine, program: &Path, args: &[String]) -> i32 {
	let dir = args
		.iter()
		.find_map(|a| a.strip_prefix("_?="))
		.map(PathBuf::from)
		.or_else(|| program.parent().map(Path::to_path_buf));
	for hive in ["HKCU", "HKLM"] {
		let key = uninstall_key(hive, &snap.product_name);
		if sim.registry_value(&key, "DisplayName").is_some() {
			for r in registry_writes(snap, hive, "", "", 0) {
				// The product key and the `.ext` keys stay: a silent uninstall cannot remove
				// them. Everything else it wrote goes.
				let residue = r.key == product_key(hive, &publisher_of(snap), &snap.product_name)
					|| r.key.contains(r"\Software\Classes\.");
				if !residue {
					sim.remove_registry_value(&r.key, &r.name);
				}
			}
		}
	}
	if let Some(dir) = dir {
		sim.remove_tree_except(&dir, "uninstall.exe");
	}
	0
}
