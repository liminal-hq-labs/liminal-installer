// Golden-text and keyboard tests for the tui renderer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use li_model::Layout;
use li_render_tui::{
	Bindings, ControlKind, Look, MapBindings, Outcome, Screen, dump, dump_with_controls,
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

fn layout(name: &str) -> Layout {
	let path = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("tests/fixtures")
		.join(format!("{name}.layout.json"));
	serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Compares `actual` with `tests/golden/<name>.txt`; `UPDATE_GOLDEN=1` rewrites the file.
fn golden(name: &str, actual: &str) {
	let path = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("tests/golden")
		.join(format!("{name}.txt"));
	if std::env::var_os("UPDATE_GOLDEN").is_some() {
		std::fs::write(&path, actual).unwrap();
		return;
	}
	let expected = std::fs::read_to_string(&path)
		.unwrap_or_else(|_| panic!("missing golden file {}", path.display()));
	assert_eq!(expected, actual, "golden mismatch for {name}");
}

fn key(code: KeyCode) -> KeyEvent {
	KeyEvent::new(code, KeyModifiers::NONE)
}

fn welcome_state() -> MapBindings {
	MapBindings::new()
		.with("state.install_dir", json!("/home/me/fizz"))
		.with("meter.free", json!(0.62))
}

#[test]
fn welcome_retro() {
	let screen = Screen::new(&layout("welcome"), Look::Retro);
	golden(
		"welcome-retro",
		&dump_with_controls(&screen, &welcome_state(), 80, 24),
	);
}

#[test]
fn welcome_plain() {
	let screen = Screen::new(&layout("welcome"), Look::Plain);
	golden(
		"welcome-plain",
		&dump_with_controls(&screen, &welcome_state(), 80, 24),
	);
}

#[test]
fn copying_retro_midway() {
	let screen = Screen::new(&layout("copying"), Look::Retro);
	let state = MapBindings::new()
		.with("meter.total", json!(0.42))
		.with("meter.file", json!(0.8))
		.with("meter.file.label", json!("SOUND/SBPRO.DRV"))
		.with("meter.free", json!(0.06))
		.with("billboard.index", json!(1));
	golden("copying-retro", &dump(&screen, &state, 80, 24));
}

#[test]
fn copying_plain_midway() {
	let screen = Screen::new(&layout("copying"), Look::Plain);
	let state = MapBindings::new()
		.with("meter.total", json!(0.42))
		.with("meter.file", json!(0.8))
		.with("meter.free", json!(0.5));
	golden("copying-plain", &dump(&screen, &state, 80, 24));
}

#[test]
fn small_terminals_do_not_panic() {
	for look in [Look::Retro, Look::Plain] {
		let screen = Screen::new(&layout("welcome"), look);
		for (w, h) in [(1, 1), (10, 3), (30, 8), (200, 60)] {
			let _ = dump(&screen, &welcome_state(), w, h);
		}
	}
}

#[test]
fn controls_have_accessible_names_in_tab_order() {
	let screen = Screen::new(&layout("welcome"), Look::Retro);
	let names: Vec<_> = screen
		.controls()
		.into_iter()
		.map(|c| (c.name, c.kind))
		.collect();
	assert_eq!(
		names,
		vec![
			("Folder".to_owned(), ControlKind::Path),
			("Components".to_owned(), ControlKind::Checklist),
			("Install for".to_owned(), ControlKind::Radio),
			("Back".to_owned(), ControlKind::Button),
			("Next".to_owned(), ControlKind::Button),
			("Cancel".to_owned(), ControlKind::Button),
		]
	);
}

#[test]
fn default_button_starts_focused_and_enter_activates_it() {
	let mut screen = Screen::new(&layout("welcome"), Look::Retro);
	let mut state = welcome_state();
	assert_eq!(screen.describe_focus(&state), "Next, button".to_owned());
	assert_eq!(
		screen.handle_key(key(KeyCode::Enter), &mut state),
		Outcome::Activate("next".into())
	);
}

#[test]
fn tab_wraps_and_shift_tab_goes_back() {
	let mut screen = Screen::new(&layout("welcome"), Look::Plain);
	let mut state = welcome_state();
	screen.handle_key(key(KeyCode::Tab), &mut state);
	assert_eq!(screen.describe_focus(&state), "Cancel, button");
	screen.handle_key(key(KeyCode::Tab), &mut state);
	assert!(screen.describe_focus(&state).starts_with("Folder"));
	screen.handle_key(
		KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
		&mut state,
	);
	assert_eq!(screen.describe_focus(&state), "Cancel, button");
}

#[test]
fn escape_cancels() {
	let mut screen = Screen::new(&layout("welcome"), Look::Plain);
	let mut state = welcome_state();
	assert_eq!(
		screen.handle_key(key(KeyCode::Esc), &mut state),
		Outcome::Cancel
	);
}

#[test]
fn typing_edits_the_bound_path() {
	let mut screen = Screen::new(&layout("welcome"), Look::Plain);
	let mut state = welcome_state();
	screen.focus_index(0);
	for c in "/x".chars() {
		screen.handle_key(key(KeyCode::Char(c)), &mut state);
	}
	assert_eq!(
		state.get("state.install_dir"),
		Some(&json!("/home/me/fizz/x"))
	);
	assert_eq!(
		screen.handle_key(key(KeyCode::Backspace), &mut state),
		Outcome::Changed("state.install_dir".into())
	);
	assert_eq!(
		state.get("state.install_dir"),
		Some(&json!("/home/me/fizz/"))
	);
}

#[test]
fn checklist_toggles_with_space_and_keeps_required_items() {
	let mut screen = Screen::new(&layout("welcome"), Look::Plain);
	let mut state = welcome_state();
	screen.focus_index(1);
	// Cursor starts on the required item, which cannot be unchecked.
	assert_eq!(
		screen.handle_key(key(KeyCode::Char(' ')), &mut state),
		Outcome::None
	);
	screen.handle_key(key(KeyCode::Down), &mut state);
	assert_eq!(
		screen.handle_key(key(KeyCode::Char(' ')), &mut state),
		Outcome::Changed("state.components".into())
	);
	assert_eq!(state.get("state.components"), Some(&json!(["game"])));
	screen.handle_key(key(KeyCode::Down), &mut state);
	screen.handle_key(key(KeyCode::Char(' ')), &mut state);
	assert_eq!(
		state.get("state.components"),
		Some(&json!(["game", "levels"]))
	);
}

#[test]
fn radio_selects_as_the_cursor_moves_and_arrows_leave_at_the_ends() {
	let mut screen = Screen::new(&layout("welcome"), Look::Plain);
	let mut state = welcome_state();
	screen.focus_index(2);
	screen.handle_key(key(KeyCode::Down), &mut state);
	assert_eq!(state.get("state.scope"), Some(&json!("machine")));
	// Down at the last item moves focus on to the next control.
	screen.handle_key(key(KeyCode::Down), &mut state);
	assert_eq!(screen.describe_focus(&state), "Back, button");
}

#[test]
fn renderer_overrides_replace_the_base_node() {
	let mut l = layout("welcome");
	let mut tui_text = l.root.children[0].children[0].clone();
	tui_text
		.props
		.insert("text".into(), json!("Terminal only."));
	l.root.children[0].children[0]
		.overrides
		.insert("tui".into(), tui_text);
	let screen = Screen::new(&l, Look::Plain);
	assert!(dump(&screen, &welcome_state(), 80, 24).contains("Terminal only."));
}

#[test]
fn custom_nodes_are_skipped() {
	let l: Layout = serde_json::from_value(json!({
		"version": 1, "id": "x",
		"root": { "kind": "window", "props": { "title": "T" }, "children": [
			{ "kind": "custom", "children": [ { "kind": "button", "props": { "label": "Hidden" } } ] },
			{ "kind": "button", "props": { "label": "Shown" } }
		] }
	}))
	.unwrap();
	let screen = Screen::new(&l, Look::Plain);
	let out = dump(&screen, &MapBindings::new(), 40, 10);
	assert!(out.contains("Shown") && !out.contains("Hidden"));
	assert_eq!(screen.controls().len(), 1);
}

fn inline(json: serde_json::Value) -> Layout {
	serde_json::from_value(json).unwrap()
}

#[test]
fn placeholders_are_filled_from_bound_values_and_unknown_ones_stay() {
	let l = inline(json!({
		"version": 1, "id": "p",
		"root": { "kind": "backdrop", "props": {}, "children": [
			{ "kind": "dialog", "props": { "title": "Setup {product.name}", "width": 50 }, "children": [
				{ "kind": "stack", "children": [
					{ "kind": "text", "props": { "text": "v{product.version} {target.id} {missing.name}" } },
					{ "kind": "button", "props": { "label": "Install {product.name}" } }
				] }
			] }
		] }
	}));
	let b = MapBindings::new()
		.with("product.name", json!("Fizz"))
		.with("product.version", json!("1.2.0"))
		.with("target.id", json!("dos-x86"));
	let out = dump_with_controls(&Screen::new(&l, Look::Plain), &b, 70, 14);
	assert!(out.contains("Setup Fizz"), "{out}");
	assert!(out.contains("v1.2.0 dos-x86 {missing.name}"), "{out}");
	assert!(out.contains("1. Install Fizz (Button)"), "{out}");
	assert!(!out.contains("{product."), "{out}");
}

#[test]
fn the_dump_accepts_source_row_options_and_leaves_out_side_art() {
	let l = inline(json!({
		"version": 1, "id": "p",
		"root": { "kind": "window", "props": { "title": "T" }, "children": [
			{ "kind": "stack", "props": { "region": "side" }, "children": [
				{ "kind": "text", "props": { "text": "SIDE ART" } } ] },
			{ "kind": "radio", "bind": "scope", "props": { "label": "Install for",
				"options": [ { "id": "u", "label": "Only me" }, { "id": "m", "label": "Everyone" } ] } },
			{ "kind": "meter", "bind": "m.total", "props": { "label": "Total", "source": "free_space" } },
			{ "kind": "stack", "props": { "direction": "row", "gap": 2 }, "children": [
				{ "kind": "button", "props": { "label": "Back" } },
				{ "kind": "button", "props": { "label": "Next" } } ] }
		] }
	}));
	let b = MapBindings::new().with("m.total", json!(0.04));
	let out = dump(&Screen::new(&l, Look::Plain), &b, 60, 14);
	assert!(!out.contains("SIDE ART"), "{out}");
	assert!(out.contains("Only me") && out.contains("Everyone"), "{out}");
	assert!(out.contains("LOW"), "{out}");
	let row = out.lines().find(|l| l.contains("Back")).unwrap();
	assert!(row.contains("Next"), "{out}");
}
