// The drawn screen as a grid of styled cells, for UIs that draw their own terminal.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt::Write as _;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::text::Span;
use serde::{Deserialize, Serialize};

use crate::bindings::Bindings;
use crate::screen::Screen;

/// A cell colour: the terminal's own default, one of the sixteen named ANSI colours, a
/// 256-colour palette index or true colour.
///
/// On the wire it is one string: `default`, a name such as `blue` or `light_red`,
/// `indexed:208`, or `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Colour {
	#[default]
	Default,
	Black,
	Red,
	Green,
	Yellow,
	Blue,
	Magenta,
	Cyan,
	Gray,
	DarkGray,
	LightRed,
	LightGreen,
	LightYellow,
	LightBlue,
	LightMagenta,
	LightCyan,
	White,
	Indexed(u8),
	Rgb(u8, u8, u8),
}

impl Colour {
	const NAMED: [(&'static str, Colour); 17] = [
		("default", Colour::Default),
		("black", Colour::Black),
		("red", Colour::Red),
		("green", Colour::Green),
		("yellow", Colour::Yellow),
		("blue", Colour::Blue),
		("magenta", Colour::Magenta),
		("cyan", Colour::Cyan),
		("gray", Colour::Gray),
		("dark_gray", Colour::DarkGray),
		("light_red", Colour::LightRed),
		("light_green", Colour::LightGreen),
		("light_yellow", Colour::LightYellow),
		("light_blue", Colour::LightBlue),
		("light_magenta", Colour::LightMagenta),
		("light_cyan", Colour::LightCyan),
		("white", Colour::White),
	];

	fn from_ratatui(c: Color) -> Colour {
		match c {
			Color::Reset => Colour::Default,
			Color::Black => Colour::Black,
			Color::Red => Colour::Red,
			Color::Green => Colour::Green,
			Color::Yellow => Colour::Yellow,
			Color::Blue => Colour::Blue,
			Color::Magenta => Colour::Magenta,
			Color::Cyan => Colour::Cyan,
			Color::Gray => Colour::Gray,
			Color::DarkGray => Colour::DarkGray,
			Color::LightRed => Colour::LightRed,
			Color::LightGreen => Colour::LightGreen,
			Color::LightYellow => Colour::LightYellow,
			Color::LightBlue => Colour::LightBlue,
			Color::LightMagenta => Colour::LightMagenta,
			Color::LightCyan => Colour::LightCyan,
			Color::White => Colour::White,
			Color::Indexed(i) => Colour::Indexed(i),
			Color::Rgb(r, g, b) => Colour::Rgb(r, g, b),
		}
	}

	/// The wire form: `default`, a name, `indexed:N` or `#rrggbb`.
	pub fn to_wire(self) -> String {
		match self {
			Colour::Indexed(i) => format!("indexed:{i}"),
			Colour::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
			named => Colour::NAMED
				.iter()
				.find(|(_, c)| *c == named)
				.map(|(n, _)| (*n).to_owned())
				.unwrap_or_default(),
		}
	}

	pub fn from_wire(s: &str) -> Option<Colour> {
		if let Some(i) = s.strip_prefix("indexed:") {
			return i.parse().ok().map(Colour::Indexed);
		}
		if let Some(hex) = s.strip_prefix('#')
			&& hex.len() == 6
		{
			let n = u32::from_str_radix(hex, 16).ok()?;
			return Some(Colour::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8));
		}
		Colour::NAMED.iter().find(|(n, _)| *n == s).map(|(_, c)| *c)
	}

	/// The SGR parameter for this colour as a foreground (`bg` false) or background.
	fn sgr(self, bg: bool) -> String {
		let base = if bg { 40 } else { 30 };
		let bright = if bg { 100 } else { 90 };
		match self {
			Colour::Default => (base + 9).to_string(),
			Colour::Black => base.to_string(),
			Colour::Red => (base + 1).to_string(),
			Colour::Green => (base + 2).to_string(),
			Colour::Yellow => (base + 3).to_string(),
			Colour::Blue => (base + 4).to_string(),
			Colour::Magenta => (base + 5).to_string(),
			Colour::Cyan => (base + 6).to_string(),
			Colour::Gray => (base + 7).to_string(),
			Colour::DarkGray => bright.to_string(),
			Colour::LightRed => (bright + 1).to_string(),
			Colour::LightGreen => (bright + 2).to_string(),
			Colour::LightYellow => (bright + 3).to_string(),
			Colour::LightBlue => (bright + 4).to_string(),
			Colour::LightMagenta => (bright + 5).to_string(),
			Colour::LightCyan => (bright + 6).to_string(),
			Colour::White => (bright + 7).to_string(),
			Colour::Indexed(i) => format!("{};5;{i}", base + 8),
			Colour::Rgb(r, g, b) => format!("{};2;{r};{g};{b}", base + 8),
		}
	}
}

impl Serialize for Colour {
	fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
		s.serialize_str(&self.to_wire())
	}
}

impl<'de> Deserialize<'de> for Colour {
	fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Colour, D::Error> {
		let s = String::deserialize(d)?;
		Colour::from_wire(&s).ok_or_else(|| serde::de::Error::custom(format!("bad colour `{s}`")))
	}
}

