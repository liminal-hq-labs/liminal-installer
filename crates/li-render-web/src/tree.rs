// Prepares a layout for rendering: the target renderer's overrides, expanded references and sample values.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::path::Path;

use li_model::{Component, Node};
use serde_json::{Map, Value, json};

/// What a layout's string references resolve against.
pub struct TreeCtx<'a> {
	pub renderer: &'a str,
	pub components: &'a [&'a Component],
	pub state: &'a Map<String, Value>,
	/// The project's `installer/` folder, where document sources such as `LICENCE.TXT` live.
	pub root: &'a Path,
}

/// The readings a meter or progress bar shows in a preview, by what it measures.
pub fn meter_sample(source: &str) -> f64 {
	match source {
		"file" => 0.7,
		"free_space" => 0.94,
		_ => 0.55,
	}
}

fn selected(state: &Map<String, Value>) -> Vec<String> {
	state
		.get("components.selected")
		.or_else(|| state.get("components"))
		.and_then(Value::as_array)
		.map(|a| {
			a.iter()
				.filter_map(|v| v.as_str().map(str::to_owned))
				.collect()
		})
		.unwrap_or_default()
}

/// The list a string `items` or `options` names: the project's components, or a state field a
/// script filled in such as `sound.cards`.
fn expand_list(name: &str, cx: &TreeCtx) -> Option<Value> {
	if name == "components" {
		let on = selected(cx.state);
		return Some(Value::Array(
			cx.components
				.iter()
				.map(|c| {
					json!({
						"id": c.id,
						"label": c.name,
						"checked": on.contains(&c.id) || c.required,
						"required": c.required,
					})
				})
				.collect(),
		));
	}
	let list = cx.state.get(name)?.as_array()?;
	Some(Value::Array(
		list.iter()
			.map(|v| match v {
				Value::Object(o) => {
					let id = o.get("id").cloned().unwrap_or(Value::Null);
					let label = o
						.get("label")
						.or_else(|| o.get("name"))
						.cloned()
						.unwrap_or_else(|| id.clone());
					json!({ "id": id, "label": label })
				}
				other => json!({ "id": other, "label": other }),
			})
			.collect(),
	))
}

/// The node as `cx.renderer` draws it, recursively, with overrides applied and references
/// expanded.
pub fn prepare_tree(node: &Node, cx: &TreeCtx) -> Node {
	let node = node.for_renderer(cx.renderer);
	let mut props = node.props.clone();
	for key in ["items", "options"] {
		if let Some(Value::String(name)) = props.get(key)
			&& let Some(list) = expand_list(name, cx)
		{
			props.insert(key.to_owned(), list);
		}
	}
	if props.get("role").and_then(Value::as_str) == Some("document")
		&& !props.contains_key("text")
		&& let Some(source) = props.get("source").and_then(Value::as_str)
		&& let Ok(text) = std::fs::read_to_string(cx.root.join(source))
	{
		let text: String = text.chars().take(1800).collect();
		props.insert("text".into(), Value::String(text));
	}
	Node {
		kind: node.kind.clone(),
		bind: node.bind.clone(),
		props,
		children: node.children.iter().map(|c| prepare_tree(c, cx)).collect(),
		overrides: BTreeMap::new(),
	}
}

/// Adds a reading for every meter and progress bar the tree binds.
pub fn seed_meters(node: &Node, state: &mut Map<String, Value>) {
	if let Some(bind) = &node.bind
		&& matches!(node.kind.as_str(), "meter" | "progress")
		&& !state.contains_key(bind)
	{
		let source = node
			.props
			.get("source")
			.or_else(|| node.props.get("meter"))
			.and_then(Value::as_str)
			.unwrap_or("total");
		state.insert(bind.clone(), json!(meter_sample(source)));
	}
	for c in &node.children {
		seed_meters(c, state);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn comp(id: &str, name: &str, required: bool) -> Component {
		Component {
			id: id.into(),
			name: name.into(),
			required,
			targets: vec![],
			files: vec![],
			shortcut: vec![],
			file_types: vec![],
			url_schemes: vec![],
			path: vec![],
			origin: None,
		}
	}

	#[test]
	fn references_overrides_and_meters_are_resolved() {
		let a = comp("game", "Game program", true);
		let b = comp("levels", "Levels", false);
		let comps = [&a, &b];
		let mut state = Map::new();
		state.insert("components.selected".into(), json!(["game"]));
		state.insert(
			"sound.cards".into(),
			json!([{ "id": "none", "name": "No sound" }]),
		);
		let cx = TreeCtx {
			renderer: "win16",
			components: &comps,
			state: &state,
			root: Path::new("/nonexistent"),
		};
		let layout: Node = serde_json::from_value(json!({
			"kind": "stack",
			"children": [
				{ "kind": "checklist", "bind": "components.selected", "props": { "items": "components" } },
				{ "kind": "radio", "props": { "options": "sound.cards" } },
				{ "kind": "meter", "bind": "m.file", "props": { "source": "file" } },
				{ "kind": "text", "props": { "text": "base" },
					"overrides": { "win16": { "kind": "text", "props": { "text": "win16" } } } }
			]
		}))
		.unwrap();
		let tree = prepare_tree(&layout, &cx);
		let items = tree.children[0].props["items"].as_array().unwrap();
		assert_eq!(items[0]["checked"], json!(true));
		assert_eq!(items[1]["checked"], json!(false));
		assert_eq!(tree.children[1].props["options"][0]["label"], "No sound");
		assert_eq!(tree.children[3].props["text"], "win16");
		let mut seeded = state.clone();
		seed_meters(&tree, &mut seeded);
		assert_eq!(seeded["m.file"], json!(0.7));
	}
}
