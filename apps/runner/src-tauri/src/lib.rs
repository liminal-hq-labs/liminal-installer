// The Tauri shell for the web runner: a window over `li-render-web`'s session.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The shell does four things and holds no installer logic:
//!
//! 1. Reads the command line (`li_render_web::cli`): the same commands as `liminal-engine`, and
//!    `install` when none is given. With `--headless` it never opens a window.
//! 2. Starts a [`Session`] when the page asks (`runner_start`), so no event is sent before the page
//!    is listening, and forwards each [`RunEvent`] to the page as `runner://event`.
//! 3. Takes the page's answers, cancel and changes (`runner_answer`, `runner_cancel`,
//!    `runner_update`) and hands them to the session.
//! 4. Sizes and shows the window from the run's first event, and exits with the run's code. Closing
//!    the window mid-run cancels it and waits for the rollback.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::Mutex;

use li_render_web::cli::{RunnerCli, parse_args, run_headless_main};
use li_render_web::{PageAnswer, PageUpdate, RunEvent, Session, ShellError, ShellErrorKind};
use serde_json::{Map, Value};
use tauri::{Emitter, LogicalSize, Manager, State, WindowEvent};

/// Where the page listens for the run's events.
const EVENT: &str = "runner://event";

/// The events a reloaded page needs to catch up: what it would have seen last of each kind.
#[derive(Default)]
struct Snapshot {
	info: Option<RunEvent>,
	page: Option<RunEvent>,
	progress: Option<RunEvent>,
	last: Option<RunEvent>,
}

impl Snapshot {
	fn record(&mut self, event: &RunEvent) {
		match event {
			RunEvent::Info { .. } => self.info = Some(event.clone()),
			RunEvent::Page { .. } => self.page = Some(event.clone()),
			RunEvent::Progress { .. } => self.progress = Some(event.clone()),
			RunEvent::Done | RunEvent::Cancelled | RunEvent::Failed { .. } => {
				self.last = Some(event.clone());
			}
		}
	}

	fn events(&self) -> Vec<RunEvent> {
		[&self.info, &self.page, &self.progress, &self.last]
			.into_iter()
			.flatten()
			.cloned()
			.collect()
	}

	/// The process exit code the run ended with: 0 done, 3 cancelled, 1 failed.
	fn exit_code(&self) -> i32 {
		match self.last {
			Some(RunEvent::Done) => 0,
			Some(RunEvent::Cancelled) => 3,
			_ => 1,
		}
	}
}

struct Shell {
	cli: Mutex<Option<RunnerCli>>,
	session: Mutex<Option<Arc<Session>>>,
	snapshot: Arc<Mutex<Snapshot>>,
}

type Res<T> = Result<T, ShellError>;

fn internal(e: impl ToString) -> ShellError {
	ShellError::new(ShellErrorKind::Internal, e)
}

fn session(shell: &Shell) -> Res<Arc<Session>> {
	shell
		.session
		.lock()
		.map_err(internal)?
		.clone()
		.ok_or_else(|| ShellError::new(ShellErrorKind::NotStarted, "the run has not started"))
}

/// Sizes the window to the run's look, centres it and shows it.
fn present(app: &tauri::AppHandle, width: u32, height: u32) {
	if let Some(window) = app.get_webview_window("main") {
		let _ = window.set_size(LogicalSize::new(f64::from(width), f64::from(height)));
		let _ = window.center();
		let _ = window.show();
		let _ = window.set_focus();
	}
}

#[tauri::command]
fn runner_start(app: tauri::AppHandle, shell: State<Shell>) -> Res<Vec<RunEvent>> {
	let mut running = shell.session.lock().map_err(internal)?;
	if running.is_some() {
		// A reload: tell the page where the run is.
		return Ok(shell.snapshot.lock().map_err(internal)?.events());
	}
	let cli = shell
		.cli
		.lock()
		.map_err(internal)?
		.take()
		.ok_or_else(|| internal("the command line was already used"))?;
	let session = Arc::new(Session::spawn(cli.runner));
	*running = Some(session.clone());
	let snapshot = shell.snapshot.clone();
	std::thread::spawn(move || {
		while let Some(event) = session.recv() {
			if let RunEvent::Info { info } = &event {
				present(&app, info.window.width, info.window.height);
			}
			if let Ok(mut s) = snapshot.lock() {
				s.record(&event);
			}
			let _ = app.emit(EVENT, &event);
		}
	});
	Ok(Vec::new())
}

#[tauri::command]
fn runner_answer(shell: State<Shell>, answer: PageAnswer) -> Res<()> {
	session(&shell)?.answer(answer);
	Ok(())
}

#[tauri::command]
fn runner_cancel(shell: State<Shell>) -> Res<()> {
	session(&shell)?.cancel();
	Ok(())
}

#[tauri::command]
fn runner_update(shell: State<Shell>, state: Map<String, Value>) -> Res<PageUpdate> {
	Ok(session(&shell)?.update(&state))
}

#[tauri::command]
fn runner_exit(app: tauri::AppHandle, shell: State<Shell>) -> Res<()> {
	let code = shell.snapshot.lock().map_err(internal)?.exit_code();
	app.exit(code);
	Ok(())
}

/// Runs the web runner: headless when asked, else in a window. The return value is the exit code of
/// a headless run, or of a window that failed to open.
pub fn run() -> ExitCode {
	let cli = match parse_args(std::env::args_os()) {
		Ok(cli) => cli,
		Err(e) => {
			let code = if e.use_stderr() { 2 } else { 0 };
			let _ = e.print();
			return ExitCode::from(code);
		}
	};
	if cli.runner.headless {
		return run_headless_main(cli);
	}
	let shell = Shell {
		cli: Mutex::new(Some(cli)),
		session: Mutex::new(None),
		snapshot: Arc::new(Mutex::new(Snapshot::default())),
	};
	let built = tauri::Builder::default()
		.plugin(tauri_plugin_dialog::init())
		.manage(shell)
		.invoke_handler(tauri::generate_handler![
			runner_start,
			runner_answer,
			runner_cancel,
			runner_update,
			runner_exit
		])
		.on_window_event(|window, event| {
			// Closing mid-run cancels it, and the window stays until the engine has rolled back.
			if let WindowEvent::CloseRequested { api, .. } = event {
				let app = window.app_handle().clone();
				let shell = app.state::<Shell>();
				let Some(session) = shell.session.lock().ok().and_then(|s| s.clone()) else {
					return;
				};
				api.prevent_close();
				session.cancel();
				let snapshot = shell.snapshot.clone();
				std::thread::spawn(move || {
					let _ = session.finish();
					let code = snapshot.lock().map_or(1, |s| s.exit_code());
					app.exit(code);
				});
			}
		})
		.build(tauri::generate_context!());
	match built {
		Ok(app) => {
			app.run(|_, _| {});
			ExitCode::SUCCESS
		}
		Err(e) => {
			eprintln!("liminal-runner: cannot open a window: {e}");
			ExitCode::from(1)
		}
	}
}
