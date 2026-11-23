// The artboard look: the Fizz-DOS setup screen as 80 by 25 text cells.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`flatten_artboard`] draws a `backdrop` layout the way the Fizz-DOS artboard does:
//!
//! ```text
//! row 0        a cyan title bar: the backdrop's `title` left, its `credit` right
//! rows 1 to 23 the `fill` character on blue; on the left a side column (the nodes with
//!              `region = "side"`: title, subtitle, pixel art as half blocks, a block
//!              progress bar), and to its right the shadowed double-line dialog with its
//!              title in the border and `< Button >` buttons
//! row 24       the hint bar (the node with `region = "hints"`): grey, key names in red
//! ```
//!
//! The dialog's inside is drawn by [`Screen::flatten`], so its widgets are the same ones the
//! terminal renderer has; the artboard adds the frame around it. The result is a [`Flat`], text
//! cells with a VGA attribute each plus the widgets that need live values, which the DOS runtime
//! (`li-retro`) draws over them. A layout whose root is not a `backdrop` returns `None`.
//!
//! The focus description a screen reader needs is not part of the grid: ask the runtime's page
//! for it.

use li_model::{Layout, Node};
use serde_json::Value;

use crate::bindings::Bindings;
use crate::flatten::{Flat, FlatCell, Widget};
use crate::glyphs::Look;
use crate::interp::interpolate;
use crate::screen::Screen;

/// Columns the side column takes, counted from the left edge.
pub const SIDE_WIDTH: u16 = 18;

const BASE: u8 = 0x1f;
const TITLE_BAR: u8 = 0x30;
const HINT_BAR: u8 = 0x70;
const HINT_KEY: u8 = 0x74;
const SIDE_TITLE: u8 = 0x1f;
const SIDE_SUBTITLE: u8 = 0x1b;
const SIDE_BAR: u8 = 0x1b;
const SIDE_BAR_EMPTY: u8 = 0x13;

/// The 16 VGA colours, as red, green and blue.
const VGA: [(i32, i32, i32); 16] = [
	(0, 0, 0),
	(0, 0, 170),
	(0, 170, 0),
	(0, 170, 170),
	(170, 0, 0),
	(170, 0, 170),
	(170, 85, 0),
	(170, 170, 170),
	(85, 85, 85),
	(85, 85, 255),
	(85, 255, 85),
	(85, 255, 255),
	(255, 85, 85),
	(255, 85, 255),
	(255, 255, 85),
	(255, 255, 255),
];

/// The VGA colour nearest to `#rrggbb`.
pub fn nearest_vga(hex: &str) -> Option<u8> {
	let h = hex.strip_prefix('#')?;
	if h.len() != 6 {
		return None;
	}
	let c = |i: usize| i32::from_str_radix(&h[i..i + 2], 16).ok();
	let (r, g, b) = (c(0)?, c(2)?, c(4)?);
	VGA.iter()
		.enumerate()
		.min_by_key(|(_, (vr, vg, vb))| (r - vr).pow(2) + (g - vg).pow(2) + (b - vb).pow(2))
		.map(|(i, _)| i as u8)
}

struct Grid {
	w: u16,
	h: u16,
	cells: Vec<FlatCell>,
}

impl Grid {
	fn put(&mut self, x: u16, y: u16, s: &str, attr: u8) {
		for (x, ch) in (x..self.w).zip(s.chars()) {
			self.set(x, y, &ch.to_string(), attr);
		}
	}

	fn set(&mut self, x: u16, y: u16, sym: &str, attr: u8) {
		if x < self.w && y < self.h {
			self.cells[usize::from(y) * usize::from(self.w) + usize::from(x)] = FlatCell {
				symbol: sym.to_owned(),
				attr,
			};
		}
	}

	fn row(&mut self, y: u16, sym: &str, attr: u8) {
		for x in 0..self.w {
			self.set(x, y, sym, attr);
		}
	}
}

fn prop<'a>(n: &'a Node, k: &str) -> Option<&'a str> {
	n.props.get(k).and_then(Value::as_str)
}

fn region(n: &Node) -> Option<&str> {
	prop(n, "region")
}

/// The pixels of an `art` value as VGA colours (`None` is the background), row by row.
fn art_pixels(art: &Value) -> Option<(usize, usize, Vec<Option<u8>>)> {
	let w = art.get("width")?.as_u64()? as usize;
	let h = art.get("height")?.as_u64()? as usize;
	let mut px = vec![None; w * h];
	for r in art.get("rects")?.as_array()? {
		let r = r.as_array()?;
		let n = |i: usize| r.get(i).and_then(Value::as_u64).map(|v| v as usize);
		let colour = nearest_vga(r.get(4)?.as_str()?)?;
		for y in n(1)?..(n(1)? + n(3)?).min(h) {
			for x in n(0)?..(n(0)? + n(2)?).min(w) {
				px[y * w + x] = Some(colour);
			}
		}
	}
	Some((w, h, px))
}

