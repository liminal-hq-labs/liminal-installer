// Measuring and drawing nodes into a ratatui buffer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cell::Cell;

use li_model::Node;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};

use crate::bindings::{Bindings, get_fraction, get_str};
use crate::glyphs::{Glyphs, Palette};
use crate::screen::{Control, Screen, accessible_name, items_of, prop_str, prop_u16, resolve};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Size {
	w: u16,
	h: u16,
}

struct Ctx<'a> {
	g: Glyphs,
	p: Palette,
	b: &'a dyn Bindings,
	focus: Option<usize>,
	cursors: &'a [usize],
	controls: &'a [Control],
	counter: Cell<usize>,
}

/// A buffer with a clip rectangle, so drawing past an edge is harmless.
struct Painter<'a> {
	buf: &'a mut Buffer,
	clip: Rect,
}

impl Painter<'_> {
	fn put(&mut self, x: i32, y: i32, s: &str, style: Style) {
		if y < 0 || x < 0 {
			return;
		}
		let (mut x, y) = (x as u32, y as u32);
		for ch in s.chars() {
			if x > u32::from(u16::MAX) || y > u32::from(u16::MAX) {
				return;
			}
			let pos = Position::new(x as u16, y as u16);
			if self.clip.contains(pos) {
				let mut tmp = [0u8; 4];
				self.buf[pos]
					.set_symbol(ch.encode_utf8(&mut tmp))
					.set_style(style);
			}
			x += 1;
		}
	}

	fn fill(&mut self, r: Rect, style: Style) {
		let r = r.intersection(self.clip);
		for y in r.top()..r.bottom() {
			for x in r.left()..r.right() {
				self.buf[(x, y)].set_symbol(" ").set_style(style);
			}
		}
	}

	fn restyle(&mut self, r: Rect, style: Style) {
		let r = r.intersection(self.clip);
		for y in r.top()..r.bottom() {
			for x in r.left()..r.right() {
				self.buf[(x, y)].set_style(style);
			}
		}
	}

	fn frame(&mut self, r: Rect, chars: [char; 6], style: Style) {
		if r.width < 2 || r.height < 2 {
			return;
		}
		let [h, v, tl, tr, bl, br] = chars;
		let (x0, y0) = (i32::from(r.x), i32::from(r.y));
		let (x1, y1) = (x0 + i32::from(r.width) - 1, y0 + i32::from(r.height) - 1);
		for x in x0 + 1..x1 {
			self.put(x, y0, &h.to_string(), style);
			self.put(x, y1, &h.to_string(), style);
		}
		for y in y0 + 1..y1 {
			self.put(x0, y, &v.to_string(), style);
			self.put(x1, y, &v.to_string(), style);
		}
		self.put(x0, y0, &tl.to_string(), style);
		self.put(x1, y0, &tr.to_string(), style);
		self.put(x0, y1, &bl.to_string(), style);
		self.put(x1, y1, &br.to_string(), style);
	}
}

fn width_of(s: &str) -> u16 {
	s.chars().count().min(usize::from(u16::MAX)) as u16
}

fn wrap(text: &str, width: u16) -> Vec<String> {
	let width = usize::from(width.max(1));
	let mut lines = Vec::new();
	for para in text.split('\n') {
		let mut line = String::new();
		for word in para.split_whitespace() {
			let wl = word.chars().count();
			let ll = line.chars().count();
			if ll > 0 && ll + 1 + wl > width {
				lines.push(std::mem::take(&mut line));
			}
			if !line.is_empty() {
				line.push(' ');
			}
			line.push_str(word);
		}
		lines.push(line);
	}
	lines
}

fn prop_gap(node: &Node, default: u16) -> u16 {
	prop_u16(node, "gap").unwrap_or(default)
}

fn visible(node: &Node) -> &Node {
	resolve(node)
}

fn is_custom(node: &Node) -> bool {
	node.kind == "custom"
}

/// Stacks run `horizontal` or `row`; anything else is a column.
fn is_horizontal(node: &Node) -> bool {
	matches!(prop_str(node, "direction"), Some("horizontal" | "row"))
}

fn text_of(node: &Node) -> String {
	prop_str(node, "text").unwrap_or_default().to_owned()
}

