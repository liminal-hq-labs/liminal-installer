// The retro renderer: a flattened screen drawn into text cells, with its widgets and keyboard.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A [`Page`] is one flattened [`Screen`] with its focus. [`Page::draw`] unpacks the static
//! cells and draws each widget over them from the run's [`Values`], following li-render-tui's
//! Retro drawing so the two stay comparable. Keys follow its [`Screen::handle_key`] too, with the
//! additions a DOS setup has: access keys, F3 to quit, a scrolling document and disabled buttons.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::cp437;
use crate::expr::{self, Scope, Val};
use crate::model::{Align, COLS, Item, Project, ROWS, Rect, Screen, Source, Theme, Widget};
use crate::platform::Key;
use crate::text;

/// The text screen: a character plane and an attribute plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cells {
	pub ch: Vec<u8>,
	pub at: Vec<u8>,
}

impl Default for Cells {
	fn default() -> Self {
		Cells {
			ch: vec![b' '; COLS * ROWS],
			at: vec![0x07; COLS * ROWS],
		}
	}
}

impl Cells {
	/// Writes `s` at (`x`, `y`) in code page 437, stopping at column `max_x` or the screen edge.
	pub fn put(&mut self, x: usize, y: usize, s: &str, attr: u8, max_x: usize) {
		if y >= ROWS {
			return;
		}
		for (x, c) in (x..max_x.min(COLS)).zip(s.chars()) {
			self.ch[y * COLS + x] = cp437::encode_char(c);
			self.at[y * COLS + x] = attr;
		}
	}

	pub fn fill(&mut self, r: Rect, ch: u8, attr: u8) {
		for y in usize::from(r.y)..(usize::from(r.y) + usize::from(r.h)).min(ROWS) {
			for x in usize::from(r.x)..(usize::from(r.x) + usize::from(r.w)).min(COLS) {
				self.ch[y * COLS + x] = ch;
				self.at[y * COLS + x] = attr;
			}
		}
	}

	/// The screen as text, one line per row with trailing spaces removed, as li-render-tui's dump.
	pub fn text(&self) -> String {
		let mut out = String::new();
		for y in 0..ROWS {
			let row: String = (0..COLS)
				.map(|x| cp437::decode_byte(self.ch[y * COLS + x]))
				.collect();
			out.push_str(row.trim_end());
			out.push('\n');
		}
		out
	}
}

/// A value the person has chosen or typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
	Str(String),
	List(Vec<String>),
}

/// Everything a page reads and writes: the flow's state, the computed values behind
/// `{placeholders}` and the meters.
#[derive(Debug, Clone, Default)]
pub struct Values {
	/// Fields by bind name, such as `install.path` and `components.selected`.
	pub state: BTreeMap<String, Value>,
	/// Computed text such as `product.name`, `install.size` and `progress.file`.
	pub vars: BTreeMap<String, String>,
	/// The meters, in whole percent.
	pub file_pct: u32,
	pub total_pct: u32,
	pub free_pct: u32,
}

impl Values {
	pub fn str(&self, key: &str) -> Option<&str> {
		match self.state.get(key) {
			Some(Value::Str(s)) => Some(s),
			_ => None,
		}
	}

	pub fn list(&self, key: &str) -> Option<&Vec<String>> {
		match self.state.get(key) {
			Some(Value::List(l)) => Some(l),
			_ => None,
		}
	}

	pub fn set_str(&mut self, key: &str, v: &str) {
		self.state
			.insert(key.to_string(), Value::Str(v.to_string()));
	}

	pub fn set_var(&mut self, key: &str, v: &str) {
		self.vars.insert(key.to_string(), v.to_string());
	}

	/// The text a `{name}` stands for.
	pub fn text(&self, name: &str) -> Option<String> {
		if let Some(v) = self.vars.get(name) {
			return Some(v.clone());
		}
		let key = name.strip_prefix("state.").unwrap_or(name);
		match self.state.get(key)? {
			Value::Str(s) => Some(s.clone()),
			Value::List(l) => Some(l.join(", ")),
		}
	}

	pub fn interpolate(&self, template: &str) -> String {
		text::interpolate(template, &|n| self.text(n))
	}

