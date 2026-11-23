// Types for Tauri app support: the `[tauri]` and `[updates]` sections, the imported snapshot and
// the receipt's record of a wrapped installer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! See `docs/tauri-apps.md`. The snapshot (`installer/tauri.json`) is what `liminst import tauri`
//! reads out of `tauri.conf.json`; the rest of the types are what a project says about it.

use serde::{Deserialize, Serialize};

/// How the installer puts a Tauri app on a platform.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallMode {
	/// The engine places the files and writes the registrations itself.
	#[default]
	Native,
	/// The engine runs Tauri's own installer silently and records its uninstall entry.
	Wrap,
}

/// How WebView2 arrives, with Tauri's own names.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Webview2Mode {
	#[default]
	DownloadBootstrapper,
	EmbedBootstrapper,
	OfflineInstaller,
	FixedRuntime,
	Skip,
}

impl Webview2Mode {
	pub const ALL: [Webview2Mode; 5] = [
		Webview2Mode::DownloadBootstrapper,
		Webview2Mode::EmbedBootstrapper,
		Webview2Mode::OfflineInstaller,
		Webview2Mode::FixedRuntime,
		Webview2Mode::Skip,
	];

	/// Tauri's name for the mode.
	pub fn name(self) -> &'static str {
		match self {
			Webview2Mode::DownloadBootstrapper => "downloadBootstrapper",
			Webview2Mode::EmbedBootstrapper => "embedBootstrapper",
			Webview2Mode::OfflineInstaller => "offlineInstaller",
			Webview2Mode::FixedRuntime => "fixedRuntime",
			Webview2Mode::Skip => "skip",
		}
	}

	/// What the mode adds to the installer, in megabytes (from the spike's measurements).
	pub fn size_cost_mb(self) -> f32 {
		match self {
			Webview2Mode::DownloadBootstrapper | Webview2Mode::Skip => 0.0,
			Webview2Mode::EmbedBootstrapper => 1.8,
			Webview2Mode::OfflineInstaller => 127.0,
			Webview2Mode::FixedRuntime => 180.0,
		}
	}

	/// Whether the person needs a network connection when the runtime is missing.
	pub fn needs_network(self) -> bool {
		matches!(
			self,
			Webview2Mode::DownloadBootstrapper | Webview2Mode::EmbedBootstrapper
		)
	}
}

/// The Evergreen bootstrapper's download address.
pub const WEBVIEW2_BOOTSTRAPPER_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";
/// The standalone (offline) Evergreen installer for x64.
pub const WEBVIEW2_OFFLINE_URL: &str = "https://go.microsoft.com/fwlink/?linkid=2124701";

fn yes() -> bool {
	true
}

/// A WebView2 choice: what `bundle.windows.webviewInstallMode` says, or a project's override.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Webview2Setting {
	#[serde(default)]
	pub mode: Webview2Mode,
	/// Run the bootstrapper without its own window.
	#[serde(default = "yes")]
	pub silent: bool,
	/// The runtime folder, for `fixedRuntime`.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub path: Option<String>,
	/// If the installed runtime is older, the step installs again.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub min_version: Option<String>,
}

impl Default for Webview2Setting {
	fn default() -> Self {
		Webview2Setting {
			mode: Webview2Mode::default(),
			silent: true,
			path: None,
			min_version: None,
		}
	}
}

/// A project's overrides of the imported WebView2 setting (`[tauri.webview2]`).
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Webview2Override {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub mode: Option<Webview2Mode>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub silent: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub path: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub min_version: Option<String>,
}

impl Webview2Override {
	/// The imported setting with this override on top.
	pub fn apply(&self, base: &Webview2Setting) -> Webview2Setting {
		Webview2Setting {
			mode: self.mode.unwrap_or(base.mode),
			silent: self.silent.unwrap_or(base.silent),
			path: self.path.clone().or_else(|| base.path.clone()),
			min_version: self
				.min_version
				.clone()
				.or_else(|| base.min_version.clone()),
		}
	}
}

/// `[tauri.wrap]`: where the Tauri installer is in the payload.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WrapSettings {
	/// A payload path, with `{target}`. Default: `dist/{target}/<productName>-setup.exe`.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub installer: Option<String>,
}

/// `[tauri]` in `installer.toml`.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TauriSection {
	/// The product's `tauri.conf.json`, relative to `installer/`. Platform overlays are found
	/// beside it.
	pub config: String,
	#[serde(default)]
	pub windows: InstallMode,
	/// macOS and Linux are always native; the fields exist so a project can say so.
	#[serde(default)]
	pub macos: InstallMode,
	#[serde(default)]
	pub linux: InstallMode,
	/// `[product]` fields the project keeps instead of taking them from the import: any of
	/// `id`, `name`, `publisher`, `version`.
	#[serde(default, rename = "override")]
	pub overrides: Vec<String>,
	#[serde(default)]
	pub webview2: Option<Webview2Override>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub wrap: Option<WrapSettings>,
}

