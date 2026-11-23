// Golden cell grids and wire-format tests for `render_cells`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use li_model::Layout;
use li_render_tui::{CellGrid, Colour, Look, MapBindings, Screen, dump, render_cells};
use serde_json::json;

fn layout(name: &str) -> Layout {
	let path = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("tests/fixtures")
		.join(format!("{name}.layout.json"));
	serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Compares `actual` with `tests/golden/<name>.ansi`, escapes written as `\e`;
/// `UPDATE_GOLDEN=1` rewrites the file.
fn golden(name: &str, grid: &CellGrid) {
	let actual = grid.to_ansi().replace('\u{1b}', "\\e");
	let path = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("tests/golden")
		.join(format!("{name}.ansi"));
	if std::env::var_os("UPDATE_GOLDEN").is_some() {
		std::fs::write(&path, actual).unwrap();
		return;
	}
	let expected = std::fs::read_to_string(&path)
		.unwrap_or_else(|_| panic!("missing golden file {}", path.display()));
	assert_eq!(expected, actual, "golden mismatch for {name}");
}

fn vars() -> MapBindings {
	MapBindings::new()
		.with("state.install_dir", json!("/home/me/fizz"))
		.with("meter.free", json!(0.62))
		.with("meter.total", json!(0.42))
		.with("meter.file", json!(0.8))
		.with("meter.file.label", json!("SOUND/SBPRO.DRV"))
		.with("install.size", json!("48 MB"))
		.with("system.free_space", json!("12.4 GB"))
}

fn each_page(f: impl Fn(&str, Look, CellGrid, &Screen)) {
	for page in ["welcome", "components", "copying"] {
		for (look, word) in [(Look::Retro, "retro"), (Look::Plain, "plain")] {
			let screen = Screen::new(&layout(page), look);
			let grid = render_cells(&screen, &vars(), 80, 24);
			f(&format!("cells-{page}-{word}"), look, grid, &screen);
		}
	}
}

#[test]
fn golden_grids() {
	each_page(|name, _, grid, _| golden(name, &grid));
}

#[test]
fn the_grid_is_the_text_dump() {
	each_page(|name, look, grid, screen| {
		assert_eq!(grid.cells.len(), 80 * 24, "{name}");
		assert_eq!(
			grid.to_text(),
			dump(screen, &vars(), 80, 24),
			"{name} {look:?}"
		);
	});
}

#[test]
fn retro_is_white_on_blue_and_plain_has_no_colour() {
	let retro = render_cells(
		&Screen::new(&layout("welcome"), Look::Retro),
		&vars(),
		80,
		24,
	);
	let corner = retro.cell(0, 0).unwrap();
	assert_eq!((corner.fg, corner.bg), (Colour::White, Colour::Blue));
	assert!(retro.to_ansi().contains("\u{1b}["));
	let plain = render_cells(
		&Screen::new(&layout("welcome"), Look::Plain),
		&vars(),
		80,
		24,
	);
	assert!(
		plain
			.cells
			.iter()
			.all(|c| c.fg == Colour::Default && c.bg == Colour::Default)
	);
	assert!(plain.cells.iter().all(|c| c.modifiers.is_empty()));
}

#[test]
fn colours_and_cells_round_trip_on_the_wire() {
	for c in [
		Colour::Default,
		Colour::LightCyan,
		Colour::DarkGray,
		Colour::Indexed(208),
		Colour::Rgb(1, 0x2b, 0xff),
	] {
		let s = serde_json::to_string(&c).unwrap();
		assert_eq!(serde_json::from_str::<Colour>(&s).unwrap(), c, "{s}");
	}
	assert_eq!(
		serde_json::to_string(&Colour::Rgb(1, 0x2b, 0xff)).unwrap(),
		"\"#012bff\""
	);
	let grid = render_cells(
		&Screen::new(&layout("welcome"), Look::Retro),
		&vars(),
		80,
		24,
	);
	let back: CellGrid = serde_json::from_str(&serde_json::to_string(&grid).unwrap()).unwrap();
	assert_eq!(back, grid);
}

#[test]
fn ansi_has_one_line_per_row_and_resets_each_one() {
	let grid = render_cells(
		&Screen::new(&layout("welcome"), Look::Retro),
		&vars(),
		40,
		12,
	);
	let ansi = grid.to_ansi();
	assert_eq!(ansi.lines().count(), 12);
	assert!(ansi.lines().all(|l| l.ends_with("\u{1b}[0m")));
}
