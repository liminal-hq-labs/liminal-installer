// Live terminal sessions: the real tui renderer, driven key by key, for the UI and agents.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A session holds one [`Screen`] and its page state on the Rust side, so the webview keeps
//! no terminal state: it sends key presses and draws the [`CellGrid`] that comes back. The
//! page is resolved as a preview resolves it (the target's theme layouts, the `tui`
//! renderer's overrides, list references expanded) and its `{placeholders}` take the preview's
//! sample values.

use std::collections::{BTreeMap, HashMap};

use li_render_tui::{Bindings, CellGrid, Outcome, Screen, render_cells};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{ErrorKind, StudioError};
use crate::screens::seed_meters;
use crate::studio::Studio;

/// The size a session starts at, as a terminal's classic 80 by 24.
pub const DEFAULT_COLS: u16 = 80;
pub const DEFAULT_ROWS: u16 = 24;
const MIN_SIZE: (u16, u16) = (20, 5);
const MAX_SIZE: (u16, u16) = (300, 100);
/// Open sessions kept at once; opening more drops the oldest.
const MAX_SESSIONS: usize = 32;

/// One key press as a browser reports it (`event.key`) or as a terminal names it.
///
/// `key` is a single character (`"y"`, `" "`) or a name: `Tab`, `Enter`, `Escape`, `Backspace`,
/// `Delete`, `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `PageUp` and
/// `PageDown`. `Up`, `Down`, `Left`, `Right`, `Esc`, `Space` and `BackTab` are accepted too.
#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TuiKey {
	pub key: String,
	#[serde(default)]
	pub shift: bool,
	#[serde(default)]
	pub ctrl: bool,
	#[serde(default)]
	pub alt: bool,
}

impl TuiKey {
	pub fn named(key: &str) -> TuiKey {
		TuiKey {
			key: key.to_owned(),
			shift: false,
			ctrl: false,
			alt: false,
		}
	}

	fn to_event(&self) -> Result<KeyEvent, StudioError> {
		let mut mods = KeyModifiers::NONE;
		if self.shift {
			mods |= KeyModifiers::SHIFT;
		}
		if self.ctrl {
			mods |= KeyModifiers::CONTROL;
		}
		if self.alt {
			mods |= KeyModifiers::ALT;
		}
		let code = match self.key.as_str() {
			"Tab" => KeyCode::Tab,
			"BackTab" => KeyCode::BackTab,
			"Enter" => KeyCode::Enter,
			"Escape" | "Esc" => KeyCode::Esc,
			"Backspace" => KeyCode::Backspace,
			"Delete" => KeyCode::Delete,
			"ArrowUp" | "Up" => KeyCode::Up,
			"ArrowDown" | "Down" => KeyCode::Down,
			"ArrowLeft" | "Left" => KeyCode::Left,
			"ArrowRight" | "Right" => KeyCode::Right,
			"Home" => KeyCode::Home,
			"End" => KeyCode::End,
			"PageUp" => KeyCode::PageUp,
			"PageDown" => KeyCode::PageDown,
			"Space" => KeyCode::Char(' '),
			other => {
				let mut chars = other.chars();
				match (chars.next(), chars.next()) {
					(Some(c), None) => KeyCode::Char(c),
					_ => {
						return Err(StudioError::invalid(
							"key",
							format!("`{other}` is not a key"),
						));
					}
				}
			}
		};
		Ok(KeyEvent::new(code, mods))
	}
}

/// What a key press did, for the caller to act on.
#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TuiOutcome {
	/// Focus moved, or nothing happened.
	None,
	/// A bound value changed; `key` is the bind path.
	Changed { key: String },
	/// A button was chosen; `action` is its action (`next`, `back`, `cancel`).
	Activate { action: String },
	/// Esc was pressed.
	Cancel,
}

/// One drawn frame of a session.
#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TuiFrame {
	pub grid: CellGrid,
	/// What the focused control is, as a screen reader would say it: `Next, button`.
	pub focus: String,
	/// The page state now, including what the person changed.
	#[ts(type = "Record<string, unknown>")]
	pub state: Map<String, Value>,
}

#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TuiOpened {
	pub session: String,
	pub frame: TuiFrame,
}

#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TuiKeyResult {
	pub frame: TuiFrame,
	pub outcome: TuiOutcome,
}

