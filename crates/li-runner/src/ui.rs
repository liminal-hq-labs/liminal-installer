// The two `li_engine::Ui` implementations: the terminal renderer and the headless console.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use li_engine::{Action, HeadlessUi, Progress, State, Ui};
use li_model::{Layout, Node, Step};
use li_render_tui::{
	Bindings, Look, MapBindings, PageResult, RENDERER_ID, Screen, TickResult, run_page,
};
use serde_json::{Value, json};

/// Answers every page from a table and prints what it copies. For `--headless` and for runs
/// with no terminal.
pub struct ConsoleUi {
	inner: HeadlessUi,
	last_file: Option<String>,
}

impl ConsoleUi {
	pub fn new(answers: State) -> Self {
		ConsoleUi {
			inner: HeadlessUi::new(answers),
			last_file: None,
		}
	}
}

impl Ui for ConsoleUi {
	fn page(&mut self, step: &Step, layout: Option<&Layout>, state: &mut State) -> Action {
		self.inner.page(step, layout, state)
	}

	fn problems(&mut self, step: &Step, problems: &[String]) {
		for p in problems {
			eprintln!("liminal-engine: {}: {p}", step.id);
		}
		self.inner.problems(step, problems);
	}

	fn progress(&mut self, p: &Progress) -> bool {
		if p.current_file != self.last_file {
			if let Some(f) = &p.current_file {
				println!("  {f}");
			}
			self.last_file.clone_from(&p.current_file);
		}
		true
	}
}

/// What the progress page shows, shared between the engine's thread and the page's.
#[derive(Default)]
struct Shared {
	values: Mutex<BTreeMap<String, Value>>,
	done: AtomicBool,
	cancelled: AtomicBool,
}

struct Running {
	shared: Arc<Shared>,
	handle: JoinHandle<()>,
	layout: Layout,
	initial_free: Option<u64>,
}

/// Draws each page with the `tui` renderer. The commit page runs in its own thread, so the
/// engine keeps copying while the page redraws; each `Progress` event updates the meters it
/// binds, and Esc on that page cancels the commit and rolls it back.
pub struct TuiUi {
	look: Look,
	running: Option<Running>,
}

impl TuiUi {
	pub fn new(look: Look) -> Self {
		TuiUi {
			look,
			running: None,
		}
	}

	/// Ends the progress page, if one is showing.
	fn stop_progress(&mut self) {
		if let Some(r) = self.running.take() {
			r.shared.done.store(true, Ordering::Relaxed);
			let _ = r.handle.join();
		}
	}
}

impl Drop for TuiUi {
	fn drop(&mut self) {
		self.stop_progress();
	}
}

/// Seeds the bindings from the flow state: each field under `state.<name>` and its bare name.
fn seed(state: &State) -> MapBindings {
	let mut b = MapBindings::new();
	for (k, v) in state {
		b.set(&format!("state.{k}"), v.clone());
		b.set(k, v.clone());
	}
	b
}

/// Copies what the person changed back into the flow state.
fn sync(before: &MapBindings, after: &MapBindings, state: &mut State) {
	for (k, v) in &after.0 {
		if k.starts_with("meter.") || before.get(k) == Some(v) {
			continue;
		}
		let name = k.strip_prefix("state.").unwrap_or(k);
		state.insert(name.to_owned(), v.clone());
	}
}

fn action_for(result: &PageResult) -> Action {
	match result {
		PageResult::Cancelled => Action::Cancel,
		PageResult::Action(a) => match a.as_str() {
			"back" => Action::Back,
			"cancel" => Action::Cancel,
			_ => Action::Next,
		},
	}
}

impl Ui for TuiUi {
	fn page(&mut self, step: &Step, layout: Option<&Layout>, state: &mut State) -> Action {
		self.stop_progress();
		let Some(layout) = layout else {
			return Action::Next;
		};
		let mut screen = Screen::new(layout, self.look);
		let before = seed(state);
		let mut bindings = before.clone();
		match run_page(&mut screen, &mut bindings, None) {
			Ok(result) => {
				sync(&before, &bindings, state);
				action_for(&result)
			}
			Err(e) => {
				eprintln!("liminal-engine: cannot draw `{}`: {e}", step.id);
				Action::Cancel
			}
		}
	}

	fn problems(&mut self, step: &Step, problems: &[String]) {
		for p in problems {
			eprintln!("{}: {p}", step.id);
		}
	}

