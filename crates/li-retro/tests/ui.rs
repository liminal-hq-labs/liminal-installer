// Keyboard handling, drawing and validation of a small hand-made project.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_retro::Key;
use li_retro::model::*;
use li_retro::ui::{Cells, Outcome, Page, Value, Values};

fn rect(x: u8, y: u8, w: u8, h: u8) -> Rect {
	Rect { x, y, w, h }
}

fn item(id: &str, label: &str) -> Item {
	Item {
		id: id.into(),
		label: label.into(),
		checked: false,
		required: id == "core",
	}
}

fn project() -> Project {
	let cells = vec![(b' ', 0x1f); COLS * ROWS];
	let widgets = vec![
		Widget::Text {
			rect: rect(1, 0, 20, 1),
			limit: 60,
			attr: 0x1f,
			align: Align::Left,
			template: "Hello {name}".into(),
		},
		Widget::List {
			rect: rect(1, 2, 30, 3),
			attr: 0x1f,
			check: true,
			label: "Parts".into(),
			bind: "parts".into(),
			items: vec![item("core", "Core"), item("extra", "Extra")],
			default: String::new(),
		},
		Widget::Field {
			rect: rect(1, 6, 30, 1),
			attr: 0x1f,
			label: "Name".into(),
			bind: "name".into(),
			width: 8,
			placeholder: String::new(),
			default: "ann".into(),
		},
		Widget::Button {
			rect: rect(1, 8, 10, 1),
			attr: 0x70,
			label: "Back".into(),
			action: "back".into(),
			default: false,
			hotkey: b'B',
			enabled_when: vec![],
		},
		Widget::Button {
			rect: rect(12, 8, 10, 1),
			attr: 0x70,
			label: "Next".into(),
			action: "next".into(),
			default: true,
			hotkey: b'N',
			enabled_when: vec![],
		},
	];
	Project {
		format: FORMAT,
		id: "t".into(),
		name: "T".into(),
		publisher: "P".into(),
		version: "1".into(),
		target: "dos-x86:tui-retro".into(),
		default_dir: "C:\\T".into(),
		theme: Theme::default(),
		strings: vec![],
		components: vec![],
		flows: vec![],
		screens: vec![Screen {
			id: "s".into(),
			runs: Screen::pack(&cells),
			widgets,
		}],
		files: vec![],
		ops: vec![],
	}
}

fn draw(p: &Page, v: &Values) -> String {
	let mut c = Cells::default();
	p.draw(&[], v, &mut c);
	c.text()
}

#[test]
fn defaults_focus_and_draw() {
	let pr = project();
	pr.validate().unwrap();
	let mut v = Values::default();
	v.set_var("name", "world");
	let p = Page::new(&pr, 0, &mut v, 0);
	let text = draw(&p, &v);
	assert!(
		text.contains("Hello ann") || text.contains("Hello world"),
		"{text}"
	);
	// The checklist starts with only the required item on; the field starts at its default.
	assert_eq!(v.list("parts").unwrap(), &["core".to_string()]);
	assert_eq!(v.str("name"), Some("ann"));
	// The default button has the focus.
	assert!(text.contains("Next, button"), "{text}");
}

