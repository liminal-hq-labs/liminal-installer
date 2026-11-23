// Visual styles: the retro text-mode look and the plain, colourless one.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use ratatui::style::{Color, Modifier, Style};

/// How the screen is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Look {
	/// White on blue, shadowed dialogs, box-drawing characters and block progress bars.
	#[default]
	Retro,
	/// No colour and ASCII only, for terminals and screen readers that handle it better.
	Plain,
}

impl Look {
	pub fn parse(name: &str) -> Option<Look> {
		match name {
			"retro" => Some(Look::Retro),
			"plain" => Some(Look::Plain),
			_ => None,
		}
	}

	pub(crate) fn glyphs(self) -> Glyphs {
		match self {
			Look::Retro => Glyphs {
				dialog: ['═', '║', '╔', '╗', '╚', '╝'],
				frame: ['─', '│', '┌', '┐', '└', '┘'],
				bar_full: '█',
				bar_empty: '░',
				check_on: "[x]",
				check_off: "[ ]",
				radio_on: "(•)",
				radio_off: "( )",
				shadow: true,
			},
			Look::Plain => Glyphs {
				dialog: ['-', '|', '+', '+', '+', '+'],
				frame: ['-', '|', '+', '+', '+', '+'],
				bar_full: '#',
				bar_empty: '-',
				check_on: "[x]",
				check_off: "[ ]",
				radio_on: "(*)",
				radio_off: "( )",
				shadow: false,
			},
		}
	}

	pub(crate) fn palette(self) -> Palette {
		match self {
			Look::Retro => Palette {
				base: Style::new().fg(Color::White).bg(Color::Blue),
				dialog: Style::new().fg(Color::Black).bg(Color::Gray),
				focus: Style::new().fg(Color::Black).bg(Color::Cyan),
				title: Style::new().add_modifier(Modifier::BOLD),
				shadow: Style::new().fg(Color::DarkGray).bg(Color::Black),
				warn: Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
				status: Style::new().fg(Color::Black).bg(Color::Cyan),
			},
			Look::Plain => Palette {
				base: Style::new(),
				dialog: Style::new(),
				focus: Style::new(),
				title: Style::new(),
				shadow: Style::new(),
				warn: Style::new(),
				status: Style::new(),
			},
		}
	}
}

/// Border characters are ordered horizontal, vertical, top-left, top-right, bottom-left and
/// bottom-right.
pub(crate) struct Glyphs {
	pub dialog: [char; 6],
	pub frame: [char; 6],
	pub bar_full: char,
	pub bar_empty: char,
	pub check_on: &'static str,
	pub check_off: &'static str,
	pub radio_on: &'static str,
	pub radio_off: &'static str,
	pub shadow: bool,
}

pub(crate) struct Palette {
	pub base: Style,
	pub dialog: Style,
	pub focus: Style,
	pub title: Style,
	pub shadow: Style,
	pub warn: Style,
	pub status: Style,
}
