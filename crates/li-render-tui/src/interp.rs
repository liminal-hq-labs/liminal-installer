// Fills `{placeholder}` text in a layout from the bound values.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_model::Node;
use serde_json::Value;

use crate::bindings::{Bindings, get_str};

/// Replaces each `{name}` with the bound value of `name`. A name with no value stays as written,
/// so a missing binding is visible rather than silently blank.
pub fn interpolate(text: &str, b: &dyn Bindings) -> String {
	let mut out = String::with_capacity(text.len());
	let mut rest = text;
	while let Some(open) = rest.find('{') {
		out.push_str(&rest[..open]);
		let after = &rest[open + 1..];
		match after.find('}') {
			Some(close)
				if !after[..close].is_empty()
					&& after[..close]
						.chars()
						.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) =>
			{
				match get_str(b, &after[..close]) {
					Some(v) => out.push_str(&v),
					None => out.push_str(&rest[open..open + close + 2]),
				}
				rest = &after[close + 1..];
			}
			_ => {
				out.push('{');
				rest = after;
			}
		}
	}
	out.push_str(rest);
	out
}

fn value(v: &Value, b: &dyn Bindings) -> Value {
	match v {
		Value::String(s) if s.contains('{') => Value::String(interpolate(s, b)),
		Value::Array(a) => Value::Array(a.iter().map(|x| value(x, b)).collect()),
		Value::Object(o) => {
			Value::Object(o.iter().map(|(k, x)| (k.clone(), value(x, b))).collect())
		}
		other => other.clone(),
	}
}

/// A copy of `node` with every string property interpolated.
pub fn interpolate_node(node: &Node, b: &dyn Bindings) -> Node {
	Node {
		kind: node.kind.clone(),
		bind: node.bind.clone(),
		props: node
			.props
			.iter()
			.map(|(k, v)| (k.clone(), value(v, b)))
			.collect(),
		children: node
			.children
			.iter()
			.map(|c| interpolate_node(c, b))
			.collect(),
		overrides: node
			.overrides
			.iter()
			.map(|(k, v)| (k.clone(), interpolate_node(v, b)))
			.collect(),
	}
}
