// The data model shared by every Liminal Installer crate.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Serde types for the files in `SPEC.md`: the project (`installer.toml`, `components.toml`),
//! flows, themes and layouts, plan operations, receipts and feeds.

pub mod feed;
pub mod flow;
pub mod plan;
pub mod project;
pub mod receipt;
pub mod tauri;
pub mod theme;

pub use feed::{Artefact, Feed, FeedProduct, Release};
pub use flow::{Branch, Flow, FlowError, Step, StepType};
pub use plan::{Operation, Plan};
pub use project::{Component, ComponentsFile, InstallerFile, Product, Scope, Target};
pub use receipt::Receipt;
pub use tauri::{
	External, InstallMode, TauriSection, TauriSnapshot, UpdateMode, Updates, Webview2Mode,
	Webview2Setting,
};
pub use theme::{Layout, Node, ThemeFile};
