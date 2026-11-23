// The plan operations a Tauri product adds on Windows: WebView2, then Tauri's own installer
// (`wrap`) or the registrations it would have written (`native`).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeSet;

use li_model::tauri::{InstallMode, NsisInstallMode, Webview2Mode, Webview2Setting};
use li_model::{Component, Operation, Scope, TauriSection, TauriSnapshot};

use crate::error::{EngineError, Result};
use crate::payload::{Payload, expand_sources};
use crate::plan::mkdirs;
use crate::project::Project;
use crate::tauri::nsis::mode_name;
use crate::tauri::registry::{publisher_of, registry_writes};

/// Where the engine stages installers it runs.
const STAGE: &str = "{temp}/liminal-installer";
/// The bootstrapper a `embedBootstrapper` build carries, in the payload.
pub const EMBEDDED_BOOTSTRAPPER: &str = "dist/webview2/MicrosoftEdgeWebview2Setup.exe";
/// The standalone runtime installer an `offlineInstaller` build carries, in the payload.
pub const EMBEDDED_OFFLINE_INSTALLER: &str =
	"dist/webview2/MicrosoftEdgeWebView2RuntimeInstallerX64.exe";

pub fn is_windows(target_id: &str) -> bool {
	target_id.starts_with("windows")
}

/// The WebView2 choice in effect: the import, with the project's `[tauri.webview2]` on top.
pub fn effective_webview2(section: &TauriSection, snap: &TauriSnapshot) -> Webview2Setting {
	match &section.webview2 {
		Some(o) => o.apply(&snap.windows.webview2),
		None => snap.windows.webview2.clone(),
	}
}

/// The hive an install writes to: where Tauri's installer would for `wrap`, the requested scope
/// for `native`.
pub fn hive_for(scope: Scope, mode: InstallMode, nsis: NsisInstallMode) -> &'static str {
	let machine = match (mode, nsis) {
		(InstallMode::Wrap, NsisInstallMode::PerMachine) => true,
		(InstallMode::Wrap, NsisInstallMode::CurrentUser) => false,
		_ => scope == Scope::Machine,
	};
	if machine { "HKLM" } else { "HKCU" }
}

/// What a plan needs to know about a Tauri product on a Windows target.
pub struct WindowsTauri<'a> {
	pub section: &'a TauriSection,
	pub snapshot: &'a TauriSnapshot,
	pub target: &'a str,
	pub hive: &'static str,
}

impl<'a> WindowsTauri<'a> {
	/// `Some` when the project imports a Tauri app, has its snapshot and `target` is Windows.
	pub fn of(project: &'a Project, target: &'a str, scope: Scope) -> Option<Self> {
		let section = project.installer.tauri.as_ref()?;
		let snapshot = project.tauri_snapshot.as_ref()?;
		is_windows(target).then(|| WindowsTauri {
			section,
			snapshot,
			target,
			hive: hive_for(scope, section.windows, snapshot.windows.install_mode),
		})
	}

	pub fn wrap(&self) -> bool {
		self.section.windows == InstallMode::Wrap
	}

	/// Whether a component's parts are the installer's own and so left out of the plan: every
	/// part for `wrap` (Tauri's installer makes them), file types and schemes for `native` (the
	/// registry writes below replace them).
	pub fn skips(&self, c: &Component) -> Skip {
		if c.origin.as_deref() != Some("tauri") {
			Skip::Nothing
		} else if self.wrap() {
			Skip::All
		} else {
			Skip::Registrations
		}
	}

