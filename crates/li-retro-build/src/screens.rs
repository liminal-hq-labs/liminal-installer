// Flattens a layout into a runtime screen: cells from li-render-tui's Retro look, widgets from its recorder.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_engine::Project;
use li_model::{Component, Layout, Node, Target};
use li_render_tui::{Look, MapBindings, Screen as TuiScreen, Widget as Rec};
use li_retro::cp437;
use li_retro::model::{self, Align, COLS, Item, ROWS, Rect, Source, Widget};
use serde_json::{Value, json};

use crate::expr::Strings;
use crate::{CompileError, err};

/// One screen: a layout drawn for a step, since a stage shows different nodes on different steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenKey {
	pub layout: String,
	pub step: String,
	pub kind: String,
}

/// What a screen is compiled for.
pub struct Context<'a> {
	pub project: &'a Project,
	pub target: &'a Target,
	pub components: &'a [&'a Component],
	pub version: &'a str,
}

/// A copy of the tree with the `tui-retro` (else `tui`) override applied at every level.
fn resolve(n: &Node, renderer: &str) -> Node {
	let base = n
		.overrides
		.get(renderer)
		.or_else(|| n.overrides.get("tui-retro"))
		.or_else(|| n.overrides.get("tui"))
		.unwrap_or(n);
	Node {
		kind: base.kind.clone(),
		bind: base.bind.clone(),
		props: base.props.clone(),
		children: base.children.iter().map(|c| resolve(c, renderer)).collect(),
		overrides: Default::default(),
	}
}

/// Gives a list the items of the target's components when its `items` is the name `components`.
fn expand_lists(n: &mut Node, components: &[&Component]) {
	if matches!(n.kind.as_str(), "checklist" | "radio")
		&& n.props.get("items").and_then(Value::as_str) == Some("components")
	{
		let items: Vec<Value> = components
			.iter()
			.map(
				|c| json!({ "id": c.id, "label": c.name, "checked": true, "required": c.required }),
			)
			.collect();
		n.props.insert("items".into(), Value::Array(items));
	}
	for c in &mut n.children {
		expand_lists(c, components);
	}
}

fn str_of<'a>(n: &'a Node, k: &str) -> Option<&'a str> {
	n.props.get(k).and_then(Value::as_str)
}

fn rect_of(r: &Rec) -> Rect {
	let x = r.x.min(COLS as u16 - 1);
	let y = r.y.min(ROWS as u16 - 1);
	Rect {
		x: x as u8,
		y: y as u8,
		w: r.w.min(COLS as u16 - x) as u8,
		h: r.h.min(ROWS as u16 - y) as u8,
	}
}

fn name_of(n: &Node) -> String {
	["name", "label", "text"]
		.iter()
		.find_map(|k| str_of(n, k))
		.map(str::to_owned)
		.or_else(|| n.bind.clone())
		.unwrap_or_else(|| n.kind.clone())
}