/// Draws the side column and returns nothing: it is part of the grid.
fn side(g: &mut Grid, n: &Node, known: &dyn Bindings) {
	let (x, mut y) = (2u16, 2u16);
	for c in &n.children {
		let c = c.for_renderer("tui-retro");
		match c.kind.as_str() {
			"text" => {
				let text = interpolate(prop(c, "text").unwrap_or_default(), known);
				let attr = match prop(c, "role") {
					Some("title") => SIDE_TITLE,
					_ => SIDE_SUBTITLE,
				};
				g.put(x, y, &text, attr);
				y += 1;
			}
			"image" => {
				if let Some((w, h, px)) = c.props.get("art").and_then(art_pixels) {
					// Two pixel rows to a text row: the upper half block carries the top pixel.
					for ty in 0..h.div_ceil(2) {
						for tx in 0..w {
							let top = px[2 * ty * w + tx];
							let bottom = if 2 * ty + 1 < h {
								px[(2 * ty + 1) * w + tx]
							} else {
								None
							};
							let (sym, attr) = match (top, bottom) {
								(None, None) => (" ", BASE),
								(Some(t), Some(b)) if t == b => ("\u{2588}", (1 << 4) | t),
								(Some(t), b) => ("\u{2580}", (b.unwrap_or(1) << 4) | t),
								(None, Some(b)) => ("\u{2584}", (1 << 4) | b),
							};
							g.set(x + tx as u16, y + ty as u16, sym, attr);
						}
					}
					y += h.div_ceil(2) as u16;
				}
			}
			"progress" => {
				let max = c
					.props
					.get("max")
					.and_then(Value::as_u64)
					.unwrap_or(1)
					.max(1);
				let value = c
					.props
					.get("value")
					.and_then(Value::as_u64)
					.unwrap_or(0)
					.min(max);
				let text = interpolate(prop(c, "text").unwrap_or("Progress"), known);
				g.put(x, y, &text, SIDE_SUBTITLE);
				let width = u64::from(SIDE_WIDTH - 4);
				let filled = (width * value + max / 2) / max;
				for i in 0..width {
					let (s, a) = if i < filled {
						("\u{2588}", SIDE_BAR)
					} else {
						("\u{2591}", SIDE_BAR_EMPTY)
					};
					g.set(x + i as u16, y + 1, s, a);
				}
				y += 2;
			}
			_ => continue,
		}
		y += 1;
	}
}

fn hints(g: &mut Grid, n: &Node, known: &dyn Bindings) {
	let y = g.h - 1;
	g.row(y, " ", HINT_BAR);
	let mut x = 1u16;
	for c in &n.children {
		let c = c.for_renderer("tui-retro");
		let text = interpolate(prop(c, "text").unwrap_or_default(), known);
		if let Some(key) = prop(c, "key") {
			g.put(x, y, key, HINT_KEY);
			x += key.chars().count() as u16 + 1;
		}
		g.put(x, y, &text, HINT_BAR);
		x += text.chars().count() as u16 + 2;
	}
}

/// Draws the layout as the artboard. `known` fills the placeholders whose values are known when
/// compiling, `samples` stands in for the others while measuring (see [`Screen::flatten`]).
pub fn flatten_artboard(
	layout: &Layout,
	known: &dyn Bindings,
	samples: &dyn Bindings,
	width: u16,
	height: u16,
) -> Option<Flat> {
	let root = layout.root.for_renderer("tui-retro");
	if root.kind != "backdrop" {
		return None;
	}
	let dialog = root
		.children
		.iter()
		.map(|c| c.for_renderer("tui-retro"))
		.find(|c| c.kind == "dialog")?;
	let fill = prop(root, "fill").unwrap_or(" ");
	let fill_attr = if fill == " " { BASE } else { 0x19 };
	let mut g = Grid {
		w: width,
		h: height,
		cells: vec![
			FlatCell {
				symbol: fill.chars().next().unwrap_or(' ').to_string(),
				attr: fill_attr,
			};
			usize::from(width) * usize::from(height)
		],
	};
	// The title bar.
	g.row(0, " ", TITLE_BAR);
	let title = interpolate(prop(root, "title").unwrap_or_default(), known);
	g.put(1, 0, &title, TITLE_BAR);
	if let Some(credit) = prop(root, "credit") {
		let n = credit.chars().count() as u16;
		g.put(width.saturating_sub(n + 1), 0, credit, TITLE_BAR);
	}
	for c in &root.children {
		let c = c.for_renderer("tui-retro");
		match region(c) {
			Some("side") => side(&mut g, c, known),
			Some("hints") => hints(&mut g, c, known),
			_ => {}
		}
	}
	if !root.children.iter().any(|c| region(c) == Some("hints")) {
		g.row(height - 1, " ", HINT_BAR);
	}
	// The dialog, drawn on its own to the right of the side column and centred in the rows between
	// the title bar and the hint bar.
	let inner_layout = Layout {
		schema: None,
		version: layout.version,
		id: layout.id.clone(),
		root: dialog.clone(),
	};
	let screen = Screen::new(&inner_layout, Look::Retro);
	let (dx, dy) = (SIDE_WIDTH, 1u16);
	let inner = screen.flatten(known, samples, width - dx, height - 1);
	for y in 0..inner.height.saturating_sub(1) {
		for x in 0..inner.width {
			let c = &inner.cells[usize::from(y) * usize::from(inner.width) + usize::from(x)];
			// Cells the dialog did not draw (blank on the base colours) leave the backdrop showing.
			if c.symbol == " " && c.attr == BASE {
				continue;
			}
			g.set(x + dx, y + dy, &c.symbol, c.attr);
		}
	}
	let widgets = inner
		.widgets
		.into_iter()
		.map(|w| Widget {
			x: w.x + dx,
			y: w.y + dy,
			limit_right: (w.limit_right + dx).min(width),
			..w
		})
		.collect();
	Some(Flat {
		width,
		height,
		cells: g.cells,
		widgets,
	})
}

impl Flat {
	/// The grid as text, one line per row with trailing spaces removed.
	pub fn text(&self) -> String {
		let mut out = String::new();
		for y in 0..usize::from(self.height) {
			let row: String = (0..usize::from(self.width))
				.map(|x| self.cells[y * usize::from(self.width) + x].symbol.as_str())
				.collect();
			out.push_str(row.trim_end());
			out.push('\n');
		}
		out
	}
}
