// The seam between the flow VM and a native window: what a page shows and what the person did.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A graphical runtime (the Win32 one) has no use for 80 by 25 cells. It reads the same compiled
//! screens as the text runtime but only for what they hold: the widgets, in tab order, with their
//! text, items and bound fields. [`Gui::page`] puts those on a window as real controls and
//! returns when the person changes something or presses a button; the flow VM then refreshes the
//! values behind the `{placeholders}` and calls again with `fresh` false, so a window keeps its
//! controls between calls.
//!
//! The flow, the conditions, the install and the receipt are the VM's own, so a native window
//! walks exactly the pages the DOS runtime does.

use alloc::format;
use alloc::string::String;

use crate::expr;
use crate::install::Progress;
use crate::model::{Kind, Screen, Source, Widget};
use crate::ui::Values;

/// The caption of a meter with no label of its own.
pub fn meter_caption(label: &str, source: Source) -> &str {
	if !label.is_empty() {
		return label;
	}
	match source {
		Source::File => "File",
		Source::FreeSpace => "Free space",
		Source::Total => "Progress",
	}
}

/// The meters a commit page gets when its layout names none (a theme's stage draws its meters as
/// part of the backdrop): the file being copied and the whole job.
pub const DEFAULT_METERS: [(&str, Source); 2] = [
	("Current file", Source::File),
	("Overall progress", Source::Total),
];

/// One page for a native window to show.
pub struct GuiPage<'a> {
	/// The compiled screen: its widgets are the controls, in tab order.
	pub screen: &'a Screen,
	/// The strings the screens' bytecode refers to.
	pub strings: &'a [String],
	/// The page's state. A control that changes a bound field writes it here.
	pub values: &'a mut Values,
	/// The step's id, such as `licence`.
	pub step: &'a str,
	pub kind: Kind,
	/// The product's name and the dialog's title with its placeholders filled in.
	pub product: &'a str,
	pub title: String,
	/// This page's place among the pages a person sees, for a caption: from 1, and the last
	/// page there can be.
	pub number: usize,
	pub total: usize,
	/// True the first time a page is shown (create the controls); false when the values have
	/// been refreshed after a change (update the text and enabled states in place).
	pub fresh: bool,
}

impl GuiPage<'_> {
	/// The text a template stands for, with its `{placeholders}` filled in.
	pub fn text(&self, template: &str) -> String {
		self.values.interpolate(template)
	}

	/// The meters to add after the widgets: [`DEFAULT_METERS`] on a commit page that has none.
	pub fn extra_meters(&self) -> &'static [(&'static str, Source)] {
		let has = self
			.screen
			.widgets
			.iter()
			.any(|w| matches!(w, Widget::Meter { .. }));
		if self.kind == Kind::Commit && !has {
			&DEFAULT_METERS
		} else {
			&[]
		}
	}

	/// The page as plain text: what a person sees in the controls, one line each. A native window
	/// writes the same text from the controls it made (`/DUMP`), so a host test and a run under
	/// Wine or on Windows compare page for page.
	pub fn describe(&self) -> String {
		let mut out = format!("== {} | {}\n", self.step, self.title);
		for w in &self.screen.widgets {
			match w {
				Widget::Text { template, .. } => {
					out.push_str(&format!("text: {}\n", self.text(template)));
				}
				Widget::Doc { text, .. } => {
					let n = text.chars().count();
					out.push_str(&format!("doc: {n} chars\n"));
				}
				Widget::List {
					check,
					label,
					bind,
					items,
					default,
					..
				} => {
					out.push_str(&format!(
						"{}: {}\n",
						if *check { "check" } else { "radio" },
						self.text(label)
					));
					for i in items {
						let on = if *check {
							self.values.list(bind).is_some_and(|l| l.contains(&i.id))
						} else {
							match self.values.str(bind) {
								Some(s) => s == i.id,
								None => *default == i.id,
							}
						};
						let mark = match (*check, on) {
							(true, true) => "[x]",
							(true, false) => "[ ]",
							(false, true) => "(o)",
							(false, false) => "( )",
						};
						out.push_str(&format!("  {mark} {}\n", self.text(&i.label)));
					}
				}
				Widget::Field { label, bind, .. } => {
					out.push_str(&format!(
						"field: {} = {}\n",
						self.text(label),
						self.values.str(bind).unwrap_or("")
					));
				}
				Widget::Meter { label, source, .. } => {
					out.push_str(&format!("meter: {}\n", meter_caption(label, *source)));
				}
				Widget::Billboard { slides, .. } => {
					let first = slides.first().map_or(String::new(), |s| self.text(s));
					out.push_str(&format!("billboard: {first}\n"));
				}
				Widget::Button { label, .. } => {
					let off = if self.enabled(w) { "" } else { " [disabled]" };
					out.push_str(&format!("button: {}{off}\n", self.text(label)));
				}
			}
		}
		for (label, _) in self.extra_meters() {
			out.push_str(&format!("meter: {label}\n"));
		}
		out
	}

	/// Whether a widget accepts input now: a button's `enabled_when` holds.
	pub fn enabled(&self, w: &Widget) -> bool {
		match w {
			Widget::Button { enabled_when, .. } => {
				expr::holds(enabled_when, self.strings, &*self.values)
			}
			_ => true,
		}
	}
}

/// What the person did on a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiEvent {
	/// A bound value changed (it is already in the page's values); the VM refreshes the values
	/// and shows the page again.
	Changed,
	/// A button was chosen; the value is its action (`next`, `back`, `cancel`, `finish`...).
	Action(String),
}

/// A native window the flow VM can show its pages on.
pub trait Gui {
	/// Shows the page and waits for the person to change something or choose a button.
	fn page(&mut self, page: &mut GuiPage<'_>) -> GuiEvent;
	/// The page of a commit step while the work runs: update its meters and file name from
	/// `progress`, answer messages, and return false when the person cancelled.
	fn working(&mut self, page: &mut GuiPage<'_>, progress: &Progress) -> bool;
	/// A message with an OK button.
	fn message(&mut self, text: &str);
}
