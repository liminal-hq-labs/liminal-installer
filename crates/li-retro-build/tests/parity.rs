// The DOS runtime draws the Fizz-DOS artboard: the structure of each screen, checked cell by cell.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use li_engine::Project;
use li_retro::ui::{Cells, Page, Value, Values};

fn fizz_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lt-fizz")
}

fn draw(c: &li_retro_build::Compiled, id: &str) -> Cells {
	let si = c.project.screens.iter().position(|s| s.id == id).unwrap();
	let mut v = Values::default();
	v.set_var("product.name", &c.project.name);
	v.set_str("install.path", "C:\\LTFIZZ");
	v.set_var("install.size", "3 KB");
	v.set_var("system.free_space", "64.0 MB");
	v.state.insert(
		"components.selected".into(),
		Value::List(c.project.components.iter().map(|c| c.id.clone()).collect()),
	);
	let page = Page::new(&c.project, si, &mut v, 0);
	let mut cells = Cells::default();
	page.draw(&c.project.strings, &v, &mut cells);
	cells
}

fn row(c: &Cells, y: usize) -> String {
	(0..80)
		.map(|x| li_retro::cp437::decode_byte(c.ch[y * 80 + x]))
		.collect()
}

fn attr(c: &Cells, x: usize, y: usize) -> u8 {
	c.at[y * 80 + x]
}

#[test]
fn the_welcome_screen_has_the_artboard_structure() {
	let project = Project::load(&fizz_dir()).unwrap();
	let c = li_retro_build::compile(&project, "dos-x86", "1.0.0").unwrap();
	let s = draw(&c, "welcome");
	// Row 0: the cyan title bar, product left and credit right.
	assert!(
		row(&s, 0).starts_with(" Lieutenant Fizz - Episode One  Setup"),
		"{}",
		row(&s, 0)
	);
	assert!(row(&s, 0).trim_end().ends_with("Liminal Installer"));
	assert!((0..80).all(|x| attr(&s, x, 0) == 0x30));
	// The side column: title, subtitle, art in half blocks, the step bar.
	let text = s.text();
	for want in ["Lt. Fizz", "Episode One", "Step 1 of 8"] {
		assert!(
			text.lines()
				.any(|l| l.chars().take(20).collect::<String>().contains(want)),
			"{want}\n{text}"
		);
	}
	let side: String = (0..25)
		.flat_map(|y| (0..18).map(move |x| (x, y)))
		.map(|(x, y)| li_retro::cp437::decode_byte(s.ch[y * 80 + x]))
		.collect();
	assert!(
		side.contains('\u{2580}') || side.contains('\u{2588}'),
		"the character is drawn in blocks"
	);
	assert!(
		side.contains('\u{2588}') && side.contains('\u{2591}'),
		"the step bar"
	);
	// The dialog: double border with the title in the top border, a drop shadow, `< Next >` buttons.
	let top = (1..24)
		.find(|&y| {
			row(&s, y).contains("Welcome \u{2550}") || row(&s, y).contains("\u{2550} Welcome")
		})
		.expect("a top border");
	assert!(row(&s, top).contains('\u{2554}') && row(&s, top).contains('\u{2557}'));
	let right = row(&s, top)
		.chars()
		.collect::<Vec<_>>()
		.iter()
		.rposition(|&c| c == '\u{2557}')
		.unwrap();
	assert_eq!(
		attr(&s, right + 1, top + 1),
		0x08,
		"the shadow at the right edge"
	);
	assert!(text.contains("< Next >") && text.contains("< Cancel >"));
	let bx = text
		.lines()
		.find(|l| l.contains("< Next >"))
		.map(|l| l.split("< Next >").next().unwrap().chars().count())
		.unwrap();
	let by = text.lines().position(|l| l.contains("< Next >")).unwrap();
	assert_eq!(attr(&s, bx + 2, by), 0x1f, "the focused default button");
	assert_eq!(attr(&s, bx + 12, by), 0x31, "Cancel is blue on cyan");
	// Row 24: the hint bar, with the key names in red.
	assert_eq!(
		row(&s, 24).trim_end(),
		" Enter Continue  F1 Help  Esc Quit setup"
	);
	assert_eq!(attr(&s, 1, 24), 0x74);
	assert_eq!(attr(&s, 7, 24), 0x70);
	// The focus description is not on the grid.
	assert!(!text.contains("Tab move"));
}

#[test]
fn the_step_bar_follows_the_step() {
	let project = Project::load(&fizz_dir()).unwrap();
	let c = li_retro_build::compile(&project, "dos-x86", "1.0.0").unwrap();
	let filled = |id: &str| {
		let s = draw(&c, id);
		(0..80 * 25)
			.filter(|&i| s.ch[i] == 0xdb && s.at[i] == 0x1b && i % 80 < 18)
			.count()
	};
	assert!(filled("welcome") < filled("components"));
	assert!(filled("components") < filled("done"));
}