	fn percent(&self, source: Source) -> u32 {
		match source {
			Source::File => self.file_pct,
			Source::Total => self.total_pct,
			Source::FreeSpace => self.free_pct,
		}
		.min(100)
	}
}

fn val_of(v: &Value) -> Val {
	match v {
		Value::Str(s) => Val::Str(s.clone()),
		Value::List(l) => Val::List(l.iter().map(|s| Val::Str(s.clone())).collect()),
	}
}

impl Scope for Values {
	/// `components` is the chosen component list, `state.x` a field, anything else a computed
	/// value (`target.renderer`, `system.free_space`, `system.installed`).
	fn lookup(&self, path: &str) -> Val {
		if path == "components" {
			return self
				.state
				.get("components.selected")
				.map_or(Val::List(Vec::new()), val_of);
		}
		if let Some(key) = path.strip_prefix("state.") {
			return self.state.get(key).map_or(Val::Null, val_of);
		}
		match self.vars.get(path) {
			None => Val::Null,
			Some(v) if path.starts_with("system.") => match (v.as_str(), v.parse::<i64>()) {
				("true", _) => Val::Bool(true),
				("false", _) => Val::Bool(false),
				(_, Ok(n)) => Val::Num(n),
				_ => Val::Str(v.clone()),
			},
			Some(v) => Val::Str(v.clone()),
		}
	}
}

/// What a key did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
	None,
	/// A bound value or the focus changed, so the screen needs drawing again.
	Redraw,
	/// A button was chosen; the value is its action.
	Activate(String),
	/// Esc or F3 on a page with no Back button.
	Cancel,
}

/// One screen on display.
pub struct Page {
	screen: Screen,
	theme: Theme,
	controls: Vec<usize>,
	pub focus: usize,
	cursors: Vec<usize>,
	doc: usize,
	pub slide: usize,
	started: u32,
}

fn rect_end(r: Rect) -> usize {
	usize::from(r.x) + usize::from(r.w)
}

impl Page {
	/// Prepares a page and gives the fields it binds their defaults, as the page would show them.
	pub fn new(project: &Project, screen: usize, v: &mut Values, now: u32) -> Page {
		let screen = project.screens[screen].clone();
		let controls = screen.controls();
		let mut focus = 0;
		for (n, &i) in controls.iter().enumerate() {
			if matches!(&screen.widgets[i], Widget::Button { default: true, .. }) {
				focus = n;
				break;
			}
		}
		for w in &screen.widgets {
			match w {
				Widget::List {
					check: true,
					bind,
					items,
					..
				} if !v.state.contains_key(bind) => {
					let on = items
						.iter()
						.filter(|i| i.checked || i.required)
						.map(|i| i.id.clone())
						.collect();
					v.state.insert(bind.clone(), Value::List(on));
				}
				Widget::List {
					check: false,
					bind,
					default,
					..
				} if !default.is_empty() && !v.state.contains_key(bind) => {
					v.set_str(bind, default);
				}
				Widget::Field { bind, default, .. } if !v.state.contains_key(bind) => {
					let d = v.interpolate(default);
					v.set_str(bind, &d);
				}
				_ => {}
			}
		}
		let cursors = vec![0; controls.len()];
		Page {
			screen,
			theme: project.theme,
			controls,
			focus,
			cursors,
			doc: 0,
			slide: 0,
			started: now,
		}
	}

	pub fn id(&self) -> &str {
		&self.screen.id
	}

	/// Moves the billboard on when its interval has passed. Returns true if the screen changed.
	pub fn tick(&mut self, now: u32) -> bool {
		for w in &self.screen.widgets {
			if let Widget::Billboard {
				slides, interval, ..
			} = w && slides.len() > 1
				&& *interval > 0
			{
				let n = (now.wrapping_sub(self.started) / (u32::from(*interval) * 1000)) as usize
					% slides.len();
				if n != self.slide {
					self.slide = n;
					return true;
				}
			}
		}
		false
	}

	fn focused(&self) -> Option<&Widget> {
		self.controls
			.get(self.focus)
			.map(|&i| &self.screen.widgets[i])
	}