fn billboard_line(node: &Node, cx: &Ctx) -> String {
	let items: Vec<String> = node
		.props
		.get("items")
		.and_then(|v| v.as_array())
		.map(|a| {
			a.iter()
				.filter_map(|v| v.as_str().map(str::to_owned))
				.collect()
		})
		.unwrap_or_default();
	if items.is_empty() {
		let captions: Vec<String> = node
			.props
			.get("captions")
			.and_then(|v| v.as_array())
			.map(|a| {
				a.iter()
					.map(|c| {
						let part = |k: &str| c.get(k).and_then(|v| v.as_str()).unwrap_or_default();
						format!("{} — {}", part("title"), part("text"))
					})
					.collect()
			})
			.unwrap_or_default();
		if !captions.is_empty() {
			let idx = node
				.bind
				.as_deref()
				.and_then(|k| cx.b.get(k))
				.and_then(|v| v.as_u64())
				.unwrap_or(0) as usize;
			return captions[idx % captions.len()].clone();
		}
		return text_of(node);
	}
	let idx = node
		.bind
		.as_deref()
		.and_then(|k| cx.b.get(k))
		.and_then(|v| v.as_u64())
		.unwrap_or(0) as usize;
	items[idx % items.len()].clone()
}

fn stack_size(children: &[Node], horizontal: bool, gap: u16, max_w: u16, cx: &Ctx) -> Size {
	let mut size = Size::default();
	let mut count = 0u16;
	for child in children.iter().map(visible).filter(|c| !is_custom(c)) {
		let s = measure(child, max_w, cx);
		if horizontal {
			size.w += s.w;
			size.h = size.h.max(s.h);
		} else {
			size.w = size.w.max(s.w);
			size.h += s.h;
		}
		count += 1;
	}
	if count > 1 {
		let total = gap * (count - 1);
		if horizontal {
			size.w += total;
		} else {
			size.h += total;
		}
	}
	size
}

fn grid_columns(node: &Node) -> usize {
	usize::from(prop_u16(node, "columns").unwrap_or(2).max(1))
}

/// Column widths and row heights of a grid.
fn grid_metrics(node: &Node, max_w: u16, cx: &Ctx) -> (Vec<u16>, Vec<u16>) {
	let cols = grid_columns(node);
	let kids: Vec<&Node> = node
		.children
		.iter()
		.map(visible)
		.filter(|c| !is_custom(c))
		.collect();
	let mut widths = vec![0u16; cols];
	let mut heights = vec![0u16; kids.len().div_ceil(cols)];
	for (i, k) in kids.iter().enumerate() {
		let s = measure(k, max_w, cx);
		widths[i % cols] = widths[i % cols].max(s.w);
		heights[i / cols] = heights[i / cols].max(s.h);
	}
	(widths, heights)
}

fn dialog_box(node: &Node, max_w: u16, cx: &Ctx) -> Size {
	let mut inner_max = max_w.saturating_sub(6).max(1);
	if let Some(w) = prop_u16(node, "width") {
		inner_max = inner_max.min(w.saturating_sub(4).max(1));
	}
	let inner = stack_size(&node.children, false, prop_gap(node, 0), inner_max, cx);
	let title = width_of(prop_str(node, "title").unwrap_or_default()) + 4;
	let w = inner
		.w
		.max(title)
		.max(prop_u16(node, "width").unwrap_or(0).saturating_sub(4));
	Size {
		w: w + 4,
		h: inner.h + 2,
	}
}

fn shadow_size(cx: &Ctx) -> Size {
	if cx.g.shadow {
		Size { w: 2, h: 1 }
	} else {
		Size::default()
	}
}

fn list_size(node: &Node) -> Size {
	let items = items_of(node);
	let label = prop_str(node, "label").map_or(0, |_| 1);
	let w = items
		.iter()
		.map(|i| width_of(&i.label) + 6)
		.max()
		.unwrap_or(0)
		.max(prop_str(node, "label").map_or(0, width_of));
	Size {
		w,
		h: items.len() as u16 + label,
	}
}

