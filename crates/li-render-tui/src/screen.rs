// The screen: one layout, its focusable controls and keyboard handling.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_model::{Layout, Node};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use serde_json::Value;

use crate::RENDERER_ID;
use crate::bindings::{Bindings, get_list, get_str};
use crate::glyphs::Look;

/// What a key press did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
	/// Nothing the caller needs to act on, or only focus moved.
	None,
	/// A bound value changed; the key is the bind path.
	Changed(String),
	/// A button was chosen; the value is its action.
	Activate(String),
	/// Esc was pressed.
	Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
	Button,
	Checklist,
	Radio,
	Field,
	Path,
}

impl ControlKind {
	fn word(self) -> &'static str {
		match self {
			ControlKind::Button => "button",
			ControlKind::Checklist => "checklist",
			ControlKind::Radio => "radio group",
			ControlKind::Field => "text field",
			ControlKind::Path => "path field",
		}
	}
}

/// A focusable control as announced to people using the keyboard and to screen readers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlSummary {
	pub kind: ControlKind,
	pub name: String,
	pub bind: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Item {
	pub id: String,
	pub label: String,
	pub checked: bool,
	pub required: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Control {
	pub kind: ControlKind,
	pub name: String,
	pub bind: Option<String>,
	pub action: String,
	pub default_button: bool,
	pub items: Vec<Item>,
	pub default_choice: Option<String>,
}

impl Control {
	fn bind_key(&self) -> String {
		self.bind.clone().unwrap_or_else(|| self.name.clone())
	}

	pub fn checked_ids(&self, b: &dyn Bindings) -> Vec<String> {
		get_list(b, &self.bind_key()).unwrap_or_else(|| {
			self.items
				.iter()
				.filter(|i| i.checked || i.required)
				.map(|i| i.id.clone())
				.collect()
		})
	}

	pub fn chosen_id(&self, b: &dyn Bindings) -> Option<String> {
		get_str(b, &self.bind_key())
			.or_else(|| self.default_choice.clone())
			.or_else(|| self.items.first().map(|i| i.id.clone()))
	}

	pub fn text(&self, b: &dyn Bindings) -> String {
		get_str(b, &self.bind_key()).unwrap_or_default()
	}
}

pub(crate) fn resolve(node: &Node) -> &Node {
	node.for_renderer(RENDERER_ID)
}

pub(crate) fn prop_str<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
	node.props.get(key).and_then(Value::as_str)
}

pub(crate) fn prop_u16(node: &Node, key: &str) -> Option<u16> {
	node.props
		.get(key)
		.and_then(Value::as_u64)
		.map(|n| n.min(u64::from(u16::MAX)) as u16)
}

pub(crate) fn prop_bool(node: &Node, key: &str) -> bool {
	node.props
		.get(key)
		.and_then(Value::as_bool)
		.unwrap_or(false)
}

pub(crate) fn items_of(node: &Node) -> Vec<Item> {
	let Some(arr) = node.props.get("items").and_then(Value::as_array) else {
		return Vec::new();
	};
	arr.iter()
		.filter_map(|v| match v {
			Value::String(s) => Some(Item {
				id: s.clone(),
				label: s.clone(),
				checked: false,
				required: false,
			}),
			Value::Object(o) => {
				let id = o.get("id").and_then(Value::as_str)?;
				Some(Item {
					id: id.to_owned(),
					label: o
						.get("label")
						.and_then(Value::as_str)
						.unwrap_or(id)
						.to_owned(),
					checked: o.get("checked").and_then(Value::as_bool).unwrap_or(false),
					required: o.get("required").and_then(Value::as_bool).unwrap_or(false),
				})
			}
			_ => None,
		})
		.collect()
}

/// The accessible name of a node: `name`, then `label`, then `text`, then its bind path.
pub(crate) fn accessible_name(node: &Node) -> String {
	["name", "label", "text"]
		.iter()
		.find_map(|k| prop_str(node, k))
		.map(str::to_owned)
		.or_else(|| node.bind.clone())
		.unwrap_or_else(|| node.kind.clone())
}

