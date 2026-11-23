// The liminst command line as a library: project loading, checks, previews and builds.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The `liminst` command line.
//!
//! The binary is a thin wrapper over this library. The parts the engine and the Studio plug
//! into are traits:
//!
//! - [`ProjectChecker`]: one set of project checks ([`DefaultChecker`] uses `li-model` only).
//! - [`Planner`]: turns a project and a target into a [`li_model::Plan`] for `preview`
//!   ([`StaticPlanner`] lists what the components declare, without running scripts).
//! - [`Builder`]: writes a target's outputs ([`NativeBuilder`] uses `li-package`).
//!
//! Exit codes: 0 means success, 1 means the project has errors, 2 means the command could not
//! run (no project, bad arguments, an unavailable feature).

pub mod build;
pub mod check;
pub mod cli;
pub mod diag;
pub mod feed;
pub mod plan;
pub mod preview;
pub mod project;
pub mod scaffold;
pub mod theme_pack;
pub mod util;

pub use build::{BuildError, BuildRequest, Builder, BuiltArtefact, NativeBuilder};
pub use check::{DefaultChecker, ProjectChecker, check_project};
pub use cli::{Cli, run};
pub use diag::{Diagnostic, Report, Severity};
pub use plan::{Planner, StaticPlanner};
pub use project::{LoadError, LoadedTheme, Project};

/// A command failure that stops `liminst` with exit code 2.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct CliError(pub String);

impl CliError {
	pub fn new(message: impl Into<String>) -> Self {
		CliError(message.into())
	}
}

impl From<LoadError> for CliError {
	fn from(e: LoadError) -> Self {
		CliError(e.to_string())
	}
}

pub type CliResult<T> = Result<T, CliError>;