	fn commit_started(&mut self, _step: &Step, layout: Option<&Layout>) {
		self.stop_progress();
		let Some(layout) = layout else { return };
		let shared = Arc::new(Shared::default());
		let thread_shared = shared.clone();
		let mut screen = Screen::new(layout, self.look);
		let handle = std::thread::spawn(move || {
			let mut bindings = MapBindings::new();
			let mut tick = |b: &mut dyn Bindings| {
				if let Ok(values) = thread_shared.values.lock() {
					for (k, v) in values.iter() {
						b.set(k, v.clone());
					}
				}
				if thread_shared.done.load(Ordering::Relaxed) {
					TickResult::Finish("done".into())
				} else {
					TickResult::Continue
				}
			};
			if let Ok(PageResult::Cancelled) = run_page(&mut screen, &mut bindings, Some(&mut tick))
			{
				thread_shared.cancelled.store(true, Ordering::Relaxed);
			}
		});
		self.running = Some(Running {
			shared,
			handle,
			layout: layout.clone(),
			initial_free: None,
		});
	}

	fn progress(&mut self, p: &Progress) -> bool {
		let Some(r) = self.running.as_mut() else {
			return true;
		};
		if r.shared.cancelled.load(Ordering::Relaxed) {
			return false;
		}
		let initial = *r.initial_free.get_or_insert(p.free_space);
		let mut values = BTreeMap::new();
		meters(&r.layout.root, p, initial, &mut values);
		if let Ok(mut shared) = r.shared.values.lock() {
			*shared = values;
		}
		true
	}
}

/// The value each `meter` and `progress` node binds to, from one progress event.
fn meters(node: &Node, p: &Progress, initial_free: u64, out: &mut BTreeMap<String, Value>) {
	let node = node.for_renderer(RENDERER_ID);
	if matches!(node.kind.as_str(), "meter" | "progress")
		&& let Some(bind) = &node.bind
	{
		let kind = if node.kind == "progress" {
			"total"
		} else {
			node.props
				.get("meter")
				.and_then(Value::as_str)
				.unwrap_or("total")
		};
		let value = match kind {
			"file" => {
				out.insert(
					format!("{bind}.label"),
					json!(p.current_file.clone().unwrap_or_default()),
				);
				if p.files_total > 0 {
					p.files_done as f64 / p.files_total as f64
				} else {
					p.fraction()
				}
			}
			"free_space" => {
				if initial_free == 0 || initial_free == u64::MAX {
					1.0
				} else {
					p.free_space as f64 / initial_free as f64
				}
			}
			_ => p.fraction(),
		};
		out.insert(bind.clone(), json!(value));
	}
	for c in &node.children {
		meters(c, p, initial_free, out);
	}
}

#[cfg(test)]
mod tests {
	use li_engine::Phase;

	use super::*;

	fn progress() -> Progress {
		Progress {
			phase: Phase::Commit,
			current_file: Some("{install}/bin/demo".into()),
			files_done: 1,
			files_total: 4,
			bytes_done: 50,
			bytes_total: 100,
			free_space: 800,
			low_space: false,
		}
	}

	fn layout() -> Layout {
		serde_json::from_value(json!({
			"version": 1, "id": "copying",
			"root": { "kind": "backdrop", "children": [
				{ "kind": "progress", "bind": "meter.total" },
				{ "kind": "meter", "bind": "meter.file", "props": { "meter": "file" } },
				{ "kind": "meter", "bind": "meter.free", "props": { "meter": "free_space" } }
			] }
		}))
		.unwrap()
	}

	#[test]
	fn meters_follow_the_progress_events() {
		let mut out = BTreeMap::new();
		meters(&layout().root, &progress(), 1000, &mut out);
		assert_eq!(out["meter.total"], json!(0.5));
		assert_eq!(out["meter.file"], json!(0.25));
		assert_eq!(out["meter.file.label"], json!("{install}/bin/demo"));
		assert_eq!(out["meter.free"], json!(0.8));
	}

	#[test]
	fn answers_come_back_into_the_state_only_when_changed() {
		let mut state = State::new();
		state.insert("install_dir".into(), json!("/a"));
		state.insert("keep".into(), json!(1));
		let before = seed(&state);
		let mut after = before.clone();
		after.set("state.install_dir", json!("/b"));
		after.set("meter.total", json!(0.5));
		after.set("state.accepted", json!(true));
		sync(&before, &after, &mut state);
		assert_eq!(state["install_dir"], json!("/b"));
		assert_eq!(state["keep"], json!(1));
		assert_eq!(state["accepted"], json!(true));
		assert!(!state.contains_key("meter.total"));
	}

	#[test]
	fn buttons_map_to_flow_actions() {
		let act = |s: &str| action_for(&PageResult::Action(s.into()));
		assert_eq!(act("next"), Action::Next);
		assert_eq!(act("finish"), Action::Next);
		assert_eq!(act("back"), Action::Back);
		assert_eq!(act("cancel"), Action::Cancel);
		assert_eq!(action_for(&PageResult::Cancelled), Action::Cancel);
	}
}