	fn enabled(&self, w: &Widget, project_strings: &[String], v: &Values) -> bool {
		match w {
			Widget::Button { enabled_when, .. } => expr::holds(enabled_when, project_strings, v),
			_ => true,
		}
	}

	/// Draws the page into `cells`.
	pub fn draw(&self, strings: &[String], v: &Values, cells: &mut Cells) {
		self.screen.unpack(&mut cells.ch, &mut cells.at);
		let t = &self.theme;
		for (wi, w) in self.screen.widgets.iter().enumerate() {
			let ctl = self.controls.iter().position(|&c| c == wi);
			let focused = ctl.is_some() && ctl == Some(self.focus);
			let cursor = ctl.map_or(0, |c| self.cursors[c]);
			match w {
				Widget::Text {
					rect,
					limit,
					attr,
					align,
					template,
				} => {
					let avail = usize::from(*limit)
						.saturating_sub(usize::from(rect.x))
						.max(usize::from(rect.w));
					let body = v.interpolate(template);
					for (i, line) in text::wrap(&body, avail)
						.iter()
						.take(usize::from(rect.h).max(1))
						.enumerate()
					{
						let n = line.chars().count();
						let pad = match align {
							Align::Centre => avail.saturating_sub(n) / 2,
							Align::Right => avail.saturating_sub(n),
							Align::Left => 0,
						};
						cells.put(
							usize::from(rect.x) + pad,
							usize::from(rect.y) + i,
							line,
							*attr,
							COLS,
						);
					}
				}
				Widget::Button {
					rect,
					attr,
					label,
					hotkey,
					..
				} => {
					let on = self.enabled(w, strings, v);
					let style = if focused {
						t.button_focus
					} else if !on {
						t.disabled
					} else {
						*attr
					};
					let label = v.interpolate(label);
					let (x, y) = (usize::from(rect.x), usize::from(rect.y));
					cells.put(x, y, &format!("< {label} >"), style, COLS);
					if !focused
						&& on && *hotkey != 0
						&& let Some(i) = label
							.chars()
							.position(|c| c.to_ascii_uppercase() as u32 == u32::from(*hotkey))
					{
						let ch = label.chars().nth(i).unwrap_or(' ');
						let hot = (style & 0xf0) | (t.hotkey & 0x0f);
						cells.put(x + 2 + i, y, &ch.to_string(), hot, COLS);
					}
				}
				Widget::List {
					rect,
					attr,
					check,
					label,
					bind,
					items,
					default,
				} => {
					let mut y = usize::from(rect.y);
					let x = usize::from(rect.x);
					if !label.is_empty() {
						cells.put(x, y, &v.interpolate(label), *attr, COLS);
						y += 1;
					}
					let on: Vec<String> = if *check {
						v.list(bind).cloned().unwrap_or_default()
					} else {
						v.str(bind)
							.map(|s| s.to_string())
							.or_else(|| (!default.is_empty()).then(|| default.clone()))
							.into_iter()
							.collect()
					};
					for (i, item) in items.iter().enumerate() {
						let glyph = match (*check, on.contains(&item.id)) {
							(true, true) => "[x]",
							(true, false) => "[ ]",
							(false, true) => "(\u{2022})",
							(false, false) => "( )",
						};
						let here = focused && cursor == i;
						let line = format!(
							"{}{} {}",
							if here { ">" } else { " " },
							glyph,
							v.interpolate(&item.label)
						);
						let style = if here { t.focus } else { *attr };
						if here {
							cells.fill(
								Rect {
									x: rect.x,
									y: y as u8,
									w: rect.w,
									h: 1,
								},
								b' ',
								style,
							);
						}
						cells.put(x, y, &line, style, rect_end(*rect));
						y += 1;
					}
				}
				Widget::Field {
					rect,
					attr,
					label,
					bind,
					width,
					placeholder,
					..
				} => {
					let mut x = usize::from(rect.x);
					let y = usize::from(rect.y);
					if !label.is_empty() {
						cells.put(x, y, &format!("{label}: "), *attr, COLS);
						x += label.chars().count() + 2;
					}
					let width = usize::from(*width);
					let value = v.str(bind).unwrap_or("").to_string();
					let shown = if value.is_empty() && !focused {
						placeholder.clone()
					} else {
						value
					};
					let mut shown = if focused { format!("{shown}_") } else { shown };
					let len = shown.chars().count();
					if len > width {
						shown = shown.chars().skip(len - width).collect();
					}
					let padded = format!("{shown:<width$}");
					let style = if focused { t.focus } else { t.field };
					cells.put(x, y, "[", *attr, COLS);
					cells.put(x + 1, y, &padded, style, COLS);
					cells.put(x + 1 + width, y, "]", *attr, COLS);
				}
				Widget::Meter {
					rect,
					attr,
					label,
					source,
					low_below,
				} => {
					draw_meter(
						cells,
						t,
						*rect,
						*attr,
						label,
						*source,
						*low_below,
						v.percent(*source),
					);
				}
				Widget::Billboard {
					rect, attr, slides, ..
				} => {
					cells.fill(*rect, b' ', *attr);
					frame(cells, *rect, *attr);
					if !slides.is_empty() {
						let body = v.interpolate(&slides[self.slide % slides.len()]);
						for (i, line) in text::wrap(&body, usize::from(rect.w).saturating_sub(4))
							.iter()
							.enumerate()
						{
							cells.put(
								usize::from(rect.x) + 2,
								usize::from(rect.y) + 1 + i,
								line,
								*attr,
								rect_end(*rect) - 1,
							);
						}
					}
				}
				Widget::Doc {
					rect,
					attr,
					text: body,
				} => {
					let lines = text::wrap(body, usize::from(rect.w).saturating_sub(2));
					let rows = usize::from(rect.h);
					for (i, line) in lines.iter().skip(self.doc).take(rows).enumerate() {
						cells.put(
							usize::from(rect.x),
							usize::from(rect.y) + i,
							line,
							*attr,
							rect_end(*rect) - 1,
						);
					}
					let x = rect_end(*rect) - 1;
					if self.doc > 0 {
						cells.put(x, usize::from(rect.y), "\u{2191}", *attr, COLS);
					}
					if self.doc + rows < lines.len() {
						cells.put(x, usize::from(rect.y) + rows - 1, "\u{2193}", *attr, COLS);
					}
				}
			}
		}
	}