	/// The payload path of the Tauri setup for `wrap`.
	pub fn setup_pattern(&self) -> String {
		self.section
			.wrap
			.as_ref()
			.and_then(|w| w.installer.clone())
			.unwrap_or_else(|| format!("dist/{{target}}/{}-setup.exe", self.snapshot.product_name))
			.replace("{target}", self.target)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
	Nothing,
	Registrations,
	All,
}

fn staged_copy(
	payload: &dyn Payload,
	src: &str,
	dir: &str,
	seen: &mut BTreeSet<String>,
	ops: &mut Vec<Operation>,
) -> Result<String> {
	if payload.size(src).is_none() {
		return Err(EngineError::MissingSource(src.to_owned()));
	}
	mkdirs(dir, seen, ops);
	let name = src.rsplit('/').next().unwrap_or(src);
	let to = format!("{dir}/{name}");
	ops.push(Operation::Copy {
		from: src.to_owned(),
		to: to.clone(),
		sha256: Some(li_pack::sha256_hex(&payload.read(src)?)),
	});
	Ok(to)
}

/// The payload files a Windows Tauri install needs beyond its components: the WebView2 installer
/// the mode carries (or the fixed runtime folder) and, for `wrap`, the Tauri setup.
pub fn payload_files(t: &WindowsTauri<'_>, payload: &dyn Payload) -> Result<Vec<String>> {
	let wv = effective_webview2(t.section, t.snapshot);
	let mut out = Vec::new();
	let one = |p: &str| -> Result<String> {
		expand_sources(payload, p)
			.into_iter()
			.next()
			.map(|(src, _)| src)
			.ok_or_else(|| EngineError::MissingSource(p.to_owned()))
	};
	match wv.mode {
		Webview2Mode::EmbedBootstrapper => out.push(one(EMBEDDED_BOOTSTRAPPER)?),
		Webview2Mode::OfflineInstaller => out.push(one(EMBEDDED_OFFLINE_INSTALLER)?),
		Webview2Mode::FixedRuntime => {
			if let Some(folder) = wv
				.path
				.as_deref()
				.and_then(|p| p.trim_end_matches('/').rsplit('/').next())
			{
				let from = format!("dist/{}/{folder}/", t.target);
				let found = expand_sources(payload, &from);
				if found.is_empty() {
					return Err(EngineError::MissingSource(from));
				}
				out.extend(found.into_iter().map(|(src, _)| src));
			}
		}
		_ => {}
	}
	if t.wrap() {
		out.push(one(&t.setup_pattern())?);
	}
	Ok(out)
}

/// WebView2, then (for `wrap`) the Tauri setup. Runs before the components so the runtime is
/// there when the app is.
pub fn pre_ops(
	t: &WindowsTauri<'_>,
	payload: &dyn Payload,
	seen: &mut BTreeSet<String>,
	ops: &mut Vec<Operation>,
) -> Result<()> {
	let wv = effective_webview2(t.section, t.snapshot);
	let dir = format!("{STAGE}/webview2");
	let (installer, source, path) = match wv.mode {
		Webview2Mode::Skip => (None, None, None),
		Webview2Mode::DownloadBootstrapper => {
			mkdirs(&dir, seen, ops);
			(
				Some(format!("{dir}/MicrosoftEdgeWebview2Setup.exe")),
				Some(li_model::tauri::WEBVIEW2_BOOTSTRAPPER_URL.to_owned()),
				None,
			)
		}
		Webview2Mode::EmbedBootstrapper => (
			Some(staged_copy(
				payload,
				EMBEDDED_BOOTSTRAPPER,
				&dir,
				seen,
				ops,
			)?),
			None,
			None,
		),
		Webview2Mode::OfflineInstaller => (
			Some(staged_copy(
				payload,
				EMBEDDED_OFFLINE_INSTALLER,
				&dir,
				seen,
				ops,
			)?),
			None,
			None,
		),
		Webview2Mode::FixedRuntime => {
			let folder = wv
				.path
				.as_deref()
				.map(|p| p.trim_end_matches('/').rsplit('/').next().unwrap_or(p))
				.filter(|f| !f.is_empty() && *f != ".")
				.ok_or_else(|| {
					EngineError::Other("fixedRuntime needs a `path` to the runtime folder".into())
				})?;
			let from = format!("dist/{}/{folder}/", t.target);
			let found = expand_sources(payload, &from);
			if found.is_empty() {
				return Err(EngineError::MissingSource(from));
			}
			for (src, rel) in found {
				let to = format!("{{install}}/{folder}/{rel}");
				if let Some((parent, _)) = to.rsplit_once('/') {
					mkdirs(parent, seen, ops);
				}
				ops.push(Operation::Copy {
					sha256: Some(li_pack::sha256_hex(&payload.read(&src)?)),
					from: src,
					to,
				});
			}
			(None, None, Some(format!("{{install}}/{folder}")))
		}
	};
	if wv.mode != Webview2Mode::Skip {
		ops.push(Operation::PrereqWebview2 {
			mode: wv.mode,
			silent: wv.silent,
			path,
			min_version: wv.min_version,
			installer,
			source,
			found: None,
			ran: false,
		});
	}

	if t.wrap() {
		let pattern = t.setup_pattern();
		let (src, _) = expand_sources(payload, &pattern)
			.into_iter()
			.next()
			.ok_or(EngineError::MissingSource(pattern))?;
		let staged = staged_copy(payload, &src, &format!("{STAGE}/wrap"), seen, ops)?;
		let sha256 = Some(li_pack::sha256_hex(&payload.read(&src)?));
		ops.push(Operation::TauriWrap {
			installer: staged,
			sha256,
			product_name: t.snapshot.product_name.clone(),
			publisher: publisher_of(t.snapshot),
			hive: t.hive.to_owned(),
			nsis_mode: mode_name(t.snapshot.windows.install_mode).to_owned(),
			args: Vec::new(),
			external: None,
		});
	}
	Ok(())
}

/// For `native`: the registry values Tauri's installer would write, so Add or Remove Programs and
/// the Tauri updater see one app.
pub fn post_ops(t: &WindowsTauri<'_>, payload: &dyn Payload, ops: &mut Vec<Operation>) {
	if t.wrap() {
		return;
	}
	let bytes: u64 = ops
		.iter()
		.filter_map(|o| match o {
			Operation::Copy { from, .. } => payload.size(from),
			_ => None,
		})
		.sum();
	let kb = u32::try_from(bytes.div_ceil(1024)).unwrap_or(u32::MAX);
	let uninstall = r#""{install}\.liminal\runner\liminal-engine.exe" uninstall"#;
	for r in registry_writes(t.snapshot, t.hive, "{install}", uninstall, kb) {
		ops.push(Operation::RegistrySet {
			key: r.key,
			name: r.name,
			value: r.value,
		});
	}
}