#[test]
fn keys_move_toggle_and_type() {
	let pr = project();
	let mut v = Values::default();
	let mut p = Page::new(&pr, 0, &mut v, 0);
	// Shift-Tab ×2 from Next reaches the field, then the checklist.
	assert_eq!(p.key(Key::BackTab, &[], &mut v), Outcome::Redraw);
	assert_eq!(p.key(Key::BackTab, &[], &mut v), Outcome::Redraw);
	assert!(draw(&p, &v).contains("Name, text field"));
	for c in b"!x" {
		p.key(Key::Char(*c), &[], &mut v);
	}
	assert_eq!(v.str("name"), Some("ann!x"));
	p.key(Key::Backspace, &[], &mut v);
	assert_eq!(v.str("name"), Some("ann!"));
	// The checklist: down to Extra, toggle it on and off; the required item cannot be turned off.
	p.key(Key::BackTab, &[], &mut v);
	p.key(Key::Char(b' '), &[], &mut v);
	assert_eq!(v.list("parts").unwrap(), &["core".to_string()]);
	p.key(Key::Down, &[], &mut v);
	p.key(Key::Char(b' '), &[], &mut v);
	assert_eq!(
		v.list("parts").unwrap(),
		&["core".to_string(), "extra".to_string()]
	);
	p.key(Key::Char(b' '), &[], &mut v);
	assert_eq!(v.list("parts").unwrap(), &["core".to_string()]);
	// Down off the end of the list moves on to the field.
	p.key(Key::Down, &[], &mut v);
	assert!(draw(&p, &v).contains("Name, text field"));
}

#[test]
fn buttons_hotkeys_and_escape() {
	let pr = project();
	let mut v = Values::default();
	let mut p = Page::new(&pr, 0, &mut v, 0);
	assert_eq!(
		p.key(Key::Enter, &[], &mut v),
		Outcome::Activate("next".into())
	);
	assert_eq!(
		p.key(Key::Char(b'b'), &[], &mut v),
		Outcome::Activate("back".into())
	);
	assert_eq!(
		p.key(Key::Esc, &[], &mut v),
		Outcome::Activate("back".into())
	);
	assert_eq!(p.key(Key::F(3), &[], &mut v), Outcome::Cancel);
	// In a field Enter takes the default button and letters are text.
	p.key(Key::BackTab, &[], &mut v);
	p.key(Key::BackTab, &[], &mut v);
	assert_eq!(
		p.key(Key::Enter, &[], &mut v),
		Outcome::Activate("next".into())
	);
	assert_eq!(p.key(Key::Char(b'n'), &[], &mut v), Outcome::Redraw);
}

#[test]
fn text_grows_to_its_limit_and_wraps() {
	let pr = project();
	let mut v = Values::default();
	v.set_var("name", &"word ".repeat(30));
	let p = Page::new(&pr, 0, &mut v, 0);
	let text = draw(&p, &v);
	let line = text.lines().next().unwrap();
	// The rectangle is 20 wide but the text may grow to column 60.
	assert!(
		line.chars().count() > 21 && line.chars().count() <= 60,
		"{line}"
	);
}

#[test]
fn the_validator_rejects_broken_projects() {
	let mut pr = project();
	pr.screens[0].runs.push(0);
	assert!(pr.validate().is_err());
	let mut pr = project();
	if let Widget::Text { rect, .. } = &mut pr.screens[0].widgets[0] {
		rect.x = 79;
		rect.w = 5;
	}
	assert!(pr.validate().is_err());
	let mut pr = project();
	pr.flows.push(Flow {
		id: "install".into(),
		start: 3,
		steps: vec![],
	});
	assert!(pr.validate().is_err());
	let mut pr = project();
	pr.format = 99;
	assert!(Project::decode(&pr.encode().unwrap()).is_err());
	assert!(Project::decode(&[1, 2, 3]).is_err());
}

#[test]
fn billboards_advance_on_their_interval() {
	let mut pr = project();
	pr.screens[0].widgets.push(Widget::Billboard {
		rect: rect(1, 12, 30, 4),
		attr: 0x1f,
		slides: vec!["one".into(), "two".into()],
		interval: 2,
	});
	let mut v = Values::default();
	let mut p = Page::new(&pr, 0, &mut v, 100);
	assert!(!p.tick(1000));
	assert!(draw(&p, &v).contains("one"));
	assert!(p.tick(2100));
	assert!(draw(&p, &v).contains("two"));
	assert!(p.tick(4200));
	assert!(draw(&p, &v).contains("one"));
	let _ = Value::Str(String::new());
}