	/// What the focused control is, for the status line: `Install, button`.
	pub fn describe_focus(&self, _strings: &[String], v: &Values) -> String {
		let Some(w) = self.focused() else {
			return String::new();
		};
		let cursor = self.cursors[self.focus];
		match w {
			Widget::Button { label, .. } => format!("{}, button", v.interpolate(label)),
			Widget::List {
				check,
				label,
				bind,
				items,
				default,
				..
			} => {
				let name = v.interpolate(label);
				let (kind, state) = match items.get(cursor) {
					Some(i) if *check => {
						let on = v.list(bind).is_some_and(|l| l.contains(&i.id));
						(
							"checklist",
							format!(
								", {} {}",
								v.interpolate(&i.label),
								if on { "checked" } else { "not checked" }
							),
						)
					}
					Some(i) => {
						let chosen = v.str(bind).map(|s| s == i.id).unwrap_or(*default == i.id);
						(
							"radio group",
							format!(
								", {} {}",
								v.interpolate(&i.label),
								if chosen { "selected" } else { "not selected" }
							),
						)
					}
					None => (
						if *check { "checklist" } else { "radio group" },
						String::new(),
					),
				};
				format!("{name}, {kind}{state}")
			}
			Widget::Field { label, bind, .. } => {
				format!(
					"{label}, text field, value \"{}\"",
					v.str(bind).unwrap_or("")
				)
			}
			_ => String::new(),
		}
	}

	fn move_focus(&mut self, delta: isize) {
		let n = self.controls.len();
		if n > 0 {
			self.focus = (self.focus as isize + delta).rem_euclid(n as isize) as usize;
		}
	}

	fn button_action(&self, w: &Widget, strings: &[String], v: &Values) -> Option<String> {
		match w {
			Widget::Button { action, .. } if self.enabled(w, strings, v) => Some(action.clone()),
			_ => None,
		}
	}

