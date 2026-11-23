// The web renderer's engine side: a flow session for a webview, its wire types and a headless driver.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The engine side of the `web` renderer.
//!
//! The web runner (`apps/runner`) shows an installer project in a Tauri webview, drawn by
//! `@liminal-hq/installer-renderer`. This crate is everything that is not the window:
//!
//! - [`WebUi`] is a `li_engine::Ui`. Each page of the real flow becomes a [`RunEvent::Page`]
//!   (the layout tree prepared for the renderer, the page's state, the `{placeholder}` values)
//!   and the flow waits for the person's [`PageAnswer`]. The commit step sends a page that does not
//!   wait, then [`RunEvent::Progress`] readings; cancelling stops the commit at its next operation
//!   and the engine rolls back.
//! - [`Session`] runs one lifecycle command (`install`, `update`, `modify`, `repair`, `uninstall`)
//!   through `li_runner::execute` on its own thread, so the window never blocks the engine and the
//!   engine never blocks the window.
//! - [`run_headless`] plays the person with no window, for tests and CI.
//! - [`tree`] prepares a layout tree for a renderer; the Studio's previews use the same code.
//!
//! The wire types derive `ts_rs::TS` under the `ts` feature; their TypeScript is generated into
//! `packages/protocol` and committed.

pub mod cli;
pub mod look;
pub mod session;
pub mod tree;
pub mod ui;
pub mod wire;

pub use session::{HeadlessReport, Session, run_headless};
pub use ui::{WebProvider, WebUi, next_is_disabled, walk};
pub use wire::{
	PageAnswer, PageKind, PageUpdate, PageView, ProgressView, RunComponent, RunEvent, RunInfo,
	RunMode, RunProduct, ShellError, ShellErrorKind, WindowSize,
};
