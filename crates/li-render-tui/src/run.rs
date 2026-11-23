// The interactive terminal loop for one page.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event};

use crate::bindings::Bindings;
use crate::screen::{Outcome, Screen};

/// How a page ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageResult {
	/// A button was chosen; the value is its action.
	Action(String),
	/// Esc was pressed.
	Cancelled,
}

/// What the engine's tick callback asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickResult {
	Continue,
	/// End the page as if the button with this action had been chosen (for example when the
	/// commit step finishes).
	Finish(String),
}

/// The engine's per-tick callback.
pub type Tick<'a> = &'a mut dyn FnMut(&mut dyn Bindings) -> TickResult;

/// Runs `screen` in the terminal until a button is chosen or Esc is pressed.
///
/// `tick` is called about ten times a second so the engine can update meter values in
/// `bindings` while it works. The terminal is restored on every exit path.
pub fn run_page(
	screen: &mut Screen,
	bindings: &mut dyn Bindings,
	mut tick: Option<Tick<'_>>,
) -> io::Result<PageResult> {
	let mut terminal = ratatui::init();
	let result = (|| {
		loop {
			terminal.draw(|f| screen.render(f.area(), f.buffer_mut(), &*bindings))?;
			if let Some(t) = tick.as_deref_mut()
				&& let TickResult::Finish(action) = t(bindings)
			{
				return Ok(PageResult::Action(action));
			}
			if event::poll(Duration::from_millis(100))?
				&& let Event::Key(key) = event::read()?
			{
				match screen.handle_key(key, bindings) {
					Outcome::Activate(a) => return Ok(PageResult::Action(a)),
					Outcome::Cancel => return Ok(PageResult::Cancelled),
					Outcome::None | Outcome::Changed(_) => {}
				}
			}
		}
	})();
	ratatui::restore();
	result
}