	fn has_action(&self, action: &str) -> bool {
		self.screen
			.widgets
			.iter()
			.any(|w| matches!(w, Widget::Button { action: a, .. } if a == action))
	}

	fn default_action(&self, strings: &[String], v: &Values) -> Option<String> {
		self.screen.widgets.iter().find_map(|w| match w {
			Widget::Button { default: true, .. } => self.button_action(w, strings, v),
			_ => None,
		})
	}

	fn toggle(&mut self, v: &mut Values) -> Outcome {
		let Some(Widget::List {
			check, bind, items, ..
		}) = self.focused().cloned()
		else {
			return Outcome::None;
		};
		let Some(item) = items.get(self.cursors[self.focus]) else {
			return Outcome::None;
		};
		if check {
			if item.required {
				return Outcome::None;
			}
			let mut on = v.list(&bind).cloned().unwrap_or_default();
			match on.iter().position(|id| *id == item.id) {
				Some(p) => {
					on.remove(p);
				}
				None => on.push(item.id.clone()),
			}
			v.state.insert(bind, Value::List(on));
		} else {
			if v.str(&bind) == Some(item.id.as_str()) {
				return Outcome::None;
			}
			v.set_str(&bind, &item.id.clone());
		}
		Outcome::Redraw
	}

	fn doc_lines(&self) -> Option<(usize, usize)> {
		self.screen.widgets.iter().find_map(|w| match w {
			Widget::Doc {
				rect, text: body, ..
			} => Some((
				text::wrap(body, usize::from(rect.w).saturating_sub(2)).len(),
				usize::from(rect.h),
			)),
			_ => None,
		})
	}

	/// Handles one key.
	pub fn key(&mut self, key: Key, strings: &[String], v: &mut Values) -> Outcome {
		if let Some((total, rows)) = self.doc_lines() {
			let max = total.saturating_sub(rows);
			let before = self.doc;
			match key {
				Key::PageDown => self.doc = (self.doc + rows.saturating_sub(1).max(1)).min(max),
				Key::PageUp => self.doc = self.doc.saturating_sub(rows.saturating_sub(1).max(1)),
				_ => {}
			}
			if self.doc != before {
				return Outcome::Redraw;
			}
		}
		match key {
			Key::F(3) => return Outcome::Cancel,
			Key::Esc => {
				return if self.has_action("back") {
					Outcome::Activate("back".into())
				} else {
					Outcome::Cancel
				};
			}
			_ => {}
		}
		if self.controls.is_empty() {
			return Outcome::None;
		}
		let focused = self.focused().cloned();
		match (key, focused) {
			(Key::BackTab, _) => {
				self.move_focus(-1);
				Outcome::Redraw
			}
			(Key::Tab, _) => {
				self.move_focus(1);
				Outcome::Redraw
			}
			(Key::Up | Key::Down, Some(Widget::List { check, items, .. })) => {
				let down = key == Key::Down;
				let cur = self.cursors[self.focus];
				if down && cur + 1 < items.len() {
					self.cursors[self.focus] += 1;
					return if check {
						Outcome::Redraw
					} else {
						self.toggle(v)
					};
				}
				if !down && cur > 0 {
					self.cursors[self.focus] -= 1;
					return if check {
						Outcome::Redraw
					} else {
						self.toggle(v)
					};
				}
				self.move_focus(if down { 1 } else { -1 });
				Outcome::Redraw
			}
			(Key::Up | Key::Down, _) => {
				self.move_focus(if key == Key::Down { 1 } else { -1 });
				Outcome::Redraw
			}
			(Key::Left | Key::Right, Some(Widget::Button { .. })) => {
				self.move_focus(if key == Key::Right { 1 } else { -1 });
				Outcome::Redraw
			}
			(Key::Enter, Some(w @ Widget::Button { .. })) => self
				.button_action(&w, strings, v)
				.map_or(Outcome::None, Outcome::Activate),
			(Key::Enter, _) => self
				.default_action(strings, v)
				.map_or(Outcome::None, Outcome::Activate),
			(Key::Char(b' '), Some(Widget::List { .. })) => self.toggle(v),
			(Key::Char(b' '), Some(w @ Widget::Button { .. })) => self
				.button_action(&w, strings, v)
				.map_or(Outcome::None, Outcome::Activate),
			(Key::Char(c), Some(Widget::Field { bind, .. })) if (0x20..0x7f).contains(&c) => {
				let mut s = v.str(&bind).unwrap_or("").to_string();
				if s.len() < 64 {
					s.push(c as char);
					v.set_str(&bind, &s);
					return Outcome::Redraw;
				}
				Outcome::None
			}
			(Key::Backspace, Some(Widget::Field { bind, .. })) => {
				let mut s = v.str(&bind).unwrap_or("").to_string();
				if s.pop().is_some() {
					v.set_str(&bind, &s);
					return Outcome::Redraw;
				}
				Outcome::None
			}
			(Key::Char(c), _) => {
				let up = c.to_ascii_uppercase();
				for w in &self.screen.widgets {
					if let Widget::Button { hotkey, .. } = w
						&& *hotkey != 0 && *hotkey == up
						&& let Some(a) = self.button_action(w, strings, v)
					{
						return Outcome::Activate(a);
					}
				}
				Outcome::None
			}
			_ => Outcome::None,
		}
	}
}

