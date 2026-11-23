// Plan operations: what Commit does, each with enough data to undo itself.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

use crate::tauri::{External, Webview2Mode};

/// One reversible change. Paths hold placeholders until the engine resolves them.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
// A plan holds a few hundred operations at most, and boxing the Windows ones would make every
// pattern that reads them reach through a pointer.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
	Mkdir {
		path: String,
	},
	Copy {
		from: String,
		to: String,
		#[serde(default)]
		sha256: Option<String>,
	},
	WriteFile {
		path: String,
		contents: String,
	},
	WriteIni {
		path: String,
		section: String,
		key: String,
		value: String,
	},
	WriteJson {
		path: String,
		pointer: String,
		value: serde_json::Value,
	},
	Shortcut {
		name: String,
		target: String,
		place: String,
	},
	FileType {
		extension: String,
		name: String,
		#[serde(default)]
		open_with: Option<String>,
	},
	UrlScheme {
		scheme: String,
		open_with: String,
	},
	#[serde(rename = "registry.set")]
	RegistrySet {
		key: String,
		name: String,
		value: String,
	},
	#[serde(rename = "path.add")]
	PathAdd {
		entry: String,
	},
	#[serde(rename = "service.install")]
	ServiceInstall {
		name: String,
		command: String,
	},
	#[serde(rename = "os.register")]
	OsRegister {
		product: String,
		version: String,
	},
	/// Makes sure the WebView2 runtime is there (Windows). Detects it through the Evergreen
	/// client key, installs it in the way `mode` says when it is missing or older than
	/// `min_version`, and records what it found.
	#[serde(rename = "prereq.webview2")]
	PrereqWebview2 {
		mode: Webview2Mode,
		#[serde(default)]
		silent: bool,
		/// The runtime folder, for `fixedRuntime`.
		#[serde(default)]
		path: Option<String>,
		#[serde(default)]
		min_version: Option<String>,
		/// Where the bootstrapper or offline installer is staged, as a placeholder path.
		#[serde(default)]
		installer: Option<String>,
		/// Where `downloadBootstrapper` fetches it from.
		#[serde(default)]
		source: Option<String>,
		/// The version found before the step ran (recorded, not planned).
		#[serde(default)]
		found: Option<String>,
		/// Whether the step ran an installer (recorded, not planned).
		#[serde(default)]
		ran: bool,
	},
	/// Runs a Tauri NSIS installer silently and records its uninstall entry (Windows `wrap`).
	#[serde(rename = "tauri.wrap")]
	TauriWrap {
		/// The staged setup, as a placeholder path.
		installer: String,
		/// The setup's SHA-256, so a new build of it is a change the plan reruns.
		#[serde(default)]
		sha256: Option<String>,
		product_name: String,
		publisher: String,
		/// `HKCU` for a per-user install and `HKLM` for a per-machine one.
		hive: String,
		/// `currentUser`, `perMachine` or `both`, as the Tauri installer was built.
		nsis_mode: String,
		/// Extra switches such as `/NS`.
		#[serde(default)]
		args: Vec<String>,
		/// The uninstall entry found after the run (recorded, not planned).
		#[serde(default)]
		external: Option<External>,
	},
	#[serde(rename = "receipt.write")]
	ReceiptWrite {
		path: String,
	},
}

/// An ordered list of operations. `receipt.write` is always last.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
	pub ops: Vec<Operation>,
}