fn items_of(n: &Node) -> Vec<Item> {
	let Some(arr) = n
		.props
		.get("items")
		.or_else(|| n.props.get("options"))
		.and_then(Value::as_array)
	else {
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

fn slides_of(n: &Node) -> Vec<String> {
	let strings = |k: &str| -> Vec<String> {
		n.props
			.get(k)
			.and_then(Value::as_array)
			.map(|a| {
				a.iter()
					.filter_map(|v| v.as_str().map(str::to_owned))
					.collect()
			})
			.unwrap_or_default()
	};
	let items = strings("items");
	if !items.is_empty() {
		return items;
	}
	n.props
		.get("captions")
		.and_then(Value::as_array)
		.map(|a| {
			a.iter()
				.map(|c| {
					let part = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or_default();
					format!("{} — {}", part("title"), part("text"))
				})
				.collect()
		})
		.unwrap_or_default()
}

fn widget(
	cx: &Context,
	layout: &str,
	rec: &Rec,
	strings: &mut Strings,
) -> Result<Widget, CompileError> {
	let n = &rec.node;
	let rect = rect_of(rec);
	let attr = rec.attr;
	let loc = format!("layout {layout}");
	Ok(match n.kind.as_str() {
		"text" if n.props.contains_key("source") => {
			let src = str_of(n, "source").unwrap_or_default();
			let path = cx.project.dir.join(src);
			let body = std::fs::read_to_string(&path).map_err(|e| {
				CompileError::Message(format!("{loc}: cannot read the document `{src}`: {e}"))
			})?;
			Widget::Doc {
				rect,
				attr,
				text: body.replace("\r\n", "\n").trim_end().to_owned(),
			}
		}
		"text" => Widget::Text {
			rect,
			limit: rec.limit_right.min(COLS as u16) as u8,
			attr,
			align: match str_of(n, "align") {
				Some("centre" | "center") => Align::Centre,
				Some("right") => Align::Right,
				_ => Align::Left,
			},
			template: str_of(n, "text").unwrap_or_default().to_owned(),
		},
		"button" => {
			let attr = model::Theme::default().button;
			let label = name_of(n);
			let action = str_of(n, "action")
				.or_else(|| str_of(n, "id"))
				.map(str::to_owned)
				.or_else(|| n.bind.clone())
				.unwrap_or_else(|| label.to_lowercase());
			let enabled_when = match str_of(n, "enabled_when") {
				Some(src) => strings.compile(src, &format!("{loc}, button `{label}`"))?,
				None => Vec::new(),
			};
			Widget::Button {
				rect,
				attr,
				action,
				default: n
					.props
					.get("default")
					.and_then(Value::as_bool)
					.unwrap_or(false),
				hotkey: str_of(n, "hotkey")
					.and_then(|h| h.chars().next())
					.map_or(0, |c| c.to_ascii_uppercase() as u8),
				enabled_when,
				label,
			}
		}
		"checklist" | "radio" => Widget::List {
			rect,
			attr,
			check: n.kind == "checklist",
			label: str_of(n, "label").unwrap_or_default().to_owned(),
			bind: n.bind.clone().unwrap_or_else(|| name_of(n)),
			items: items_of(n),
			default: str_of(n, "default").unwrap_or_default().to_owned(),
		},
		"field" | "path" => Widget::Field {
			rect,
			attr,
			label: str_of(n, "label").unwrap_or_default().to_owned(),
			bind: n.bind.clone().unwrap_or_else(|| name_of(n)),
			width: n
				.props
				.get("width")
				.and_then(Value::as_u64)
				.unwrap_or(24)
				.min(60) as u8,
			placeholder: str_of(n, "placeholder").unwrap_or_default().to_owned(),
			default: str_of(n, "default").unwrap_or_default().to_owned(),
		},
		"meter" | "progress" => {
			let source = if n.kind == "progress" {
				Source::Total
			} else {
				match str_of(n, "meter").or_else(|| str_of(n, "source")) {
					Some("file") => Source::File,
					Some("free_space") => Source::FreeSpace,
					_ => Source::Total,
				}
			};
			let low = n
				.props
				.get("low_below")
				.and_then(Value::as_f64)
				.unwrap_or(0.1);
			Widget::Meter {
				rect,
				attr,
				label: str_of(n, "label").unwrap_or_default().to_owned(),
				source,
				low_below: (low * 100.0).round().clamp(0.0, 100.0) as u8,
			}
		}
		"billboard" => Widget::Billboard {
			rect,
			attr,
			slides: slides_of(n),
			interval: n
				.props
				.get("interval_seconds")
				.and_then(Value::as_u64)
				.unwrap_or(6)
				.clamp(1, 255) as u8,
		},
		other => {
			return err(format!(
				"{loc}: the node kind `{other}` cannot be drawn by the DOS runtime"
			));
		}
	})
}

/// Stand-ins of a typical length for the values only the running installer knows, so a layout
/// leaves room for them.
fn samples() -> MapBindings {
	MapBindings::new()
		.with("install.path", json!("C:\\GAMES\\FIZZ"))
		.with("install.size", json!("12.3 MB"))
		.with("system.free_space", json!("123.4 MB"))
		.with("system.free_space_after", json!("123.4 MB"))
		.with("progress.file", json!("SOUND\\ADLIB.DRV"))
		.with("progress.percent", json!("100"))
		.with("plan.summary", json!("12 files, 12.3 MB in 3 components"))
		.with("sound.detected", json!("Sound Blaster"))
		.with("validate.problems", json!(""))
}

/// The layout as the compiler draws it: the `tui-retro` overrides applied and the `components`
/// lists filled in. li-render-tui can draw this directly, which is how the tests compare.
///
/// The page is composed with its theme's stage first, as the runner and the Studio do
/// (`Project::screen_for`), so `step` and `kind` (`page`, `commit`, `finish` and so on) choose which
/// stage nodes show.
pub fn tui_layout(cx: &Context, id: &str, step: &str, kind: &str) -> Result<Layout, CompileError> {
	let layout = cx
		.project
		.screen_for(cx.target, id, step, kind)
		.ok_or_else(|| {
			CompileError::Message(format!(
				"layout `{id}` was not found for target `{}`",
				cx.target.id
			))
		})?;
	let mut root = resolve(&layout.root, &cx.target.renderer);
	expand_lists(&mut root, cx.components);
	Ok(Layout {
		schema: None,
		version: layout.version,
		id: layout.id.clone(),
		root,
	})
}

/// The heading of a page: the title of its first `dialog`, else the root's own.
fn page_title(root: &Node) -> Option<String> {
	fn dialog(n: &Node) -> Option<String> {
		if n.kind == "dialog"
			&& let Some(t) = str_of(n, "title")
		{
			return Some(t.to_owned());
		}
		n.children.iter().find_map(dialog)
	}
	dialog(root).or_else(|| str_of(root, "title").map(str::to_owned))
}

/// The text roles a Windows 3.1 screen carries in a `Text` widget's `attr` (it has no VGA colours).
pub mod role {
	pub const BODY: u8 = 0;
	pub const HEADING: u8 = 1;
	pub const CAPTION: u8 = 2;
	pub const SUMMARY: u8 = 3;
}

fn role_of(n: &Node) -> u8 {
	match str_of(n, "role") {
		Some("heading" | "title") => role::HEADING,
		Some("caption") => role::CAPTION,
		Some("summary") => role::SUMMARY,
		_ => role::BODY,
	}
}

/// Collects the widgets of a layout in reading order, for a front end that draws its own
/// controls and has no use for the text cells. The side column, the hint bar and pictures belong
/// to the DOS artboard and are left out; every text node is kept, with its role, so a page keeps
/// its explanations (the recorder behind the cells only keeps text that has live values).
fn collect(
	cx: &Context,
	layout: &str,
	step_kind: &str,
	n: &Node,
	strings: &mut Strings,
	out: &mut Vec<Widget>,
) -> Result<(), CompileError> {
	match str_of(n, "region") {
		Some("side" | "hints") => return Ok(()),
		// The stage's status meters are for the page that copies.
		Some("status") if step_kind != "commit" => return Ok(()),
		_ => {}
	}
	match n.kind.as_str() {
		"window" | "stack" | "grid" | "dialog" | "backdrop" | "slot" | "custom" => {
			for c in &n.children {
				collect(cx, layout, step_kind, c, strings, out)?;
			}
		}
		"image" => {}
		_ => {
			let rec = Rec {
				x: 0,
				y: 0,
				w: 0,
				h: 0,
				limit_right: 0,
				attr: 0,
				control: None,
				node: Node {
					kind: n.kind.clone(),
					bind: n.bind.clone(),
					props: n.props.clone(),
					children: Vec::new(),
					overrides: Default::default(),
				},
			};
			let mut w = widget(cx, layout, &rec, strings)?;
			if let Widget::Text { attr, template, .. } = &mut w {
				if template.is_empty() {
					return Ok(());
				}
				*attr = role_of(n);
			}
			out.push(w);
		}
	}
	Ok(())
}

/// Compiles the layout `id` for Windows 3.1: widgets in reading order and no text cells.
fn compile_win16(
	cx: &Context,
	key: &ScreenKey,
	id: &str,
	strings: &mut Strings,
) -> Result<model::Screen, CompileError> {
	let layout = tui_layout(cx, &key.layout, &key.step, &key.kind)?;
	let mut widgets = Vec::new();
	collect(cx, id, &key.kind, &layout.root, strings, &mut widgets)?;
	Ok(model::Screen {
		id: id.to_owned(),
		title: page_title(&layout.root).unwrap_or_default(),
		runs: model::Screen::pack(&vec![(b' ', 0x07); COLS * ROWS]),
		widgets,
	})
}

/// Flattens the layout `id` for the target.
pub fn compile(
	cx: &Context,
	key: &ScreenKey,
	id: &str,
	strings: &mut Strings,
	warnings: &mut Vec<String>,
) -> Result<model::Screen, CompileError> {
	if cx.target.renderer == "win16" {
		return compile_win16(cx, key, id, strings);
	}
	let flat_layout = tui_layout(cx, &key.layout, &key.step, &key.kind)?;
	let screen = TuiScreen::new(&flat_layout, Look::Retro);
	let product = &cx.project.installer.product;
	let statics = MapBindings::new()
		.with("product.id", json!(product.id))
		.with("product.name", json!(product.name))
		.with("product.publisher", json!(product.publisher))
		.with("product.version", json!(cx.version))
		.with("target.id", json!(cx.target.id))
		.with("target.renderer", json!(cx.target.renderer));
	// A `backdrop` layout is drawn as the Fizz-DOS artboard; any other root as the plain Retro look.
	let flat = li_render_tui::flatten_artboard(
		&flat_layout,
		&statics,
		&samples(),
		COLS as u16,
		ROWS as u16,
	)
	.unwrap_or_else(|| screen.flatten(&statics, &samples(), COLS as u16, ROWS as u16));
	let mut cells = Vec::with_capacity(COLS * ROWS);
	for c in &flat.cells {
		let ch = c.symbol.chars().next().unwrap_or(' ');
		if cx.target.renderer == "tui-retro" && !cp437::has_glyph(ch) && ch != ' ' {
			warnings.push(format!(
				"layout {id}: `{ch}` has no glyph in code page 437 and is drawn as `?`"
			));
		}
		cells.push((cp437::encode_char(ch), c.attr));
	}
	let mut widgets = Vec::new();
	for rec in &flat.widgets {
		widgets.push(widget(cx, id, rec, strings)?);
	}
	warnings.sort();
	warnings.dedup();
	let title = page_title(&flat_layout.root).unwrap_or_default();
	Ok(model::Screen {
		id: id.to_owned(),
		title,
		runs: model::Screen::pack(&cells),
		widgets,
	})
}
