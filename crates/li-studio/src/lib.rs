// The Studio's editing core: project files, shared undo history, the engine trait and MCP.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Everything the Studio does that is not drawing. The Tauri shell and the MCP server both
//! call [`Studio`], so a change made by a person or an agent goes through one undo history
//! and raises one event.

pub mod backend;
pub mod checks;
pub mod error;
pub mod history;
pub mod mcp;
pub mod mock;
pub mod project;
pub mod resolve;
pub mod studio;

pub use backend::{
	BackendError, HookInfo, Preview, PreviewStep, Problem, Screenshot, ScriptReport, Severity,
	StudioBackend,
};
pub use error::StudioError;
pub use history::{Entry, EntrySummary, History, Source};
pub use mock::MockBackend;
pub use project::{Doc, DocKey, Project};
pub use studio::{ChangeEvent, Studio};
