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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
	#[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub schema: Option<String>,
	pub version: u32,
	pub id: String,
	pub root: Node,
}

/// One layout node. `kind` is one of `window`, `backdrop`, `dialog`, `stack`, `grid`, `text`,
/// `image`, `billboard`, `meter`, `button`, `checklist`, `radio`, `field`, `path`, `progress`,
/// `slot` (in a theme's stage) or `custom`. Properties are open so renderers can read what they understand.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
	pub kind: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub bind: Option<String>,
	#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
	#[cfg_attr(feature = "ts", ts(optional, type = "Record<string, unknown>"))]
	pub props: BTreeMap<String, serde_json::Value>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	#[cfg_attr(feature = "ts", ts(optional, as = "Option<Vec<Node>>"))]
	pub children: Vec<Node>,
	/// Per-renderer replacements, keyed by renderer id (the `@renderer` overrides of the spec).
	#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
	#[cfg_attr(
		feature = "ts",
		ts(optional, as = "Option<std::collections::BTreeMap<String, Node>>")
	)]
	pub overrides: BTreeMap<String, Node>,
}

/// The id of the layout a theme may carry to draw the screen around every page: the backdrop, its
/// wordmark, the status meters and the taskbar, with a `slot` node where the page's own layout goes.
pub const STAGE_LAYOUT: &str = "stage";

/// The kind of the node in a stage that a page's layout replaces.
pub const SLOT_KIND: &str = "slot";

impl Layout {
	/// The whole screen for one step: the `stage` with the `page` in its `slot`. A stage node with a
	/// `show_on` list appears only on steps whose id or type is in it, and one with a `hide_on` list
	/// is left out on those steps. A page whose root is already a `backdrop` draws itself and the
	/// stage is not used. A stage with no `slot` has the page appended to its root.
	pub fn composed(stage: &Layout, page: &Layout, step_id: &str, step_kind: &str) -> Layout {
		if page.root.kind == "backdrop" {
			return page.clone();
		}
		let mut root = stage.root.clone();
		prune(&mut root, step_id, step_kind);
		if !fill_slot(&mut root, &page.root) {
			root.children.push(page.root.clone());
		}
		Layout {
			schema: page.schema.clone(),
			version: page.version,
			id: page.id.clone(),
			root,
		}
	}
}

fn listed(node: &Node, key: &str, step_id: &str, step_kind: &str) -> Option<bool> {
	let list = node.props.get(key)?.as_array()?;
	Some(
		list.iter()
			.filter_map(|v| v.as_str())
			.any(|s| s == step_id || s == step_kind),
	)
}

fn visible(node: &Node, step_id: &str, step_kind: &str) -> bool {
	listed(node, "show_on", step_id, step_kind).unwrap_or(true)
		&& !listed(node, "hide_on", step_id, step_kind).unwrap_or(false)
}

fn prune(node: &mut Node, step_id: &str, step_kind: &str) {
	node.children.retain(|c| visible(c, step_id, step_kind));
	for c in &mut node.children {
		prune(c, step_id, step_kind);
	}
}

fn fill_slot(node: &mut Node, page: &Node) -> bool {
	for c in &mut node.children {
		if c.kind == SLOT_KIND {
			*c = page.clone();
			return true;
		}
		if fill_slot(c, page) {
			return true;
		}
	}
	false
}

impl Node {
	/// The node a given renderer should draw: its override if one exists, else the base node.
	pub fn for_renderer(&self, renderer: &str) -> &Node {
		self.overrides.get(renderer).unwrap_or(self)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	fn node(kind: &str) -> Node {
		Node {
			kind: kind.into(),
			bind: None,
			props: BTreeMap::new(),
			children: vec![],
			overrides: BTreeMap::new(),
		}
	}

	fn layout(id: &str, root: Node) -> Layout {
		Layout {
			schema: None,
			version: 1,
			id: id.into(),
			root,
		}
	}

	fn stage() -> Layout {
		let mut only_commit = node("meter");
		only_commit
			.props
			.insert("show_on".into(), json!(["commit"]));
		let mut not_finish = node("text");
		not_finish.props.insert("hide_on".into(), json!(["finish"]));
		let mut root = node("backdrop");
		root.children = vec![only_commit, not_finish, node("slot")];
		layout("stage", root)
	}

	#[test]
	fn the_page_fills_the_slot_and_keeps_its_own_id() {
		let mut page_root = node("dialog");
		page_root.children = vec![node("button")];
		let page = layout("welcome", page_root);
		let out = Layout::composed(&stage(), &page, "welcome", "page");
		assert_eq!(out.id, "welcome");
		assert_eq!(out.root.kind, "backdrop");
		let kinds: Vec<_> = out.root.children.iter().map(|c| c.kind.as_str()).collect();
		assert_eq!(kinds, ["text", "dialog"]);
	}

	#[test]
	fn show_on_and_hide_on_match_a_step_id_or_type() {
		let page = layout("copying", node("dialog"));
		let out = Layout::composed(&stage(), &page, "copying", "commit");
		let kinds: Vec<_> = out.root.children.iter().map(|c| c.kind.as_str()).collect();
		assert_eq!(kinds, ["meter", "text", "dialog"]);
		let out = Layout::composed(&stage(), &page, "done", "finish");
		let kinds: Vec<_> = out.root.children.iter().map(|c| c.kind.as_str()).collect();
		assert_eq!(kinds, ["dialog"]);
	}

	#[test]
	fn a_page_with_its_own_backdrop_ignores_the_stage() {
		let page = layout("welcome", node("backdrop"));
		assert_eq!(Layout::composed(&stage(), &page, "welcome", "page"), page);
	}

	#[test]
	fn a_stage_without_a_slot_takes_the_page_last() {
		let st = layout("stage", node("backdrop"));
		let page = layout("welcome", node("window"));
		let out = Layout::composed(&st, &page, "welcome", "page");
		assert_eq!(out.root.children.len(), 1);
		assert_eq!(out.root.children[0].kind, "window");
	}
}