fn measure(node: &Node, max_w: u16, cx: &Ctx) -> Size {
	match node.kind.as_str() {
		"text" => {
			let lines = wrap(&text_of(node), max_w);
			Size {
				w: lines.iter().map(|l| width_of(l)).max().unwrap_or(0),
				h: lines.len() as u16,
			}
		}
		"image" => Size {
			w: width_of(prop_str(node, "alt").unwrap_or("image")) + 9,
			h: 1,
		},
		"button" => Size {
			w: width_of(&accessible_name(node)) + 4,
			h: 1,
		},
		"checklist" | "radio" => list_size(node),
		"field" | "path" => {
			let label = prop_str(node, "label").map_or(0, |l| width_of(l) + 2);
			Size {
				w: (label + prop_u16(node, "width").unwrap_or(24) + 2).min(max_w.max(1)),
				h: 1,
			}
		}
		"meter" | "progress" => Size {
			w: prop_u16(node, "width").unwrap_or(44).min(max_w.max(1)),
			h: 1,
		},
		"billboard" => {
			let w = max_w.min(prop_u16(node, "width").unwrap_or(44)).max(5);
			let lines = wrap(&billboard_line(node, cx), w - 4);
			Size {
				w,
				h: lines.len() as u16 + 2,
			}
		}
		"stack" => stack_size(
			&node.children,
			is_horizontal(node),
			prop_gap(node, 0),
			max_w,
			cx,
		),
		"grid" => {
			let (widths, heights) = grid_metrics(node, max_w, cx);
			let gap = prop_gap(node, 2);
			let w: u16 = widths.iter().sum::<u16>() + gap * widths.len().saturating_sub(1) as u16;
			Size {
				w,
				h: heights.iter().sum(),
			}
		}
		"dialog" => {
			let b = dialog_box(node, max_w.saturating_sub(shadow_size(cx).w), cx);
			let s = shadow_size(cx);
			Size {
				w: b.w + s.w,
				h: b.h + s.h,
			}
		}
		"window" | "backdrop" => {
			let inner = stack_size(&node.children, false, prop_gap(node, 1), max_w, cx);
			Size {
				w: max_w,
				h: inner.h + 2,
			}
		}
		_ => Size::default(),
	}
}

fn sub(r: Rect, dx: u16, dy: u16, w: u16, h: u16) -> Rect {
	let x = r.x.saturating_add(dx);
	let y = r.y.saturating_add(dy);
	Rect::new(
		x,
		y,
		w.min(r.right().saturating_sub(x)),
		h.min(r.bottom().saturating_sub(y)),
	)
}

fn draw_vertical(
	children: &[Node],
	area: Rect,
	gap: u16,
	centre: bool,
	surface: Style,
	cx: &Ctx,
	pt: &mut Painter,
) {
	let mut y = 0u16;
	for child in children.iter().map(visible) {
		let s = measure(child, area.width, cx);
		let dx = if centre {
			area.width.saturating_sub(s.w) / 2
		} else {
			0
		};
		let r = sub(area, dx, y, s.w, s.h);
		draw(child, r, surface, cx, pt);
		if !is_custom(child) {
			y = y.saturating_add(s.h).saturating_add(gap);
		}
	}
}

fn draw_title(pt: &mut Painter, r: Rect, title: &str, style: Style) {
	if title.is_empty() || r.width < 6 {
		return;
	}
	let t: String = title.chars().take(usize::from(r.width) - 4).collect();
	let x = i32::from(r.x) + (i32::from(r.width) - i32::from(width_of(&t)) - 2) / 2;
	pt.put(x, i32::from(r.y), &format!(" {t} "), style);
}

fn bar_style(surface: Style, look_colour: bool) -> Style {
	if !look_colour {
		return Style::new();
	}
	let fg = if surface.bg == Some(Color::Blue) {
		Color::Cyan
	} else {
		Color::Blue
	};
	surface.fg(fg)
}