/// Text attributes of a cell. Only the ones set are written on the wire.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Modifiers {
	#[serde(default, skip_serializing_if = "is_false")]
	#[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
	pub bold: bool,
	#[serde(default, skip_serializing_if = "is_false")]
	#[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
	pub dim: bool,
	#[serde(default, skip_serializing_if = "is_false")]
	#[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
	pub italic: bool,
	#[serde(default, skip_serializing_if = "is_false")]
	#[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
	pub underline: bool,
	/// Foreground and background swap. The colours in the cell are as drawn, not swapped.
	#[serde(default, skip_serializing_if = "is_false")]
	#[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
	pub reversed: bool,
}

fn is_false(b: &bool) -> bool {
	!*b
}

impl Modifiers {
	pub fn is_empty(&self) -> bool {
		*self == Modifiers::default()
	}

	fn from_ratatui(m: Modifier) -> Modifiers {
		Modifiers {
			bold: m.contains(Modifier::BOLD),
			dim: m.contains(Modifier::DIM),
			italic: m.contains(Modifier::ITALIC),
			underline: m.contains(Modifier::UNDERLINED),
			reversed: m.contains(Modifier::REVERSED),
		}
	}
}

/// One terminal cell.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
	/// One grapheme, a space for an empty cell, or empty for the right half of a wide
	/// character.
	pub ch: String,
	#[cfg_attr(feature = "ts", ts(type = "string"))]
	pub fg: Colour,
	#[cfg_attr(feature = "ts", ts(type = "string"))]
	pub bg: Colour,
	#[serde(default, skip_serializing_if = "Modifiers::is_empty")]
	#[cfg_attr(feature = "ts", ts(as = "Option<Modifiers>", optional))]
	pub modifiers: Modifiers,
}

/// A rendered screen: `height` rows of `width` cells, row by row.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellGrid {
	pub width: u16,
	pub height: u16,
	pub cells: Vec<Cell>,
}

impl CellGrid {
	pub fn cell(&self, x: u16, y: u16) -> Option<&Cell> {
		(x < self.width && y < self.height)
			.then(|| &self.cells[usize::from(y) * usize::from(self.width) + usize::from(x)])
	}

	fn rows(&self) -> impl Iterator<Item = &[Cell]> {
		self.cells.chunks(usize::from(self.width).max(1))
	}

	/// The rows as plain text, trailing spaces trimmed. The same text as
	/// [`dump`](crate::dump).
	pub fn to_text(&self) -> String {
		let mut out = String::new();
		for row in self.rows() {
			let line: String = row.iter().map(|c| c.ch.as_str()).collect();
			out.push_str(line.trim_end());
			out.push('\n');
		}
		out
	}

	/// The grid as a string with ANSI escape sequences, as a terminal would show it: one
	/// line per row, every row ending in a reset. Cells in the default style carry no
	/// escapes.
	pub fn to_ansi(&self) -> String {
		let mut out = String::new();
		for row in self.rows() {
			let mut current: Option<(Colour, Colour, Modifiers)> = None;
			for c in row {
				let style = (c.fg, c.bg, c.modifiers);
				let plain = style == (Colour::Default, Colour::Default, Modifiers::default());
				if current.is_none() && plain {
					out.push_str(&c.ch);
					continue;
				}
				if current != Some(style) {
					out.push_str("\u{1b}[0");
					let m = c.modifiers;
					for (on, code) in [
						(m.bold, "1"),
						(m.dim, "2"),
						(m.italic, "3"),
						(m.underline, "4"),
						(m.reversed, "7"),
					] {
						if on {
							out.push(';');
							out.push_str(code);
						}
					}
					if c.fg != Colour::Default {
						let _ = write!(out, ";{}", c.fg.sgr(false));
					}
					if c.bg != Colour::Default {
						let _ = write!(out, ";{}", c.bg.sgr(true));
					}
					out.push('m');
					current = Some(style);
				}
				out.push_str(&c.ch);
			}
			if current.is_some() {
				out.push_str("\u{1b}[0m");
			}
			out.push('\n');
		}
		out
	}
}

/// Draws `screen` at `width` by `height` cells and returns the buffer ratatui would send to a
/// terminal, cell for cell.
pub fn render_cells(screen: &Screen, b: &dyn Bindings, width: u16, height: u16) -> CellGrid {
	let area = Rect::new(0, 0, width, height);
	let mut buf = Buffer::empty(area);
	screen.render(area, &mut buf, b);
	let mut cells = Vec::with_capacity(usize::from(width) * usize::from(height));
	for y in 0..height {
		// A wide character covers the next cell, which the terminal never draws.
		let mut covered = 0usize;
		for x in 0..width {
			let cell = &buf[(x, y)];
			let ch = if covered > 0 {
				covered -= 1;
				String::new()
			} else {
				let w = Span::raw(cell.symbol()).width();
				covered = w.saturating_sub(1);
				cell.symbol().to_owned()
			};
			cells.push(Cell {
				ch,
				fg: Colour::from_ratatui(cell.fg),
				bg: Colour::from_ratatui(cell.bg),
				modifiers: Modifiers::from_ratatui(cell.modifier),
			});
		}
	}
	CellGrid {
		width,
		height,
		cells,
	}
}
