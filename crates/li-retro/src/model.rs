// The compiled project: what `SETUP.LPK` carries besides the payload.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The compiled project is a `postcard` encoding of [`Project`]: a flow graph whose conditions are
//! bytecode ([`crate::expr`]), screens flattened to 80 by 25 text cells with their widgets, the
//! file list and the plan operations. Strings are UTF-8; the renderer maps them to code page 437.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::Error;

/// The columns of the text screen.
pub const COLS: usize = 80;
/// The rows of the text screen.
pub const ROWS: usize = 25;
/// The version of the compiled format.
pub const FORMAT: u16 = 1;

/// The attributes of the retro look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
	pub base: u8,
	pub dialog: u8,
	pub focus: u8,
	pub title: u8,
	pub shadow: u8,
	pub warn: u8,
	pub status: u8,
	/// A disabled button.
	pub disabled: u8,
	/// A button: blue on cyan, drawn `< Next >`.
	pub button: u8,
	/// The button that has the focus.
	pub button_focus: u8,
	/// The access key of a button.
	pub hotkey: u8,
	/// A text field: black on bright white, which needs the card's intensity mode.
	pub field: u8,
}

impl Default for Theme {
	/// White on blue, black on grey dialogs and cyan focus: li-render-tui's Retro look.
	fn default() -> Self {
		Theme {
			base: 0x1f,
			dialog: 0x70,
			focus: 0x30,
			title: 0x78,
			shadow: 0x08,
			warn: 0x1c,
			status: 0x30,
			disabled: 0x78,
			button: 0x31,
			button_focus: 0x1f,
			hotkey: 0x74,
			field: 0xf0,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
	pub id: String,
	pub name: String,
	pub required: bool,
}

/// A component's files are those `FileEntry`s whose `component` is its index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
	pub component: u16,
	/// The destination below the install directory, in 8.3 form with `\` separators.
	pub dest: String,
	pub size: u32,
	pub crc: u32,
}

/// The kinds of step, as in a flow file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
	Page,
	Action,
	Branch,
	Commit,
	Finish,
}

/// The `builtin:` actions the DOS runtime carries out itself. Scripts are not available on DOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Builtin {
	None,
	/// Reads `BLASTER`, probes the usual ports and fills the `sound.*` fields.
	DetectSound,
	/// Refuses to leave the page when the install does not fit the drive.
	CheckDiskSpace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
	pub id: String,
	pub kind: Kind,
	pub screen: Option<u16>,
	/// Bytecode; empty means always.
	pub when: Vec<u8>,
	pub builtin: Builtin,
	pub next: Option<u16>,
	pub branches: Vec<(Vec<u8>, u16)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flow {
	pub id: String,
	pub start: u16,
	pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
	pub x: u8,
	pub y: u8,
	pub w: u8,
	pub h: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
	pub id: String,
	pub label: String,
	pub checked: bool,
	pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Align {
	Left,
	Centre,
	Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
	File,
	Total,
	FreeSpace,
}

/// What a screen draws from live values, over its static cells. The controls (`Button`, `List`,
/// `Field`) are in tab order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Widget {
	/// Text with `{placeholders}`; `limit` is the column it may grow to.
	Text {
		rect: Rect,
		limit: u8,
		attr: u8,
		align: Align,
		template: String,
	},
	Button {
		rect: Rect,
		attr: u8,
		label: String,
		action: String,
		default: bool,
		/// The access key, upper case, or 0.
		hotkey: u8,
		/// Bytecode; empty means always enabled.
		enabled_when: Vec<u8>,
	},
	/// A checklist (`check`) or a radio group.
	List {
		rect: Rect,
		attr: u8,
		check: bool,
		label: String,
		bind: String,
		items: Vec<Item>,
		default: String,
	},
	Field {
		rect: Rect,
		attr: u8,
		label: String,
		bind: String,
		width: u8,
		placeholder: String,
		default: String,
	},
	Meter {
		rect: Rect,
		attr: u8,
		label: String,
		source: Source,
		low_below: u8,
	},
	Billboard {
		rect: Rect,
		attr: u8,
		slides: Vec<String>,
		interval: u8,
	},
	/// Scrolling text, such as the licence.
	Doc { rect: Rect, attr: u8, text: String },
}

impl Widget {
	pub fn is_control(&self) -> bool {
		matches!(
			self,
			Widget::Button { .. } | Widget::List { .. } | Widget::Field { .. }
		)
	}
}

/// A flattened screen: character and attribute planes in runs, and the widgets over them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
	pub id: String,
	/// Runs of `(count: u16 little-endian, char, attr)`, 4 bytes each, covering `COLS * ROWS` cells.
	pub runs: Vec<u8>,
	pub widgets: Vec<Widget>,
}