fn draw(node: &Node, r: Rect, surface: Style, cx: &Ctx, pt: &mut Painter) {
	// Controls take their place in tab order even when clipped, so draw order and focus order
	// stay in step.
	let control_index = matches!(
		node.kind.as_str(),
		"button" | "checklist" | "radio" | "field" | "path"
	)
	.then(|| {
		let i = cx.counter.get();
		cx.counter.set(i + 1);
		i
	});
	let focused = control_index.is_some() && control_index == cx.focus;
	let colour = surface != Style::new();
	let pt_clip = pt.clip;
	pt.clip = r.intersection(pt_clip);
	match node.kind.as_str() {
		"text" => {
			let align = prop_str(node, "align").unwrap_or("left");
			for (i, line) in wrap(&text_of(node), r.width).iter().enumerate() {
				let pad = match align {
					"centre" | "center" => r.width.saturating_sub(width_of(line)) / 2,
					"right" => r.width.saturating_sub(width_of(line)),
					_ => 0,
				};
				pt.put(
					i32::from(r.x + pad),
					i32::from(r.y) + i as i32,
					line,
					surface,
				);
			}
		}
		"image" => {
			let alt = prop_str(node, "alt").unwrap_or("image");
			pt.put(
				i32::from(r.x),
				i32::from(r.y),
				&format!("[image: {alt}]"),
				surface,
			);
		}
		"button" => {
			let name = accessible_name(node);
			let text = match (focused, colour) {
				(true, false) => format!("[>{name}<]"),
				_ => format!("[ {name} ]"),
			};
			let style = if focused { cx.p.focus } else { surface };
			pt.put(i32::from(r.x), i32::from(r.y), &text, style);
		}
		"checklist" | "radio" => {
			let ctl = &cx.controls[control_index.unwrap_or(0)];
			let mut y = i32::from(r.y);
			if let Some(label) = prop_str(node, "label") {
				pt.put(i32::from(r.x), y, label, surface);
				y += 1;
			}
			let cursor = cx
				.cursors
				.get(control_index.unwrap_or(0))
				.copied()
				.unwrap_or(0);
			let on = if node.kind == "checklist" {
				ctl.checked_ids(cx.b)
			} else {
				ctl.chosen_id(cx.b).into_iter().collect()
			};
			for (i, item) in ctl.items.iter().enumerate() {
				let glyph = match (node.kind == "checklist", on.contains(&item.id)) {
					(true, true) => cx.g.check_on,
					(true, false) => cx.g.check_off,
					(false, true) => cx.g.radio_on,
					(false, false) => cx.g.radio_off,
				};
				let here = focused && cursor == i;
				let line = format!("{}{} {}", if here { ">" } else { " " }, glyph, item.label);
				let row = Rect::new(r.x, y as u16, r.width, 1);
				let style = if here && colour { cx.p.focus } else { surface };
				if here && colour {
					pt.fill(row, style);
				}
				pt.put(i32::from(r.x), y, &line, style);
				y += 1;
			}
		}
		"field" | "path" => {
			let ctl = &cx.controls[control_index.unwrap_or(0)];
			let mut x = i32::from(r.x);
			if let Some(label) = prop_str(node, "label") {
				pt.put(x, i32::from(r.y), &format!("{label}: "), surface);
				x += i32::from(width_of(label)) + 2;
			}
			let width = prop_u16(node, "width").unwrap_or(24);
			let text = ctl.text(cx.b);
			let shown = if text.is_empty() && !focused {
				prop_str(node, "placeholder").unwrap_or_default().to_owned()
			} else {
				text
			};
			let mut shown = if focused { format!("{shown}_") } else { shown };
			let len = shown.chars().count();
			if len > usize::from(width) {
				shown = shown.chars().skip(len - usize::from(width)).collect();
			}
			let padded = format!("{shown:<w$}", w = usize::from(width));
			let style = if !colour {
				Style::new()
			} else if focused {
				cx.p.focus
			} else {
				Style::new().fg(Color::Black).bg(Color::White)
			};
			pt.put(x, i32::from(r.y), "[", surface);
			pt.put(x + 1, i32::from(r.y), &padded, style);
			pt.put(x + 1 + i32::from(width), i32::from(r.y), "]", surface);
		}
		"meter" | "progress" => draw_meter(node, r, surface, cx, pt),
		"billboard" => {
			pt.fill(Rect::new(r.x, r.y, r.width, r.height), surface);
			pt.frame(r, cx.g.frame, surface);
			let lines = wrap(&billboard_line(node, cx), r.width.saturating_sub(4));
			for (i, line) in lines.iter().enumerate() {
				pt.put(
					i32::from(r.x) + 2,
					i32::from(r.y) + 1 + i as i32,
					line,
					surface,
				);
			}
		}
		"stack" => {
			let horizontal = is_horizontal(node);
			let gap = prop_gap(node, 0);
			if horizontal {
				let mut x = 0u16;
				for child in node.children.iter().map(visible) {
					let s = measure(child, r.width, cx);
					draw(child, sub(r, x, 0, s.w, s.h), surface, cx, pt);
					if !is_custom(child) {
						x = x.saturating_add(s.w).saturating_add(gap);
					}
				}
			} else {
				let centre = matches!(prop_str(node, "align"), Some("centre" | "center"));
				draw_vertical(&node.children, r, gap, centre, surface, cx, pt);
			}
		}
		"grid" => {
			let cols = grid_columns(node);
			let (widths, heights) = grid_metrics(node, r.width, cx);
			let gap = prop_gap(node, 2);
			let mut ys = vec![0u16; heights.len()];
			for i in 1..heights.len() {
				ys[i] = ys[i - 1] + heights[i - 1];
			}
			let mut xs = vec![0u16; cols];
			for i in 1..cols {
				xs[i] = xs[i - 1] + widths[i - 1] + gap;
			}
			let kids = node.children.iter().map(visible).filter(|c| !is_custom(c));
			for (i, child) in kids.enumerate() {
				let s = measure(child, r.width, cx);
				draw(
					child,
					sub(r, xs[i % cols], ys[i / cols], s.w, s.h),
					surface,
					cx,
					pt,
				);
			}
		}
		"dialog" => draw_dialog(node, r, cx, pt),
		"window" => {
			pt.fill(r, cx.p.base);
			pt.frame(r, cx.g.frame, cx.p.base);
			draw_title(
				pt,
				r,
				prop_str(node, "title").unwrap_or_default(),
				cx.p.title.patch(cx.p.base),
			);
			let inner = sub(
				r,
				2,
				1,
				r.width.saturating_sub(4),
				r.height.saturating_sub(2),
			);
			draw_vertical(
				&node.children,
				inner,
				prop_gap(node, 1),
				false,
				cx.p.base,
				cx,
				pt,
			);
		}
		"backdrop" => draw_backdrop(node, r, cx, pt),
		_ => {}
	}
	pt.clip = pt_clip;
}