fn control_of(node: &Node) -> Option<Control> {
	let kind = match node.kind.as_str() {
		"button" => ControlKind::Button,
		"checklist" => ControlKind::Checklist,
		"radio" => ControlKind::Radio,
		"field" => ControlKind::Field,
		"path" => ControlKind::Path,
		_ => return None,
	};
	let name = accessible_name(node);
	let action = prop_str(node, "action")
		.or_else(|| prop_str(node, "id"))
		.map(str::to_owned)
		.or_else(|| node.bind.clone())
		.unwrap_or_else(|| name.to_lowercase());
	Some(Control {
		kind,
		name,
		bind: node.bind.clone(),
		action,
		default_button: prop_bool(node, "default"),
		items: items_of(node),
		default_choice: prop_str(node, "default").map(str::to_owned),
	})
}

fn collect(node: &Node, out: &mut Vec<Control>) {
	let node = resolve(node);
	if node.kind == "custom" {
		return;
	}
	if let Some(c) = control_of(node) {
		out.push(c);
	}
	for child in &node.children {
		collect(child, out);
	}
}

/// One layout with its focus and per-control cursors.
#[derive(Debug, Clone)]
pub struct Screen {
	pub(crate) root: Node,
	pub(crate) look: Look,
	pub(crate) controls: Vec<Control>,
	pub(crate) focus: usize,
	pub(crate) cursors: Vec<usize>,
}

impl Screen {
	pub fn new(layout: &Layout, look: Look) -> Screen {
		let root = resolve(&layout.root).clone();
		let mut controls = Vec::new();
		collect(&root, &mut controls);
		let cursors = vec![0; controls.len()];
		let focus = controls
			.iter()
			.position(|c| c.default_button && c.kind == ControlKind::Button)
			.unwrap_or(0);
		Screen {
			root,
			look,
			controls,
			focus,
			cursors,
		}
	}

	pub fn look(&self) -> Look {
		self.look
	}

	/// Every focusable control in tab order.
	pub fn controls(&self) -> Vec<ControlSummary> {
		self.controls
			.iter()
			.map(|c| ControlSummary {
				kind: c.kind,
				name: c.name.clone(),
				bind: c.bind.clone(),
			})
			.collect()
	}

	pub fn focused(&self) -> Option<usize> {
		(!self.controls.is_empty()).then_some(self.focus)
	}

	pub fn focus_index(&mut self, index: usize) {
		if index < self.controls.len() {
			self.focus = index;
		}
	}

	/// What the focused control is, for the status line: `Install, button`.
	pub fn describe_focus(&self, b: &dyn Bindings) -> String {
		let Some(c) = self.controls.get(self.focus) else {
			return String::new();
		};
		let detail = match c.kind {
			ControlKind::Button => String::new(),
			ControlKind::Checklist => {
				let on = c.checked_ids(b);
				let item = c.items.get(self.cursors[self.focus]);
				match item {
					Some(i) => format!(
						", {} {}",
						i.label,
						if on.contains(&i.id) {
							"checked"
						} else {
							"not checked"
						}
					),
					None => String::new(),
				}
			}
			ControlKind::Radio => {
				let chosen = c.chosen_id(b);
				match c.items.get(self.cursors[self.focus]) {
					Some(i) => format!(
						", {} {}",
						i.label,
						if chosen.as_deref() == Some(i.id.as_str()) {
							"selected"
						} else {
							"not selected"
						}
					),
					None => String::new(),
				}
			}
			ControlKind::Field | ControlKind::Path => format!(", value \"{}\"", c.text(b)),
		};
		format!("{}, {}{}", c.name, c.kind.word(), detail)
	}

	fn move_focus(&mut self, delta: isize) {
		let n = self.controls.len();
		if n > 0 {
			self.focus = (self.focus as isize + delta).rem_euclid(n as isize) as usize;
		}
	}

