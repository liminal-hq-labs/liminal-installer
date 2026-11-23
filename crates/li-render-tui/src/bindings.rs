// The values a layout binds to: state fields and live meter readings.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

use serde_json::Value;

/// Reads and writes the values layout nodes bind to.
///
/// Meter and progress values are fractions from 0 to 1 (a value above 1 is read as a
/// percentage). A `file` meter may also have a caption under the key `<bind>.label`.
pub trait Bindings {
	fn get(&self, key: &str) -> Option<&Value>;
	fn set(&mut self, key: &str, value: Value);
}

/// A plain in-memory implementation, used by tests and previews.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapBindings(pub BTreeMap<String, Value>);

impl MapBindings {
	pub fn new() -> Self {
		Self::default()
	}

	/// Builder-style insert, for tests and previews.
	pub fn with(mut self, key: &str, value: Value) -> Self {
		self.0.insert(key.to_owned(), value);
		self
	}
}

impl Bindings for MapBindings {
	fn get(&self, key: &str) -> Option<&Value> {
		self.0.get(key)
	}

	fn set(&mut self, key: &str, value: Value) {
		self.0.insert(key.to_owned(), value);
	}
}

pub(crate) fn get_str(b: &dyn Bindings, key: &str) -> Option<String> {
	match b.get(key)? {
		Value::String(s) => Some(s.clone()),
		Value::Null => None,
		other => Some(other.to_string()),
	}
}

pub(crate) fn get_fraction(b: &dyn Bindings, key: &str) -> f64 {
	let v = b.get(key).and_then(Value::as_f64).unwrap_or(0.0);
	let v = if v > 1.0 { v / 100.0 } else { v };
	v.clamp(0.0, 1.0)
}

pub(crate) fn get_list(b: &dyn Bindings, key: &str) -> Option<Vec<String>> {
	let arr = b.get(key)?.as_array()?;
	Some(
		arr.iter()
			.filter_map(|v| v.as_str().map(str::to_owned))
			.collect(),
	)
}
