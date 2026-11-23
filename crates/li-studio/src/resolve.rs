// Layout helpers: resolving renderer overrides, JSON merge patches and node paths.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_model::Node;
use serde_json::Value;

/// The tree a renderer draws: each node replaced by its override for `renderer`, if any, with
/// the override tables removed.
pub fn resolve_for_renderer(node: &Node, renderer: &str) -> Node {
	let mut out = node.for_renderer(renderer).clone();
	out.overrides.clear();
	out.children = out
		.children
		.iter()
		.map(|c| resolve_for_renderer(c, renderer))
		.collect();
	out
}

/// The node at a path of child indexes from `root`; an empty path is the root itself.
pub fn node_at_mut<'a>(root: &'a mut Node, path: &[usize]) -> Option<&'a mut Node> {
	let mut node = root;
	for &i in path {
		node = node.children.get_mut(i)?;
	}
	Some(node)
}

/// RFC 7386 JSON merge patch: objects merge, `null` deletes a key, anything else replaces.
pub fn merge_patch(target: &mut Value, patch: &Value) {
	match patch {
		Value::Object(map) => {
			if !target.is_object() {
				*target = Value::Object(Default::default());
			}
			let obj = target.as_object_mut().expect("object");
			for (k, v) in map {
				if v.is_null() {
					obj.remove(k);
				} else {
					merge_patch(obj.entry(k.clone()).or_insert(Value::Null), v);
				}
			}
		}
		other => *target = other.clone(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	fn node(kind: &str, children: Vec<Node>) -> Node {
		Node {
			kind: kind.into(),
			bind: None,
			props: Default::default(),
			children,
			overrides: Default::default(),
		}
	}

	#[test]
	fn resolves_nested_overrides() {
		let mut root = node("window", vec![node("text", vec![]), node("button", vec![])]);
		root.children[0]
			.overrides
			.insert("tui-retro".into(), node("dialog", vec![]));
		let retro = resolve_for_renderer(&root, "tui-retro");
		assert_eq!(retro.children[0].kind, "dialog");
		assert_eq!(retro.children[1].kind, "button");
		let canvas = resolve_for_renderer(&root, "canvas");
		assert_eq!(canvas.children[0].kind, "text");
		assert!(canvas.children[0].overrides.is_empty());
	}

	#[test]
	fn finds_nodes_by_path() {
		let mut root = node("window", vec![node("stack", vec![node("text", vec![])])]);
		assert_eq!(node_at_mut(&mut root, &[0, 0]).unwrap().kind, "text");
		assert!(node_at_mut(&mut root, &[1]).is_none());
		assert_eq!(node_at_mut(&mut root, &[]).unwrap().kind, "window");
	}

	#[test]
	fn merge_patch_follows_rfc_7386() {
		let mut v = json!({"a": 1, "b": {"c": 2, "d": 3}});
		merge_patch(&mut v, &json!({"b": {"c": null, "e": 4}, "f": [1]}));
		assert_eq!(v, json!({"a": 1, "b": {"d": 3, "e": 4}, "f": [1]}));
	}
}