impl Screen {
	/// Packs 2000 `(char, attr)` cells into runs.
	pub fn pack(cells: &[(u8, u8)]) -> Vec<u8> {
		let mut out = Vec::new();
		let mut i = 0;
		while i < cells.len() {
			let c = cells[i];
			let mut n = 1usize;
			while i + n < cells.len() && cells[i + n] == c && n < usize::from(u16::MAX) {
				n += 1;
			}
			out.extend_from_slice(&(n as u16).to_le_bytes());
			out.push(c.0);
			out.push(c.1);
			i += n;
		}
		out
	}

	/// Unpacks into `chars` and `attrs`, each `COLS * ROWS` long.
	pub fn unpack(&self, chars: &mut [u8], attrs: &mut [u8]) {
		let mut at = 0;
		#[allow(clippy::chunks_exact_to_as_chunks)]
		for run in self.runs.chunks_exact(4) {
			let n = usize::from(u16::from_le_bytes([run[0], run[1]]));
			let end = (at + n).min(chars.len());
			chars[at..end].fill(run[2]);
			attrs[at..end].fill(run[3]);
			at = end;
		}
	}

	/// The indices of the controls in `widgets`, in tab order.
	pub fn controls(&self) -> Vec<usize> {
		self.widgets
			.iter()
			.enumerate()
			.filter(|(_, w)| w.is_control())
			.map(|(i, _)| i)
			.collect()
	}
}

/// A plan operation the DOS runtime performs besides copying the component files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
	/// Creates a directory below the install directory.
	Mkdir { path: String },
	/// Writes one `key=value` into an INI file below the install directory. The value may hold
	/// `{placeholders}`. Skipped when `when_set` names a field that has no value.
	WriteIni {
		path: String,
		section: String,
		key: String,
		value: String,
		when_set: String,
	},
	/// A launcher: writes a batch file that changes to the install directory and runs `target`.
	Shortcut {
		name: String,
		target: String,
		batch: String,
	},
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
	pub format: u16,
	pub id: String,
	pub name: String,
	pub publisher: String,
	pub version: String,
	/// The target, such as `dos-x86:tui-retro`.
	pub target: String,
	pub default_dir: String,
	pub theme: Theme,
	/// The strings the bytecode refers to.
	pub strings: Vec<String>,
	pub components: Vec<Component>,
	pub flows: Vec<Flow>,
	pub screens: Vec<Screen>,
	pub files: Vec<FileEntry>,
	pub ops: Vec<Op>,
}

impl Project {
	pub fn flow(&self, id: &str) -> Option<&Flow> {
		self.flows.iter().find(|f| f.id == id)
	}

	pub fn encode(&self) -> Result<Vec<u8>, Error> {
		postcard::to_allocvec(self).map_err(|e| Error::Format(e.to_string()))
	}

	/// Decodes and validates.
	pub fn decode(bytes: &[u8]) -> Result<Project, Error> {
		let p: Project = postcard::from_bytes(bytes).map_err(|e| Error::Format(e.to_string()))?;
		p.validate()?;
		Ok(p)
	}

	/// Checks every index and every run, so the VM can index without checking again.
	pub fn validate(&self) -> Result<(), Error> {
		let bad = |m: &str| Err(Error::Format(m.to_string()));
		if self.format != FORMAT {
			return bad("unsupported format version");
		}
		for s in &self.screens {
			let mut n = 0usize;
			for run in s.runs.chunks(4) {
				if run.len() != 4 {
					return bad("a screen's cell runs are cut short");
				}
				n += usize::from(u16::from_le_bytes([run[0], run[1]]));
			}
			if n != COLS * ROWS {
				return bad("a screen does not cover 80 by 25 cells");
			}
			for w in &s.widgets {
				let r = match w {
					Widget::Text { rect, .. }
					| Widget::Button { rect, .. }
					| Widget::List { rect, .. }
					| Widget::Field { rect, .. }
					| Widget::Meter { rect, .. }
					| Widget::Billboard { rect, .. }
					| Widget::Doc { rect, .. } => rect,
				};
				if usize::from(r.x) + usize::from(r.w) > COLS
					|| usize::from(r.y) + usize::from(r.h) > ROWS
				{
					return bad("a widget lies outside the screen");
				}
			}
		}
		for f in &self.flows {
			if usize::from(f.start) >= f.steps.len() {
				return bad("a flow starts at a step that does not exist");
			}
			for st in &f.steps {
				let ok = |i: u16| usize::from(i) < f.steps.len();
				if st.next.is_some_and(|n| !ok(n)) || st.branches.iter().any(|(_, n)| !ok(*n)) {
					return bad("a step points at a step that does not exist");
				}
				if st
					.screen
					.is_some_and(|s| usize::from(s) >= self.screens.len())
				{
					return bad("a step names a screen that does not exist");
				}
				crate::expr::check(&st.when, self.strings.len())?;
				for (code, _) in &st.branches {
					crate::expr::check(code, self.strings.len())?;
				}
			}
		}
		for f in &self.files {
			if usize::from(f.component) >= self.components.len() {
				return bad("a file belongs to a component that does not exist");
			}
		}
		Ok(())
	}
}
