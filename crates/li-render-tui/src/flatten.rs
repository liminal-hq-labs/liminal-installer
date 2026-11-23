// Flattening: a screen drawn once into text cells, with its dynamic widgets recorded.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_model::Node;
use ratatui::style::{Color, Modifier, Style};

/// One text cell: the symbol and its VGA text attribute (background in the high nibble).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatCell {
	pub symbol: String,
	pub attr: u8,
}

/// A widget the layout draws from live values, so a flattened screen leaves its rectangle blank:
/// controls, meters, billboards, documents and text with `{placeholders}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Widget {
	pub x: u16,
	pub y: u16,
	pub w: u16,
	pub h: u16,
	/// The right edge of the space the widget may grow into, for text that gets longer.
	pub limit_right: u16,
	/// The surface's attribute behind the widget.
	pub attr: u8,
	/// The tab-order index when the widget is a control.
	pub control: Option<usize>,
	/// The node, interpolated with the values known when flattening and without children.
	pub node: Node,
}

/// A screen as text cells and the widgets to draw over them.
#[derive(Debug, Clone, PartialEq)]
pub struct Flat {
	pub width: u16,
	pub height: u16,
	pub cells: Vec<FlatCell>,
	pub widgets: Vec<Widget>,
}

/// The default attribute: light grey on black.
pub const DEFAULT_ATTR: u8 = 0x07;

fn vga(c: Color) -> Option<u8> {
	Some(match c {
		Color::Reset => return None,
		Color::Black => 0,
		Color::Blue => 1,
		Color::Green => 2,
		Color::Cyan => 3,
		Color::Red => 4,
		Color::Magenta => 5,
		Color::Yellow => 6,
		Color::Gray => 7,
		Color::DarkGray => 8,
		Color::LightBlue => 9,
		Color::LightGreen => 10,
		Color::LightCyan => 11,
		Color::LightRed => 12,
		Color::LightMagenta => 13,
		Color::LightYellow => 14,
		Color::White => 15,
		_ => return None,
	})
}

/// The VGA attribute of a ratatui style. Bold brightens the foreground, as a terminal does. A
/// bright background (bit 7) needs the video card in intensity mode rather than blink mode.
pub fn attr_of(style: Style) -> u8 {
	let mut fg = style.fg.and_then(vga).unwrap_or(DEFAULT_ATTR & 15);
	let bg = style.bg.and_then(vga).unwrap_or(0);
	if style.add_modifier.contains(Modifier::BOLD) {
		fg |= 8;
	}
	(bg << 4) | fg
}

/// The property that carries a node's template props through a measuring pass.
const TEMPLATE_KEY: &str = "\u{1}template";

/// Escapes braces so the stored template is not interpolated along with the node.
fn escape(s: &str) -> String {
	s.replace('{', "\u{2}").replace('}', "\u{3}")
}

fn unescape(s: &str) -> String {
	s.replace('\u{2}', "{").replace('\u{3}', "}")
}

/// Stores each node's template props (`templ`) in the matching node of `sampled`.
pub(crate) fn mark(sampled: &mut Node, templ: &Node) {
	if let Ok(json) = serde_json::to_string(&templ.props) {
		sampled.props.insert(
			TEMPLATE_KEY.to_owned(),
			serde_json::Value::String(escape(&json)),
		);
	}
	for (s, t) in sampled.children.iter_mut().zip(&templ.children) {
		mark(s, t);
	}
}

/// The node with its template props back, and without the marker.
pub(crate) fn template_of(node: &Node) -> Node {
	let mut out = node.clone();
	if let Some(serde_json::Value::String(s)) = out.props.remove(TEMPLATE_KEY)
		&& let Ok(props) = serde_json::from_str(&unescape(&s))
	{
		out.props = props;
	}
	out
}