fn draw_dialog(node: &Node, r: Rect, cx: &Ctx, pt: &mut Painter) {
	let s = shadow_size(cx);
	let b = dialog_box(node, r.width.saturating_sub(s.w), cx);
	let bx = Rect::new(r.x, r.y, b.w.min(r.width), b.h.min(r.height));
	// The shadow darkens what is already drawn beneath it.
	if cx.g.shadow {
		let outer = pt.clip;
		pt.clip = Rect::new(
			outer.x,
			outer.y,
			outer.width.max(r.width + 2),
			outer.height.max(r.height + 1),
		)
		.intersection(Rect::new(0, 0, pt.buf.area.right(), pt.buf.area.bottom()));
		pt.restyle(Rect::new(bx.right(), bx.y + 1, 2, bx.height), cx.p.shadow);
		pt.restyle(Rect::new(bx.x + 2, bx.bottom(), bx.width, 1), cx.p.shadow);
		pt.clip = outer;
	}
	pt.fill(bx, cx.p.dialog);
	pt.frame(bx, cx.g.dialog, cx.p.dialog);
	draw_title(
		pt,
		bx,
		prop_str(node, "title").unwrap_or_default(),
		cx.p.title.patch(cx.p.dialog),
	);
	let inner = sub(
		bx,
		2,
		1,
		bx.width.saturating_sub(4),
		bx.height.saturating_sub(2),
	);
	let saved = pt.clip;
	pt.clip = inner.intersection(saved);
	draw_vertical(
		&node.children,
		inner,
		prop_gap(node, 0),
		false,
		cx.p.dialog,
		cx,
		pt,
	);
	pt.clip = saved;
}

fn draw_backdrop(node: &Node, r: Rect, cx: &Ctx, pt: &mut Painter) {
	pt.fill(r, cx.p.base);
	if let Some(title) = prop_str(node, "title") {
		pt.put(
			i32::from(r.x) + 1,
			i32::from(r.y),
			title,
			cx.p.title.patch(cx.p.base),
		);
	}
	let (meters, others): (Vec<&Node>, Vec<&Node>) = node
		.children
		.iter()
		.map(visible)
		.filter(|c| !is_custom(c))
		.partition(|c| c.kind == "meter");
	let meter_rows = meters.len() as u16;
	let top = 2u16.min(r.height);
	let avail = Rect::new(
		r.x + 1,
		r.y + top,
		r.width.saturating_sub(2),
		r.height.saturating_sub(top + meter_rows),
	);
	let gap = prop_gap(node, 1);
	let sizes: Vec<Size> = others.iter().map(|c| measure(c, avail.width, cx)).collect();
	let total: u16 =
		sizes.iter().map(|s| s.h).sum::<u16>() + gap * sizes.len().saturating_sub(1) as u16;
	let mut y = avail.height.saturating_sub(total) / 2;
	for (child, s) in others.iter().zip(&sizes) {
		let dx = avail.width.saturating_sub(s.w) / 2;
		draw(child, sub(avail, dx, y, s.w, s.h), cx.p.base, cx, pt);
		y = y.saturating_add(s.h).saturating_add(gap);
	}
	for (i, m) in meters.iter().enumerate() {
		let row = Rect::new(
			r.x + 1,
			r.bottom().saturating_sub(meter_rows) + i as u16,
			r.width.saturating_sub(2),
			1,
		);
		draw(m, row, cx.p.base, cx, pt);
	}
}