	/// Handles one key press. Key releases are ignored.
	pub fn handle_key(&mut self, key: KeyEvent, b: &mut dyn Bindings) -> Outcome {
		if key.kind == KeyEventKind::Release {
			return Outcome::None;
		}
		let shift = key.modifiers.contains(KeyModifiers::SHIFT);
		let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
		if self.controls.is_empty() {
			return match key.code {
				KeyCode::Esc => Outcome::Cancel,
				_ => Outcome::None,
			};
		}
		let focus = self.focus;
		let kind = self.controls[focus].kind;
		match key.code {
			KeyCode::Esc => Outcome::Cancel,
			KeyCode::BackTab => {
				self.move_focus(-1);
				Outcome::None
			}
			KeyCode::Tab => {
				self.move_focus(if shift { -1 } else { 1 });
				Outcome::None
			}
			KeyCode::Up | KeyCode::Down => {
				let down = key.code == KeyCode::Down;
				if matches!(kind, ControlKind::Checklist | ControlKind::Radio) {
					let len = self.controls[focus].items.len();
					let cur = self.cursors[focus];
					if down && cur + 1 < len {
						self.cursors[focus] += 1;
						return self.after_cursor_move(b);
					}
					if !down && cur > 0 {
						self.cursors[focus] -= 1;
						return self.after_cursor_move(b);
					}
				}
				self.move_focus(if down { 1 } else { -1 });
				Outcome::None
			}
			KeyCode::Left | KeyCode::Right => {
				if kind == ControlKind::Button {
					self.move_focus(if key.code == KeyCode::Right { 1 } else { -1 });
				}
				Outcome::None
			}
			KeyCode::Enter => {
				if kind == ControlKind::Button {
					return Outcome::Activate(self.controls[focus].action.clone());
				}
				match self
					.controls
					.iter()
					.find(|c| c.kind == ControlKind::Button && c.default_button)
				{
					Some(c) => Outcome::Activate(c.action.clone()),
					None => Outcome::None,
				}
			}
			KeyCode::Char(' ') if matches!(kind, ControlKind::Checklist | ControlKind::Radio) => {
				self.toggle(b)
			}
			KeyCode::Char(' ') if kind == ControlKind::Button => {
				Outcome::Activate(self.controls[focus].action.clone())
			}
			KeyCode::Char(c) if matches!(kind, ControlKind::Field | ControlKind::Path) && !ctrl => {
				let ctl = &self.controls[focus];
				let key = ctl.bind_key();
				let mut text = ctl.text(b);
				text.push(c);
				b.set(&key, Value::String(text));
				Outcome::Changed(key)
			}
			KeyCode::Backspace if matches!(kind, ControlKind::Field | ControlKind::Path) => {
				let ctl = &self.controls[focus];
				let key = ctl.bind_key();
				let mut text = ctl.text(b);
				if text.pop().is_some() {
					b.set(&key, Value::String(text));
					Outcome::Changed(key)
				} else {
					Outcome::None
				}
			}
			_ => Outcome::None,
		}
	}

	fn after_cursor_move(&mut self, b: &mut dyn Bindings) -> Outcome {
		// Radio groups select as the cursor moves; checklists only move.
		if self.controls[self.focus].kind == ControlKind::Radio {
			return self.toggle(b);
		}
		Outcome::None
	}

	fn toggle(&mut self, b: &mut dyn Bindings) -> Outcome {
		let ctl = &self.controls[self.focus];
		let Some(item) = ctl.items.get(self.cursors[self.focus]) else {
			return Outcome::None;
		};
		let key = ctl.bind_key();
		match ctl.kind {
			ControlKind::Radio => {
				if ctl.chosen_id(b).as_deref() == Some(item.id.as_str()) {
					return Outcome::None;
				}
				b.set(&key, Value::String(item.id.clone()));
			}
			_ => {
				if item.required {
					return Outcome::None;
				}
				let mut on = ctl.checked_ids(b);
				if let Some(pos) = on.iter().position(|id| *id == item.id) {
					on.remove(pos);
				} else {
					on.push(item.id.clone());
				}
				b.set(
					&key,
					Value::Array(on.into_iter().map(Value::String).collect()),
				);
			}
		}
		Outcome::Changed(key)
	}
}
