// The DOS runtime draws what li-render-tui's Retro look draws, apart from the differences listed here.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use li_engine::Project;
use li_render_tui::{Bindings, Look, MapBindings, Screen};
use li_retro::ui::{Cells, Page, Value, Values};
use serde_json::json;

fn fizz_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lt-fizz")
}

/// The values both renderers are given: what Setup knows on its first pass.
fn values(c: &li_retro_build::Compiled) -> Values {
	let mut v = Values::default();
	let p = &c.project;
	for (k, val) in [
		("product.id", p.id.as_str()),
		("product.name", &p.name),
		("product.publisher", &p.publisher),
		("product.version", &p.version),
		("target.id", "dos-x86"),
		("target.renderer", "tui-retro"),
		("install.size", "3 KB"),
		("system.free_space", "64.0 MB"),
		("system.free_space_after", "64.0 MB"),
		("progress.file", "FIZZ.EXE"),
		("progress.percent", "40"),
		("plan.summary", "5 files, 3 KB in 3 components"),
		("sound.detected", "No sound card"),
		("validate.problems", ""),
	] {
		v.set_var(k, val);
	}
	v.set_str("install.path", "C:\\LTFIZZ");
	v.set_str("sound.card", "none");
	v.set_str("licence.accepted", "yes");
	v.state.insert(
		"components.selected".into(),
		Value::List(p.components.iter().map(|c| c.id.clone()).collect()),
	);
	v.file_pct = 50;
	v.total_pct = 40;
	v.free_pct = 99;
	v
}

fn bindings(v: &Values) -> MapBindings {
	let mut b = MapBindings::new();
	for (k, val) in &v.vars {
		b.set(k, json!(val));
	}
	for (k, val) in &v.state {
		match val {
			Value::Str(s) => b.set(k, json!(s)),
			Value::List(l) => b.set(k, json!(l)),
		}
	}
	b.set("engine.meter.file", json!(f64::from(v.file_pct) / 100.0));
	b.set("engine.meter.total", json!(f64::from(v.total_pct) / 100.0));
	b.set(
		"engine.meter.free_space",
		json!(f64::from(v.free_pct) / 100.0),
	);
	b
}

/// Draws like the runtime would on a PC: through code page 437, so an em dash is a hyphen.
fn on_a_pc(text: &str) -> String {
	text.chars()
		.map(|c| li_retro::cp437::decode_byte(li_retro::cp437::encode_char(c)))
		.collect()
}

fn rows(text: &str) -> Vec<String> {
	text.lines().map(on_a_pc).collect()
}

/// What differs on purpose. Every other screen must match cell for cell.
const DOCUMENTED: &[(&str, &str)] = &[
	(
		"licence",
		"the runtime draws the licence text and scrolls it; li-render-tui leaves a `source` text blank",
	),
	(
		"components",
		"the runtime fills a list named `components` with the target's components; li-render-tui draws it empty",
	),
];

#[test]
fn the_runtime_matches_the_tui_retro_look() {
	let project = Project::load(&fizz_dir()).unwrap();
	let c = li_retro_build::compile(&project, "dos-x86", "1.0.0").unwrap();
	let target = project.target("dos-x86").unwrap();
	let comps: Vec<&li_model::Component> = project
		.components
		.components
		.iter()
		.filter(|c| c.applies_to("dos-x86"))
		.collect();
	let cx = li_retro_build::Context {
		project: &project,
		target,
		components: &comps,
		version: "1.0.0",
	};
	let mut v = values(&c);
	let mut compared = 0;
	for (si, s) in c.project.screens.iter().enumerate() {
		if DOCUMENTED.iter().any(|(id, _)| *id == s.id) {
			continue;
		}
		let page = Page::new(&c.project, si, &mut v, 0);
		let mut cells = Cells::default();
		page.draw(&c.project.strings, &v, &mut cells);
		let layout = li_retro_build::tui_layout(&cx, &s.id).unwrap();
		let b = bindings(&v);
		let screen = Screen::new(&layout, Look::Retro);
		let ours = rows(&cells.text());
		let theirs = rows(&li_render_tui::dump(&screen, &b, 80, 25));
		let diff: Vec<usize> = (0..24).filter(|&i| ours.get(i) != theirs.get(i)).collect();
		assert!(
			diff.is_empty(),
			"{}: rows {diff:?} differ\n{}\n---\n{}",
			s.id,
			cells.text(),
			li_render_tui::dump(&screen, &b, 80, 25)
		);
		// The colours, cell by cell, except an access key, which the runtime colours.
		let area = ratatui::layout::Rect::new(0, 0, 80, 25);
		let mut buf = ratatui::buffer::Buffer::empty(area);
		screen.render(area, &mut buf, &b);
		let hot = c.project.theme.hotkey & 0x0f;
		for y in 0..24usize {
			for x in 0..80usize {
				let want = li_render_tui::attr_of(buf[(x as u16, y as u16)].style());
				let got = cells.at[y * 80 + x];
				let access_key = got & 0x0f == hot && got & 0xf0 == want & 0xf0;
				assert!(
					got == want || access_key,
					"{}: attribute at ({x},{y}): runtime {got:02x}, tui {want:02x}",
					s.id
				);
			}
		}
		compared += 1;
	}
	assert!(compared >= 6, "only {compared} screens were compared");
}