/// The values a screen binds to: the preview's sample strings under the page state, which is
/// what changes.
struct SessionBindings {
	vars: BTreeMap<String, Value>,
	state: Map<String, Value>,
}

impl Bindings for SessionBindings {
	fn get(&self, key: &str) -> Option<&Value> {
		self.state.get(key).or_else(|| self.vars.get(key))
	}

	fn set(&mut self, key: &str, value: Value) {
		self.state.insert(key.to_owned(), value);
	}
}

struct Session {
	screen: Screen,
	bindings: SessionBindings,
	cols: u16,
	rows: u16,
	/// Opening order, so the oldest is dropped first.
	seq: u64,
}

impl Session {
	fn frame(&self) -> TuiFrame {
		TuiFrame {
			grid: render_cells(&self.screen, &self.bindings, self.cols, self.rows),
			focus: self.screen.describe_focus(&self.bindings),
			state: self.bindings.state.clone(),
		}
	}
}

/// The open sessions of one studio.
#[derive(Default)]
pub struct Sessions {
	map: HashMap<String, Session>,
	next_seq: u64,
}

fn new_id() -> Result<String, StudioError> {
	let mut bytes = [0u8; 8];
	getrandom::fill(&mut bytes).map_err(|e| StudioError::new(ErrorKind::Internal, e))?;
	Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn check_size(cols: u16, rows: u16) -> Result<(), StudioError> {
	if (MIN_SIZE.0..=MAX_SIZE.0).contains(&cols) && (MIN_SIZE.1..=MAX_SIZE.1).contains(&rows) {
		Ok(())
	} else {
		Err(StudioError::invalid(
			"size",
			format!(
				"{cols} by {rows} cells; use {}x{} to {}x{}",
				MIN_SIZE.0, MIN_SIZE.1, MAX_SIZE.0, MAX_SIZE.1
			),
		))
	}
}

impl Studio {
	/// Opens a terminal session for `target` (`os-arch:renderer`, a `tui` one) on `page`: a
	/// step of the install flow, or a layout id. `state` is the page state to start from, such
	/// as a preview step's; meters are seeded as a preview seeds them. `size` is columns and
	/// rows, 80 by 24 when `None`.
	pub fn tui_session_open(
		&self,
		target: &str,
		page: &str,
		state: Map<String, Value>,
		size: Option<(u16, u16)>,
	) -> Result<TuiOpened, StudioError> {
		let (cols, rows) = size.unwrap_or((DEFAULT_COLS, DEFAULT_ROWS));
		check_size(cols, rows)?;
		let vars = match self.get_preview()? {
			Some(p) if p.target == target => p.vars,
			_ => self.run_preview(target)?.vars,
		};
		let tui = self.with_project(|p| Ok(self.backend.tui_page(p, target, page, &state)?))?;
		let mut state = state;
		seed_meters(&tui.layout.root, &mut state);
		let session = Session {
			screen: Screen::new(&tui.layout, tui.look),
			bindings: SessionBindings {
				vars: vars
					.into_iter()
					.map(|(k, v)| (k, Value::String(v)))
					.collect(),
				state,
			},
			cols,
			rows,
			seq: 0,
		};
		let id = new_id()?;
		let frame = session.frame();
		let mut sessions = self.sessions.lock().expect("sessions");
		let seq = sessions.next_seq;
		sessions.next_seq += 1;
		if sessions.map.len() >= MAX_SESSIONS
			&& let Some(oldest) = sessions
				.map
				.iter()
				.min_by_key(|(_, s)| s.seq)
				.map(|(k, _)| k.clone())
		{
			sessions.map.remove(&oldest);
		}
		sessions.map.insert(id.clone(), Session { seq, ..session });
		Ok(TuiOpened { session: id, frame })
	}

	fn with_session<T>(
		&self,
		id: &str,
		f: impl FnOnce(&mut Session) -> Result<T, StudioError>,
	) -> Result<T, StudioError> {
		let mut sessions = self.sessions.lock().expect("sessions");
		let session = sessions
			.map
			.get_mut(id)
			.ok_or_else(|| StudioError::NotFound(format!("terminal session `{id}`")))?;
		f(session)
	}

	/// Sends one key to the session and returns the new frame and what the key did.
	pub fn tui_session_key(&self, id: &str, key: &TuiKey) -> Result<TuiKeyResult, StudioError> {
		let event = key.to_event()?;
		self.with_session(id, |s| {
			let outcome = match s.screen.handle_key(event, &mut s.bindings) {
				Outcome::None => TuiOutcome::None,
				Outcome::Changed(key) => TuiOutcome::Changed { key },
				Outcome::Activate(action) => TuiOutcome::Activate { action },
				Outcome::Cancel => TuiOutcome::Cancel,
			};
			Ok(TuiKeyResult {
				frame: s.frame(),
				outcome,
			})
		})
	}

	/// Redraws the session at `cols` by `rows` cells.
	pub fn tui_session_resize(
		&self,
		id: &str,
		cols: u16,
		rows: u16,
	) -> Result<TuiFrame, StudioError> {
		check_size(cols, rows)?;
		self.with_session(id, |s| {
			s.cols = cols;
			s.rows = rows;
			Ok(s.frame())
		})
	}

	/// The session's current frame, without a key.
	pub fn tui_session_frame(&self, id: &str) -> Result<TuiFrame, StudioError> {
		self.with_session(id, |s| Ok(s.frame()))
	}

	/// Closes a session and frees it. Closing one that is already gone is not an error.
	pub fn tui_session_close(&self, id: &str) {
		self.sessions.lock().expect("sessions").map.remove(id);
	}

	/// How many sessions are open.
	pub fn tui_session_count(&self) -> usize {
		self.sessions.lock().expect("sessions").map.len()
	}
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use serde_json::json;

	use super::*;
	use crate::EngineBackend;

	fn studio(example: &str) -> Studio {
		let s = Studio::new(Box::new(EngineBackend));
		s.open(
			&Path::new(env!("CARGO_MANIFEST_DIR"))
				.join("../../examples")
				.join(example),
		)
		.unwrap();
		s
	}

	fn key(s: &Studio, id: &str, k: &str) -> TuiKeyResult {
		s.tui_session_key(id, &TuiKey::named(k)).unwrap()
	}

	const TARGET: &str = "dos-x86:tui-retro";

	#[test]
	fn a_session_opens_on_a_step_and_draws_the_real_screen() {
		let s = studio("lt-fizz");
		let o = s
			.tui_session_open(TARGET, "components", Map::new(), None)
			.unwrap();
		let text = o.frame.grid.to_text();
		assert!(text.contains("Choose what to install"), "{text}");
		assert!(text.contains("Game program"));
		assert!(!text.contains('{'), "placeholders are filled:\n{text}");
		assert_eq!(o.frame.grid.width, 80);
		assert_eq!(o.frame.grid.height, 24);
		assert_eq!(o.frame.focus, "Next, button");
	}

	#[test]
	fn keys_move_focus_and_space_toggles_a_checklist() {
		let s = studio("lt-fizz");
		let o = s
			.tui_session_open(TARGET, "components", Map::new(), None)
			.unwrap();
		let id = o.session;
		let r = key(&s, &id, "Tab");
		assert_ne!(r.frame.focus, o.frame.focus, "Tab moved focus");
		assert_eq!(r.outcome, TuiOutcome::None);
		// Go to the checklist and onto its second item, then toggle it.
		let mut r = key(&s, &id, "Tab");
		for _ in 0..4 {
			if r.frame.focus.contains("checklist") {
				break;
			}
			r = key(&s, &id, "Tab");
		}
		assert!(r.frame.focus.contains("checklist"), "{}", r.frame.focus);
		key(&s, &id, "ArrowDown");
		let r = key(&s, &id, "Space");
		let toggled = r.frame.state.get("components.selected").cloned();
		assert!(toggled.is_some());
		let text = r.frame.grid.to_text();
		assert!(text.contains("[x] Sound drivers") || text.contains("[ ] Sound drivers"));
		assert_eq!(
			r.outcome,
			TuiOutcome::Changed {
				key: "components.selected".into()
			}
		);
		let on = r.frame.grid.to_text().contains("[x] Sound drivers");
		let r2 = key(&s, &id, "Space");
		assert_ne!(r2.frame.state.get("components.selected").cloned(), toggled);
		assert_eq!(r2.frame.grid.to_text().contains("[x] Sound drivers"), !on);
	}

	#[test]
	fn enter_on_next_reports_activate_and_esc_cancels() {
		let s = studio("lt-fizz");
		let id = s
			.tui_session_open(TARGET, "components", Map::new(), None)
			.unwrap()
			.session;
		let r = key(&s, &id, "Enter");
		assert_eq!(
			r.outcome,
			TuiOutcome::Activate {
				action: "next".into()
			}
		);
		let r = key(&s, &id, "Escape");
		assert_eq!(r.outcome, TuiOutcome::Cancel);
		// Shift-Tab goes back; Left and Right walk the buttons.
		let r = s
			.tui_session_key(
				&id,
				&TuiKey {
					key: "Tab".into(),
					shift: true,
					ctrl: false,
					alt: false,
				},
			)
			.unwrap();
		assert_eq!(r.outcome, TuiOutcome::None);
	}

	#[test]
	fn typed_letters_edit_a_text_field() {
		let s = studio("lt-fizz");
		let id = s
			.tui_session_open(TARGET, "folder", Map::new(), None)
			.unwrap()
			.session;
		let mut r = key(&s, &id, "Tab");
		for _ in 0..3 {
			if r.frame.focus.contains("path field") {
				break;
			}
			r = key(&s, &id, "Tab");
		}
		let r = key(&s, &id, "x");
		assert!(
			matches!(r.outcome, TuiOutcome::Changed { .. }),
			"{:?}",
			r.frame.focus
		);
		assert!(r.frame.grid.to_text().contains('x'));
	}

	#[test]
	fn the_given_state_is_what_the_screen_starts_from() {
		let s = studio("lt-fizz");
		let mut state = Map::new();
		state.insert("components.selected".into(), json!(["game"]));
		let o = s
			.tui_session_open(TARGET, "components", state, None)
			.unwrap();
		let text = o.frame.grid.to_text();
		assert!(text.contains("[x] Game program"), "{text}");
		assert!(text.contains("[ ] Sound"), "{text}");
	}

	#[test]
	fn resize_redraws_and_close_frees_the_session() {
		let s = studio("lt-fizz");
		let id = s
			.tui_session_open(TARGET, "welcome", Map::new(), Some((100, 30)))
			.unwrap()
			.session;
		assert_eq!(s.tui_session_count(), 1);
		let f = s.tui_session_resize(&id, 60, 20).unwrap();
		assert_eq!((f.grid.width, f.grid.height), (60, 20));
		assert_eq!(f.grid.cells.len(), 60 * 20);
		assert!(s.tui_session_resize(&id, 5, 5).is_err());
		s.tui_session_close(&id);
		assert_eq!(s.tui_session_count(), 0);
		let e = s.tui_session_key(&id, &TuiKey::named("Tab")).unwrap_err();
		assert!(matches!(e, StudioError::NotFound(_)), "{e}");
		s.tui_session_close(&id);
	}

	#[test]
	fn only_terminal_targets_have_sessions_and_keys_are_checked() {
		let s = studio("lt-fizz");
		let e = s
			.tui_session_open("windows-x86_64:win32-classic", "welcome", Map::new(), None)
			.unwrap_err();
		assert!(e.to_string().contains("not a terminal target"), "{e}");
		let e = s
			.tui_session_open(TARGET, "no-such-page", Map::new(), None)
			.unwrap_err();
		assert!(e.to_string().contains("no layout"), "{e}");
		let id = s
			.tui_session_open(TARGET, "welcome", Map::new(), None)
			.unwrap()
			.session;
		assert!(s.tui_session_key(&id, &TuiKey::named("Hyper")).is_err());
		assert_eq!(s.tui_session_count(), 1);
	}

	#[test]
	fn the_oldest_session_is_dropped_past_the_limit() {
		let s = studio("lt-fizz");
		let first = s
			.tui_session_open(TARGET, "welcome", Map::new(), None)
			.unwrap()
			.session;
		for _ in 0..MAX_SESSIONS {
			s.tui_session_open(TARGET, "welcome", Map::new(), None)
				.unwrap();
		}
		assert_eq!(s.tui_session_count(), MAX_SESSIONS);
		assert!(s.tui_session_frame(&first).is_err());
	}
}
