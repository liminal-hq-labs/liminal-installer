// The liminal-engine runner as a library: the lifecycle commands a themed installer runs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The `liminal-engine` runner.
//!
//! A themed `.run` carries this binary and a `.lip`. The `.lip` holds the payload and, under
//! `_project/`, the project (see [`li_engine::bundle`]), so the runner walks the real flows,
//! layouts and scripts. After a successful install the runner keeps a copy of itself, the
//! `.lip` and the pinned feed key in `{install}/.liminal/runner/`, so `update`, `modify`,
//! `repair`, `uninstall` and `launch` need nothing but the receipt's install directory.
//!
//! - Interactive runs draw each page with `li-render-tui` ([`ui::TuiUi`]); with `--headless`,
//!   or when there is no terminal, answers come from `--answer key=value` ([`ui::ConsoleUi`]).
//! - `update` verifies the feed's Ed25519 signature against the key pinned at install, refuses
//!   a feed older than the last one it saw, and checks the `.lip` against the feed's size and
//!   SHA-256 before anything runs.
//!
//! Exit codes: 0 success, 1 failure (every change was rolled back), 2 bad arguments, 3 the
//! person cancelled.

pub mod cli;
pub mod feedsrc;
pub mod stash;
pub mod ui;

pub use cli::{Cli, run};

/// Why the runner stopped.
#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
	/// Something failed; the engine has already rolled back any change it was making.
	#[error("{0}")]
	Failed(String),
	/// The person cancelled; every change was rolled back.
	#[error("cancelled; every change was rolled back")]
	Cancelled,
}

impl RunnerError {
	pub fn new(message: impl Into<String>) -> Self {
		RunnerError::Failed(message.into())
	}
}

impl From<li_engine::EngineError> for RunnerError {
	fn from(e: li_engine::EngineError) -> Self {
		match e {
			li_engine::EngineError::Cancelled => RunnerError::Cancelled,
			other => RunnerError::Failed(other.to_string()),
		}
	}
}

impl From<li_pack::PackError> for RunnerError {
	fn from(e: li_pack::PackError) -> Self {
		RunnerError::Failed(e.to_string())
	}
}

pub type RunnerResult<T> = Result<T, RunnerError>;
