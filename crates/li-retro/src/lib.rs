// The portable core of the DOS runtime for Liminal Installer: format, flow VM, renderer and engine.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `li-retro` is `no_std` plus `alloc`, so the same code runs in `SETUP.EXE` on DOS and in the
//! host's tests. `liminst build -t dos-x86` compiles a project into [`model::Project`]; the
//! runtime decodes it, walks its flows against a [`Platform`], draws its screens into text cells
//! and installs or removes its files.
//!
//! | Module | Does |
//! |---|---|
//! | [`model`] | The compiled project and its validator |
//! | [`expr`] | The `when` bytecode and its VM |
//! | [`ui`] | The cell buffer, the retro renderer for a flattened screen, keyboard handling |
//! | [`gui`] | The seam a native window (Win32) implements to show the pages as real controls |
//! | [`install`] | Install, uninstall and the `INSTALL.LIR` receipt |
//! | [`app`] | The flow VM that ties them together (`run`) |
//! | [`platform`] | What the runtime needs from the machine |
//! | [`sound`] | The `detect-sound` built-in |
//! | [`pack`] | `SETUP.LPK`: the Liminal Pack that holds the project and the files, read through `li-lpk` |
//! | [`cp437`], [`text`] | The PC character set, wrapping, `{placeholders}` and sizes |
//! | `sim` (`std` only) | A `Platform` over a host directory, for tests |
#![no_std]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use alloc::string::String;

pub mod app;
pub mod cp437;
pub mod expr;
pub mod gui;
pub mod install;
pub mod model;
pub mod pack;
pub mod platform;
#[cfg(feature = "std")]
pub mod sim;
pub mod sound;
pub mod text;
pub mod ui;

pub use platform::{Key, Platform};

/// What can go wrong in the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
	/// `SETUP.LPK` is not a pack this runtime reads.
	Format(String),
	/// The machine refused a file or directory operation.
	Io(String),
	/// The person cancelled; what was copied has been removed.
	Cancelled,
	/// A stored file does not match its checksum.
	Corrupt(String),
	/// There is not enough room on the drive.
	NoSpace { need: u64, have: u64 },
}

impl core::fmt::Display for Error {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Error::Format(m) | Error::Io(m) | Error::Corrupt(m) => f.write_str(m),
			Error::Cancelled => f.write_str("Setup was cancelled"),
			Error::NoSpace { need, have } => {
				write!(
					f,
					"not enough disk space: {} needed, {} free",
					text::human(*need),
					text::human(*have)
				)
			}
		}
	}
}