fn draw_meter(node: &Node, r: Rect, surface: Style, cx: &Ctx, pt: &mut Painter) {
	let kind = if node.kind == "progress" {
		"total"
	} else {
		prop_str(node, "meter")
			.or_else(|| prop_str(node, "source"))
			.unwrap_or("total")
	};
	let value = node.bind.as_deref().map_or(0.0, |k| get_fraction(cx.b, k));
	let caption = node
		.bind
		.as_deref()
		.and_then(|k| get_str(cx.b, &format!("{k}.label")))
		.or_else(|| prop_str(node, "label").map(str::to_owned))
		.unwrap_or_else(|| match kind {
			"file" => "File".to_owned(),
			"free_space" => "Free space".to_owned(),
			_ => "Progress".to_owned(),
		});
	let low = kind == "free_space"
		&& value
			< node
				.props
				.get("low_below")
				.and_then(|v| v.as_f64())
				.unwrap_or(0.1);
	let pct = format!("{:>3}%", (value * 100.0).round() as u32);
	let tail = if low { format!("{pct} LOW") } else { pct };
	let label_w = width_of(&caption).min(r.width / 3);
	let caption: String = caption.chars().take(usize::from(label_w)).collect();
	let bar_w = r
		.width
		.saturating_sub(label_w + 1 + 2 + 1 + width_of(&tail))
		.max(1);
	let filled = ((f64::from(bar_w) * value).round() as u16).min(bar_w);
	let mut x = i32::from(r.x);
	let y = i32::from(r.y);
	pt.put(x, y, &caption, surface);
	x += i32::from(label_w) + 1;
	pt.put(x, y, "[", surface);
	let bar = bar_style(surface, surface != Style::new());
	pt.put(
		x + 1,
		y,
		&cx.g.bar_full.to_string().repeat(usize::from(filled)),
		bar,
	);
	pt.put(
		x + 1 + i32::from(filled),
		y,
		&cx.g
			.bar_empty
			.to_string()
			.repeat(usize::from(bar_w - filled)),
		surface,
	);
	pt.put(x + 1 + i32::from(bar_w), y, "] ", surface);
	let tail_style = if low {
		cx.p.warn.bg(surface.bg.unwrap_or(Color::Reset))
	} else {
		surface
	};
	let tail_style = if surface == Style::new() {
		Style::new()
	} else {
		tail_style
	};
	pt.put(x + 3 + i32::from(bar_w), y, &tail, tail_style);
}

impl Screen {
	/// Draws the screen into `area` of `buf`. The last row is a status line naming the focused
	/// control and the keys.
	pub fn render(&self, area: Rect, buf: &mut Buffer, b: &dyn Bindings) {
		let area = area.intersection(buf.area);
		if area.is_empty() {
			return;
		}
		let controls: Vec<Control> = self.controls.iter().map(|c| c.interpolated(b)).collect();
		let cx = Ctx {
			g: self.look.glyphs(),
			p: self.look.palette(),
			b,
			focus: self.focused(),
			cursors: &self.cursors,
			controls: &controls,
			counter: Cell::new(0),
		};
		let mut pt = Painter { buf, clip: area };
		pt.fill(area, cx.p.base);
		let (content, status) = if area.height > 2 {
			(
				Rect::new(area.x, area.y, area.width, area.height - 1),
				Some(Rect::new(area.x, area.bottom() - 1, area.width, 1)),
			)
		} else {
			(area, None)
		};
		let root = &crate::interp::interpolate_node(&self.root, b);
		match root.kind.as_str() {
			"window" | "backdrop" => draw(root, content, cx.p.base, &cx, &mut pt),
			_ => {
				let s = measure(root, content.width.saturating_sub(2), &cx);
				let dx = content.width.saturating_sub(s.w) / 2;
				let dy = content.height.saturating_sub(s.h) / 2;
				draw(
					root,
					sub(content, dx, dy, s.w, s.h),
					cx.p.base,
					&cx,
					&mut pt,
				);
			}
		}
		if let Some(row) = status {
			pt.clip = row;
			pt.fill(row, cx.p.status);
			let line = format!(
				" {}  |  Tab move, Space toggle, Enter choose, Esc cancel",
				self.describe_focus(b)
			);
			pt.put(
				i32::from(row.x),
				i32::from(row.y),
				line.trim_end(),
				cx.p.status,
			);
		}
	}
}
