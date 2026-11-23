// The undo history shared by the UI and the MCP tools.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

use crate::project::{Doc, DocKey};

/// Who made a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
	Ui,
	Mcp,
}

/// One undoable change to one document. `None` means the document does not exist.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
	pub label: String,
	pub source: Source,
	pub key: DocKey,
	pub before: Option<Doc>,
	pub after: Option<Doc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EntrySummary {
	pub index: usize,
	pub label: String,
	pub source: Source,
	pub doc: DocKey,
	/// False for entries that have been undone and can be redone.
	pub applied: bool,
}

/// A linear history with a cursor. Recording a change drops anything that was undone.
#[derive(Debug, Clone)]
pub struct History {
	entries: Vec<Entry>,
	cursor: usize,
	limit: usize,
}

impl Default for History {
	fn default() -> Self {
		History::with_limit(200)
	}
}

impl History {
	pub fn with_limit(limit: usize) -> Self {
		History {
			entries: Vec::new(),
			cursor: 0,
			limit: limit.max(1),
		}
	}

	pub fn record(&mut self, entry: Entry) {
		self.entries.truncate(self.cursor);
		self.entries.push(entry);
		if self.entries.len() > self.limit {
			self.entries.remove(0);
		}
		self.cursor = self.entries.len();
	}

	/// Steps back and returns the entry whose `before` should be restored.
	pub fn undo(&mut self) -> Option<&Entry> {
		if self.cursor == 0 {
			return None;
		}
		self.cursor -= 1;
		self.entries.get(self.cursor)
	}

	/// Steps forward and returns the entry whose `after` should be restored.
	pub fn redo(&mut self) -> Option<&Entry> {
		let entry = self.entries.get(self.cursor)?;
		self.cursor += 1;
		Some(entry)
	}

	pub fn can_undo(&self) -> bool {
		self.cursor > 0
	}

	pub fn can_redo(&self) -> bool {
		self.cursor < self.entries.len()
	}

	pub fn clear(&mut self) {
		self.entries.clear();
		self.cursor = 0;
	}

	pub fn summaries(&self) -> Vec<EntrySummary> {
		self.entries
			.iter()
			.enumerate()
			.map(|(index, e)| EntrySummary {
				index,
				label: e.label.clone(),
				source: e.source,
				doc: e.key.clone(),
				applied: index < self.cursor,
			})
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn entry(label: &str) -> Entry {
		Entry {
			label: label.into(),
			source: Source::Ui,
			key: DocKey::Script("scripts/a.lua".into()),
			before: None,
			after: Some(Doc::Script(label.into())),
		}
	}

	#[test]
	fn undo_and_redo_walk_the_cursor() {
		let mut h = History::default();
		assert!(!h.can_undo() && !h.can_redo());
		h.record(entry("one"));
		h.record(entry("two"));
		assert_eq!(h.undo().unwrap().label, "two");
		assert_eq!(h.undo().unwrap().label, "one");
		assert!(h.undo().is_none());
		assert_eq!(h.redo().unwrap().label, "one");
		assert!(h.can_redo() && h.can_undo());
	}

	#[test]
	fn recording_drops_the_redo_tail() {
		let mut h = History::default();
		h.record(entry("one"));
		h.record(entry("two"));
		h.undo();
		h.record(entry("three"));
		assert!(!h.can_redo());
		let labels: Vec<_> = h.summaries().into_iter().map(|s| s.label).collect();
		assert_eq!(labels, ["one", "three"]);
	}

	#[test]
	fn the_limit_drops_the_oldest() {
		let mut h = History::with_limit(2);
		for l in ["a", "b", "c"] {
			h.record(entry(l));
		}
		let labels: Vec<_> = h.summaries().into_iter().map(|s| s.label).collect();
		assert_eq!(labels, ["b", "c"]);
		assert!(h.summaries().iter().all(|s| s.applied));
	}
}
