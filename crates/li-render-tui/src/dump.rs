// Headless text dumps of a screen, for `liminst preview` and snapshot tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::bindings::Bindings;
use crate::screen::Screen;

/// Renders the screen at `width` by `height` cells and returns the rows as text, trailing
/// spaces trimmed. Colour is not part of the dump.
pub fn dump(screen: &Screen, b: &dyn Bindings, width: u16, height: u16) -> String {
	let area = Rect::new(0, 0, width, height);
	let mut buf = Buffer::empty(area);
	screen.render(area, &mut buf, b);
	let mut out = String::new();
	for y in 0..height {
		let mut row = String::new();
		for x in 0..width {
			row.push_str(buf[(x, y)].symbol());
		}
		out.push_str(row.trim_end());
		out.push('\n');
	}
	out
}

/// [`dump`] followed by the list of focusable controls with their accessible names, in tab
/// order.
pub fn dump_with_controls(screen: &Screen, b: &dyn Bindings, width: u16, height: u16) -> String {
	let mut out = dump(screen, b, width, height);
	let controls = screen.controls();
	if !controls.is_empty() {
		out.push_str("Controls, in tab order:\n");
		for (i, c) in controls.iter().enumerate() {
			out.push_str(&format!(
				"  {}. {} ({:?})\n",
				i + 1,
				crate::interp::interpolate(&c.name, b),
				c.kind
			));
		}
	}
	out
}