fn frame(cells: &mut Cells, r: Rect, attr: u8) {
	if r.w < 2 || r.h < 2 {
		return;
	}
	let (x0, y0) = (usize::from(r.x), usize::from(r.y));
	let (x1, y1) = (x0 + usize::from(r.w) - 1, y0 + usize::from(r.h) - 1);
	for x in x0 + 1..x1 {
		cells.put(x, y0, "\u{2500}", attr, COLS);
		cells.put(x, y1, "\u{2500}", attr, COLS);
	}
	for y in y0 + 1..y1 {
		cells.put(x0, y, "\u{2502}", attr, COLS);
		cells.put(x1, y, "\u{2502}", attr, COLS);
	}
	cells.put(x0, y0, "\u{250c}", attr, COLS);
	cells.put(x1, y0, "\u{2510}", attr, COLS);
	cells.put(x0, y1, "\u{2514}", attr, COLS);
	cells.put(x1, y1, "\u{2518}", attr, COLS);
}

#[allow(clippy::too_many_arguments)]
fn draw_meter(
	cells: &mut Cells,
	t: &Theme,
	r: Rect,
	attr: u8,
	label: &str,
	source: Source,
	low_below: u8,
	pct: u32,
) {
	let caption = if !label.is_empty() {
		label
	} else {
		match source {
			Source::File => "File",
			Source::FreeSpace => "Free space",
			Source::Total => "Progress",
		}
	};
	let low = source == Source::FreeSpace && pct < u32::from(low_below);
	let tail = if low {
		format!("{pct:>3}% LOW")
	} else {
		format!("{pct:>3}%")
	};
	let w = usize::from(r.w);
	let label_w = caption.chars().count().min(w / 3);
	let caption: String = caption.chars().take(label_w).collect();
	let bar_w = w
		.saturating_sub(label_w + 1 + 2 + 1 + tail.chars().count())
		.max(1);
	let filled = ((bar_w * pct as usize + 50) / 100).min(bar_w);
	let (mut x, y) = (usize::from(r.x), usize::from(r.y));
	let end = x + w;
	cells.put(x, y, &caption, attr, end);
	x += label_w + 1;
	cells.put(x, y, "[", attr, end);
	let bg = attr >> 4;
	let bar = (bg << 4) | if bg == 1 { 3 } else { 1 };
	cells.put(x + 1, y, &"\u{2588}".repeat(filled), bar, end);
	cells.put(
		x + 1 + filled,
		y,
		&"\u{2591}".repeat(bar_w - filled),
		attr,
		end,
	);
	cells.put(x + 1 + bar_w, y, "] ", attr, end);
	let tail_attr = if low {
		(bg << 4) | (t.warn & 0x0f)
	} else {
		attr
	};
	cells.put(x + 3 + bar_w, y, &tail, tail_attr, end);
}

/// The items of a list, for tests and tools.
pub fn item(id: &str, label: &str) -> Item {
	Item {
		id: id.to_string(),
		label: label.to_string(),
		checked: false,
		required: false,
	}
}
