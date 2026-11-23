// Themes and layouts: a tree of nodes drawn by a renderer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// `theme.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeFile {
	pub theme: ThemeMeta,
	/// Pages this theme adds to any flow.
	#[serde(default)]
	pub pages: BTreeMap<String, PageInsert>,
	#[serde(default)]
	pub audio: Option<Audio>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeMeta {
	pub id: String,
	pub name: String,
	pub renderers: Vec<String>,
	#[serde(default)]
	pub fallback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageInsert {
	pub layout: String,
	#[serde(default)]
	pub before: Option<String>,
	#[serde(default)]
	pub after: Option<String>,
	#[serde(default)]
	pub skippable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Audio {
	pub music: String,
}

/// A layout file, `layouts/<id>.layout.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
	#[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
	pub schema: Option<String>,
	pub version: u32,
	pub id: String,
	pub root: Node,
}

/// One layout node. `kind` is one of `window`, `backdrop`, `dialog`, `stack`, `grid`, `text`,
/// `image`, `billboard`, `meter`, `button`, `checklist`, `radio`, `field`, `path`, `progress`
/// or `custom`. Properties are open so renderers can read what they understand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
	pub kind: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub bind: Option<String>,
	#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
	pub props: BTreeMap<String, serde_json::Value>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub children: Vec<Node>,
	/// Per-renderer replacements, keyed by renderer id (the `@renderer` overrides of the spec).
	#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
	pub overrides: BTreeMap<String, Node>,
}

impl Node {
	/// The node a given renderer should draw: its override if one exists, else the base node.
	pub fn for_renderer(&self, renderer: &str) -> &Node {
		self.overrides.get(renderer).unwrap_or(self)
	}
}
