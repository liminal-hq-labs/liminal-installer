// SETUP.EXE: the Liminal Installer runtime for DOS.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The program is the portable runtime in `li-retro` on the DOS platform in `platform`. It reads
//! `SETUP.DAT` from the folder it runs from, draws the screens in text mode and installs.
//!
//! `SETUP` installs, `SETUP /U [folder]` removes an install and `SETUP /V` prints the version.
//! The exit status is 0 when done, 1 when it failed (and rolled back), 2 for a bad command line
//! or a missing or damaged `SETUP.DAT`, and 3 when the person cancelled.
//!
//! Set `LI_DUMP=D:\DUMP.TXT` to have every screen shown written to a text file, which is how
//! the headless tests look at the screens.
#![no_std]
#![no_main]

extern crate alloc;

mod keys;
mod platform;
mod video;

use alloc::string::String;

rt::entry!(main);

fn main() -> i32 {
	let args = String::from_utf8_lossy(&rt::env::command_tail()).into_owned();
	let mut p = platform::DosPlatform::new();
	let code = li_retro::app::run(&mut p, &args);
	p.finish();
	match code {
		1 => rt::println!("Setup could not finish. Nothing was left behind."),
		3 => rt::println!("Setup was cancelled."),
		_ => {}
	}
	code
}
