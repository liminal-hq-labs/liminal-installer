// The installer engine: loads projects, checks them, and runs the install lifecycle.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The engine behind `liminst` and the Studio.
//!
//! - [`Project::load`] reads a project; [`check`] validates it.
//! - [`cond`] evaluates the `when` and `branches` expressions.
//! - [`preview`] builds the plan without touching anything.
//! - [`install`], [`update`], [`modify`], [`repair`], [`uninstall`] and [`launch`] run the four
//!   phases, Plan, Stage, Commit and Record, against a [`Machine`]. [`RealMachine`] uses the
//!   local file system; [`SimMachine`] is in memory, for dry runs and tests.
//! - [`FlowRunner`] walks a flow and asks a [`Ui`] for each page; [`HeadlessUi`] answers from
//!   defaults.
//!
//! # Phases
//!
//! 1. **Plan** turns the selected components, and any `on_commit` script hooks, into an ordered
//!    list of operations that hold placeholders (`{install}` and so on) rather than machine
//!    paths. `receipt.write` is last.
//! 2. **Stage** checks every source file against the SHA-256 the plan recorded, resolves every
//!    placeholder and checks free space. Nothing has changed yet.
//! 3. **Commit** writes each change to a journal in `{temp}` before making it. A failure or
//!    cancel replays the journal in reverse, restoring replaced files from backups kept beside
//!    it, so the machine ends up as it began. A journal left by a crash is replayed at the start
//!    of the next run.
//! 4. **Record** writes `{install}/.liminal/receipt.json` last, inside the same transaction.
//!
//! The receipt lists what each operation did in placeholder form. `uninstall` reads it and
//! undoes the operations in reverse; `update`, `modify` and `repair` start from it.

pub mod check;
pub mod cond;
pub mod error;
mod exec;
pub mod fault;
pub mod flow;
pub mod lifecycle;
pub mod machine;
pub mod payload;
pub mod plan;
pub mod progress;
pub mod project;
pub mod real;
pub mod scripts;
pub mod sim;

pub use check::{Diagnostic, Severity, check};
pub use cond::{CondContext, CondError, Expr};
pub use error::{EngineError, Result};
pub use exec::recover;
pub use fault::FaultMachine;
pub use flow::{Action, CommitFn, FlowEnv, FlowOutcome, FlowRunner, HeadlessUi, State, Ui};
pub use lifecycle::{
	InstallRequest, LaunchOutcome, RepairReport, UpdateInfo, find_update, install, launch, modify,
	preview, read_receipt, repair, system_info, uninstall, update,
};
pub use machine::{Dirs, Machine, NodeKind, Resolver};
pub use payload::{DirPayload, LipPayload, MemPayload, Payload, expand_sources, pack_payload};
pub use plan::{build_plan, op_key, receipt_path, select_components};
pub use progress::{Phase, Progress, RunOptions};
pub use project::{Project, ThemeData};
pub use real::RealMachine;
pub use scripts::ProjectScripts;
pub use sim::{SimMachine, SimNode};