/// Who applies updates to the installed product.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateMode {
	/// No feed; the engine can still repair and uninstall from the receipt.
	#[serde(rename = "none")]
	Off,
	/// The installer owns updates, from its signed feed. Works for any product.
	#[default]
	Liminal,
	/// The app updates itself with the Tauri updater; the installer never replaces its files.
	Tauri,
	/// Both are on. The installer still applies feed updates, and the app may update itself in
	/// between; the newer installed version always wins (see `docs/tauri-apps.md`).
	Both,
}

impl UpdateMode {
	/// The word used in `installer.toml`.
	pub fn as_str(self) -> &'static str {
		match self {
			UpdateMode::Off => "none",
			UpdateMode::Liminal => "liminal",
			UpdateMode::Tauri => "tauri",
			UpdateMode::Both => "both",
		}
	}

	/// The engine applies updates from the feed.
	pub fn installer_updates(self) -> bool {
		matches!(self, UpdateMode::Liminal | UpdateMode::Both)
	}

	/// The installed app updates itself.
	pub fn app_updates(self) -> bool {
		matches!(self, UpdateMode::Tauri | UpdateMode::Both)
	}
}

/// `[updates]` in `installer.toml`.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Updates {
	#[serde(default)]
	pub mode: UpdateMode,
	/// The signed feed, as a URL or a path.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub feed: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub channel: Option<String>,
	/// Hours between checks at launch; 0 or absent checks every time.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub check_interval_hours: Option<u32>,
}

/// The `bundle.windows.nsis.installMode` choices.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NsisInstallMode {
	#[default]
	CurrentUser,
	PerMachine,
	Both,
}

/// One source file the snapshot was read from, with its hash.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceHash {
	/// The file name, relative to the config's folder (`tauri.conf.json`, `tauri.windows.conf.json`).
	pub path: String,
	pub sha256: String,
}

/// A file or folder `bundle.resources` copies, normalized to a source and a destination.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRule {
	pub from: String,
	pub to: String,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileAssociation {
	/// Extensions without the dot.
	pub ext: Vec<String>,
	/// The association name (default: the first extension).
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub mime_type: Option<String>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdaterSnapshot {
	#[serde(default)]
	pub endpoints: Vec<String>,
	/// The minisign public key. Never a secret.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub pubkey: Option<String>,
	/// `passive`, `basicUi` or `quiet`, for the Windows installer the updater runs.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub windows_install_mode: Option<String>,
	/// `bundle.createUpdaterArtifacts`.
	#[serde(default)]
	pub create_artifacts: bool,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowsSnapshot {
	pub webview2: Webview2Setting,
	#[serde(default)]
	pub install_mode: NsisInstallMode,
	/// `bundle.windows.allowDowngrades`.
	#[serde(default = "yes")]
	pub allow_downgrades: bool,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub start_menu_folder: Option<String>,
	#[serde(default)]
	pub has_installer_hooks: bool,
	#[serde(default)]
	pub has_custom_template: bool,
}

/// The normalized state of a Tauri project, written to `installer/tauri.json`.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TauriSnapshot {
	pub version: u32,
	pub sources: Vec<SourceHash>,
	pub product_name: String,
	pub identifier: String,
	pub app_version: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub main_binary_name: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub publisher: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub homepage: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	/// `bundle.targets`, as written (`all`, or a list such as `nsis`, `deb`, `app`).
	#[serde(default)]
	pub targets: Vec<String>,
	pub windows: WindowsSnapshot,
	#[serde(default)]
	pub resources: Vec<ResourceRule>,
	#[serde(default)]
	pub external_bin: Vec<String>,
	#[serde(default)]
	pub file_associations: Vec<FileAssociation>,
	/// Deep-link schemes (`plugins.deep-link.desktop.schemes`).
	#[serde(default)]
	pub deep_link_schemes: Vec<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub updater: Option<UpdaterSnapshot>,
}

impl TauriSnapshot {
	pub const VERSION: u32 = 1;
	/// Where the snapshot lives, relative to `installer/`.
	pub const PATH: &'static str = "tauri.json";

	/// The binary's file stem: `mainBinaryName`, else `productName`.
	pub fn binary_stem(&self) -> &str {
		self.main_binary_name
			.as_deref()
			.unwrap_or(&self.product_name)
	}
}

/// What a wrapped installer left behind, recorded in the receipt beside `tauri.wrap`.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct External {
	/// `nsis` for a Tauri setup.
	pub kind: String,
	/// `HKCU` or `HKLM`, where the uninstall key was found.
	pub hive: String,
	/// The key under the hive, `Software\Microsoft\Windows\CurrentVersion\Uninstall\<productName>`.
	pub uninstall_key: String,
	/// The uninstaller's path, unquoted.
	pub uninstaller: String,
	pub install_location: String,
	pub version: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub main_binary: Option<String>,
	/// `Software\<publisher>\<productName>`, which a silent uninstall leaves behind.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub publisher_key: Option<String>,
}
