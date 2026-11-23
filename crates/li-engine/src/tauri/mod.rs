// Tauri app support: importing a `tauri.conf.json`, drift against it, WebView2, and the two
// Windows install modes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The design is in `docs/tauri-apps.md`; the Windows facts are in `docs/tauri-windows-spike.md`.
//!
//! - [`conf`] reads a Tauri 2 config (and its platform overlays) into a [`TauriSnapshot`].
//! - [`import`] writes the snapshot, fills `installer.toml` and generates the `origin = "tauri"`
//!   components; [`import::drift`] and [`import::diff`] say what changed since.
//! - [`webview2`] detects the runtime through the Evergreen client keys.
//! - [`plan`] adds `prereq.webview2` and then either `tauri.wrap` or the native registrations to a
//!   Windows plan; [`nsis`] drives the setup and simulates it; [`registry`] is the one list of
//!   values the setup writes.
//! - [`updates`] decides who applies updates, and reconciles the receipt after the app did.

pub mod check;
pub mod conf;
pub mod edit;
pub mod import;
pub mod nsis;
pub mod plan;
pub mod registry;
pub mod settings;
pub mod updates;
pub mod webview2;

pub use li_model::{TauriSection, TauriSnapshot};
