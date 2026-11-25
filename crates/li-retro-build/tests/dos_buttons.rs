// The DOS text screens: one button is lit, it is the one Enter presses, and the row fits its frame.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Every page of every example project (and a Quill checkout named by `QUILL_INSTALLER`) is
//! compiled for DOS and drawn at 80 by 25.

use std::path::{Path, PathBuf};

use li_engine::Project;
use li_retro::Key;
use li_retro::model::{COLS, Widget};
use li_retro::ui::{Cells, Outcome, Page, Values};

fn projects() -> Vec<PathBuf> {
	let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
	let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
		.unwrap()
		.map(|e| e.unwrap().path())
		.filter(|p| p.join("installer/installer.toml").exists())
		.collect();
	if let Some(q) = std::env::var_os("QUILL_INSTALLER") {
		dirs.push(PathBuf::from(q).parent().unwrap().to_path_buf());
	}
	dirs.sort();
	dirs
}

fn values(p: &li_retro::model::Project) -> Values {
	let mut v = Values::default();
	for (k, val) in [
		("product.id", p.id.as_str()),
		("product.name", &p.name),
		("product.publisher", &p.publisher),
		("product.version", &p.version),
		("target.id", "dos-x86"),
		("target.renderer", "tui-retro"),
		("system.os", "dos"),
		("install.path", "C:\\QUILL"),
		("install.size", "1.2 MB"),
		("system.free_space", "12.3 MB"),
		("system.free_space_after", "11.1 MB"),
		("plan.summary", "3 files, 1.2 MB in 2 components"),
	] {
		v.set_var(k, val);
	}
	v.set_str("licence.accepted", "yes");
	v
}

#[test]
fn one_button_is_lit_it_is_the_one_enter_presses_and_the_row_fits() {
	let mut checked = 0;
	for dir in projects() {
		let project = Project::load(&dir).expect("load");
		for t in project
			.installer
			.targets
			.iter()
			.filter(|t| t.renderer == "tui-retro")
		{
			let c = li_retro_build::compile(&project, &t.id, "1.2.3").expect("compile");
			let strings = &c.project.strings;
			for (si, screen) in c.project.screens.iter().enumerate() {
				let mut v = values(&c.project);
				let mut page = Page::new(&c.project, si, &mut v, 0);
				let mut cells = Cells::default();
				page.draw(strings, &v, &mut cells);
				let lit = c.project.theme.button_focus;
				let at = |x: usize, y: usize| cells.at[y * COLS + x];
				let buttons: Vec<_> = screen
					.widgets
					.iter()
					.filter_map(|w| match w {
						Widget::Button {
							rect,
							label,
							action,
							..
						} => Some((rect, label.clone(), action.clone())),
						_ => None,
					})
					.collect();
				if buttons.is_empty() {
					continue;
				}
				let mut lit_actions = Vec::new();
				for (rect, label, action) in &buttons {
					let (x, y) = (usize::from(rect.x), usize::from(rect.y));
					let text = format!("< {label} >");
					let n = text.chars().count();
					let shown: String = (x..x + n.min(COLS - x))
						.map(|i| cells.ch[y * COLS + i] as char)
						.collect();
					assert_eq!(
						shown, text,
						"{dir:?} {} {}: the button is cut off",
						t.id, screen.id
					);
					// A row inside a frame has its closing border beyond the button's end. (A row a
					// layout puts outside any frame has no border to run past.)
					let is_border = |i: usize| matches!(cells.ch[y * COLS + i], 0xb3 | 0xba);
					assert!(
						!(0..x).any(is_border) || (x + n..COLS).any(is_border),
						"{dir:?} {} {}: `{text}` runs past the frame\n{}",
						t.id,
						screen.id,
						(0..25)
							.map(|r| (0..COLS)
								.map(|i| li_retro::cp437::decode_byte(cells.ch[r * COLS + i]))
								.collect::<String>())
							.collect::<Vec<_>>()
							.join("\n")
					);
					if at(x, y) == lit {
						lit_actions.push(action.clone());
					}
				}
				assert_eq!(
					lit_actions.len(),
					1,
					"{dir:?} {} {}: lit buttons {lit_actions:?}",
					t.id,
					screen.id
				);
				match page.key(Key::Enter, strings, &mut v) {
					Outcome::Activate(a) => assert_eq!(
						a, lit_actions[0],
						"{dir:?} {} {}: Enter does not press the lit button",
						t.id, screen.id
					),
					other => panic!("{dir:?} {} {}: Enter gave {other:?}", t.id, screen.id),
				}
				checked += 1;
			}
		}
	}
	assert!(checked > 10);
}

#[test]
fn the_welcome_page_lights_next_and_enter_goes_on() {
	let project = Project::load(&projects()[0]).unwrap();
	let target = project
		.installer
		.targets
		.iter()
		.find(|t| t.renderer == "tui-retro")
		.unwrap();
	let c = li_retro_build::compile(&project, &target.id, "1.2.3").unwrap();
	let si = c
		.project
		.screens
		.iter()
		.position(|s| s.id == "welcome")
		.unwrap();
	let mut v = values(&c.project);
	let mut page = Page::new(&c.project, si, &mut v, 0);
	let mut cells = Cells::default();
	page.draw(&c.project.strings, &v, &mut cells);
	let lit = c.project.theme.button_focus;
	let row = |label: &str| {
		c.project.screens[si]
			.widgets
			.iter()
			.find_map(|w| match w {
				Widget::Button { rect, label: l, .. } if l == label => {
					Some(usize::from(rect.y) * COLS + usize::from(rect.x))
				}
				_ => None,
			})
			.unwrap()
	};
	assert_eq!(cells.at[row("Next")], lit);
	assert_ne!(cells.at[row("Cancel")], lit);
	assert_eq!(
		page.key(Key::Enter, &c.project.strings, &mut v),
		Outcome::Activate("next".into())
	);
	// Tab and the arrow keys move the light.
	page.key(Key::Tab, &c.project.strings, &mut v);
	page.draw(&c.project.strings, &v, &mut cells);
	assert_eq!(cells.at[row("Cancel")], lit);
	assert_eq!(
		page.key(Key::Enter, &c.project.strings, &mut v),
		Outcome::Activate("cancel".into())
	);
	page.key(Key::Left, &c.project.strings, &mut v);
	page.draw(&c.project.strings, &v, &mut cells);
	assert_eq!(cells.at[row("Next")], lit);
}
