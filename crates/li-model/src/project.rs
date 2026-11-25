// Types for `installer.toml` and `components.toml`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

/// `installer.toml`: identity, defaults, targets and signing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstallerFile {
	pub product: Product,
	#[serde(default)]
	pub defaults: Defaults,
	#[serde(default, rename = "target")]
	pub targets: Vec<Target>,
	#[serde(default)]
	pub signing: Signing,
	/// A Tauri project to import from (see `docs/tauri-apps.md`).
	#[serde(default)]
	pub tauri: Option<crate::tauri::TauriSection>,
	/// Who applies updates; absent means the installer does, from the feed in the receipt.
	#[serde(default)]
	pub updates: Option<crate::tauri::Updates>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Product {
	pub id: String,
	pub name: String,
	pub publisher: String,
	pub version: VersionSource,
	#[serde(default)]
	pub licence: Option<String>,
}

/// A literal version, or the place to read it from at build time (`{ from = "git-tag" }`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VersionSource {
	Literal(String),
	From { from: String },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Defaults {
	#[serde(default)]
	pub scope: Scope,
	#[serde(default)]
	pub theme: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
	#[default]
	User,
	Machine,
	Ask,
}

/// A build target such as `linux-x86_64` drawn by one renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Target {
	pub id: String,
	pub renderer: String,
	#[serde(default)]
	pub theme: Option<String>,
	#[serde(default)]
	pub outputs: Vec<String>,
}

impl Target {
	/// The `os-arch:renderer` form used in receipts.
	pub fn full_id(&self) -> String {
		format!("{}:{}", self.id, self.renderer)
	}
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Signing {
	#[serde(default)]
	pub feed_key: Option<String>,
}

/// `components.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ComponentsFile {
	#[serde(default, rename = "component")]
	pub components: Vec<Component>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Component {
	pub id: String,
	pub name: String,
	#[serde(default)]
	pub required: bool,
	/// Whether the component starts ticked on the components page, and is installed by a
	/// silent install that names no components. `false` makes an optional component that
	/// the person ticks to get; a required component cannot start unticked.
	#[serde(default = "ticked")]
	pub default: bool,
	/// Targets this component applies to; empty means all.
	#[serde(default)]
	pub targets: Vec<String>,
	#[serde(default)]
	pub files: Vec<FileRule>,
	#[serde(default)]
	pub shortcut: Vec<ShortcutRule>,
	#[serde(default)]
	pub file_types: Vec<FileTypeRule>,
	#[serde(default)]
	pub url_schemes: Vec<String>,
	#[serde(default)]
	pub path: Vec<String>,
	/// `tauri` for a component the import generated; the next import replaces it.
	#[serde(default)]
	pub origin: Option<String>,
}

fn ticked() -> bool {
	true
}

impl Component {
	/// Whether the install includes the component when the person chooses nothing: required
	/// components, and optional ones that start ticked.
	pub fn on_by_default(&self) -> bool {
		self.required || self.default
	}

	pub fn applies_to(&self, target_id: &str) -> bool {
		self.targets.is_empty() || self.targets.iter().any(|t| t == target_id)
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileRule {
	pub from: String,
	pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortcutRule {
	pub name: String,
	pub target: String,
	#[serde(rename = "in")]
	pub place: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileTypeRule {
	pub extension: String,
	pub name: String,
	#[serde(default)]
	pub open_with: Option<String>,
}
