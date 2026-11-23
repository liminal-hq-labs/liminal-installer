// A running flow behind a handle: events out, answers in, and a headless driver for tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use li_engine::State;
use li_runner::{Cli, RunnerError, RunnerResult};
use serde_json::{Map, Value};

use crate::ui::{Shared, WebProvider, next_is_disabled};
use crate::wire::{PageAnswer, PageKind, PageUpdate, PageView, RunEvent};

/// One run of a lifecycle command, on its own thread. A front end reads [`RunEvent`]s with
/// [`recv`](Session::recv) and answers pages with [`answer`](Session::answer); the Tauri shell
/// forwards them to the webview, and a test plays the person.
pub struct Session {
	events: Mutex<Receiver<RunEvent>>,
	answers: Sender<PageAnswer>,
	shared: Arc<Shared>,
	join: Mutex<Option<JoinHandle<RunnerResult<()>>>>,
}

impl Session {
	/// Starts the command `cli` names. Nothing is shown until the first event is read.
	pub fn spawn(cli: Cli) -> Session {
		let (event_tx, event_rx) = channel();
		let (answer_tx, answer_rx) = channel();
		let shared = Arc::new(Shared::default());
		let mut provider = WebProvider {
			events: event_tx.clone(),
			answers: Some(answer_rx),
			shared: shared.clone(),
		};
		let join = std::thread::spawn(move || {
			let result = li_runner::execute(&cli, &mut provider);
			let last = match &result {
				Ok(()) => RunEvent::Done,
				Err(RunnerError::Cancelled) => RunEvent::Cancelled,
				Err(RunnerError::Failed(message)) => RunEvent::Failed {
					message: message.clone(),
				},
			};
			let _ = event_tx.send(last);
			result
		});
		Session {
			events: Mutex::new(event_rx),
			answers: answer_tx,
			shared,
			join: Mutex::new(Some(join)),
		}
	}

	/// The next event, waiting for it; `None` once the run has ended and every event was read.
	pub fn recv(&self) -> Option<RunEvent> {
		self.events.lock().ok()?.recv().ok()
	}

	/// The next event, waiting at most `wait`.
	pub fn recv_timeout(&self, wait: Duration) -> Option<RunEvent> {
		self.events.lock().ok()?.recv_timeout(wait).ok()
	}

	/// Answers the page on show.
	pub fn answer(&self, answer: PageAnswer) {
		let _ = self.answers.send(answer);
	}

	/// Cancels: a page that is waiting gets a Cancel, and a commit stops at its next operation and
	/// rolls back.
	pub fn cancel(&self) {
		self.shared.cancel.store(true, Ordering::Relaxed);
		self.answer(PageAnswer {
			action: "cancel".into(),
			state: Map::new(),
		});
	}

	/// What changed on the page when its values became `state`.
	pub fn update(&self, state: &Map<String, Value>) -> PageUpdate {
		self.shared.update(state)
	}

	/// Waits for the run to end and returns how it ended.
	pub fn finish(&self) -> RunnerResult<()> {
		let handle = self.join.lock().ok().and_then(|mut j| j.take());
		match handle {
			Some(h) => h
				.join()
				.unwrap_or_else(|_| Err(RunnerError::new("the run stopped unexpectedly"))),
			None => Ok(()),
		}
	}
}

/// What a headless run saw.
#[derive(Debug)]
pub struct HeadlessReport {
	/// Every event, in order.
	pub events: Vec<RunEvent>,
	/// The step ids of the pages shown, in order, commit page included.
	pub pages: Vec<String>,
	pub result: RunnerResult<()>,
}

impl HeadlessReport {
	/// The pages that waited for an answer.
	pub fn answered(&self) -> Vec<&PageView> {
		self.events
			.iter()
			.filter_map(|e| match e {
				RunEvent::Page { page } if page.kind != PageKind::Commit => Some(page),
				_ => None,
			})
			.collect()
	}

	/// The last progress reading's overall fraction, if the commit reported any.
	pub fn last_total(&self) -> Option<f64> {
		self.events.iter().rev().find_map(|e| match e {
			RunEvent::Progress { progress } => Some(progress.total),
			_ => None,
		})
	}
}

/// Plays the person with no window: every page is answered from `answers` and Next is pressed,
/// unless the page has switched Next off (`enabled_when`), which fails the run as a person would
/// be stuck. The same engine, flow and page events the window gets.
pub fn run_headless(cli: Cli, answers: State) -> HeadlessReport {
	let session = Session::spawn(cli);
	let mut events = Vec::new();
	let mut pages = Vec::new();
	let mut stuck: Option<String> = None;
	while let Some(event) = session.recv() {
		if let RunEvent::Page { page } = &event {
			pages.push(page.step.clone());
			if page.kind != PageKind::Commit {
				let mut state = page.state.clone();
				for (k, v) in &answers {
					state.insert(k.clone(), v.clone());
				}
				let update = session.update(&state);
				let blocked = page
					.layout
					.as_ref()
					.is_some_and(|t| next_is_disabled(t, &update.disabled));
				if blocked && stuck.is_none() {
					stuck = Some(format!(
						"page `{}` has Next switched off; answer what it asks, such as --answer licence.accepted=yes",
						page.step
					));
					session.cancel();
				} else {
					session.answer(PageAnswer {
						action: "next".into(),
						state: answers.clone(),
					});
				}
			}
		}
		events.push(event);
	}
	let result = match (stuck, session.finish()) {
		(Some(why), _) => Err(RunnerError::new(why)),
		(None, r) => r,
	};
	HeadlessReport {
		events,
		pages,
		result,
	}
}
