// SETUP.EXE for Windows 3.1: the portable runtime in `li-retro` on Windows controls.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The program is the flow VM and installer engine of `li-retro` (shared with the DOS runtime) on
//! the Windows 3.1 platform in `plat`, built as Rust for a 16-bit code segment and packed into an NE
//! executable by `tools/ne_pack.py`. It reads `SETUP.LPK` from the folder it runs from.
//!
//! `SETUP` installs, `SETUP /U [folder]` removes an install and `SETUP /V` shows the version.
//! `/LOG=path` writes the pages shown and the exit status to a text file, for the tests. The exit
//! status is the DOS runtime's: 0 done, 1 failed (rolled back), 2 bad command line or pack,
//! 3 cancelled.
#![no_std]
#![no_main]

extern crate alloc;

mod dosio;
mod heap;
mod plat;
mod ui;

use alloc::string::String;
use w16::*;

#[unsafe(no_mangle)]
pub extern "C" fn w16_main(_hinst: u32, _hprev: u32, _show: u32) -> u32 {
	unsafe { SetErrorMode(1) };
	let tail = command_line();
	let mut args = String::new();
	let mut log = None;
	let mut play = None;
	let mut dump = None;
	let mut pause = 0u32;
	for w in tail.split_whitespace() {
		if let Some(p) = w.strip_prefix("/LOG=").or_else(|| w.strip_prefix("/log=")) {
			log = Some(String::from(p));
		} else if let Some(p) = w.strip_prefix("/PLAY=") {
			play = dosio::read_file(p).map(|b| String::from_utf8_lossy(&b).into_owned());
		} else if let Some(p) = w.strip_prefix("/PAUSE=") {
			pause = p.parse().unwrap_or(0);
		} else if let Some(p) = w.strip_prefix("/DUMP=") {
			dump = Some(String::from(p));
		} else {
			if !args.is_empty() {
				args.push(' ');
			}
			args.push_str(w);
		}
	}
	let mut p = plat::Win16Platform::new(log);
	p.testing(play.as_deref(), dump, pause);
	let code = li_retro::app::run(&mut p, &args);
	p.finish(code);
	code as u32
}

/// The command line Windows passed, as text.
fn command_line() -> String {
	let far = unsafe { CMDLINE };
	let mut s = String::new();
	if far == 0 {
		return s;
	}
	for i in 0..120 {
		let b = far_byte(far, i);
		if b == 0 || b == b'\r' {
			break;
		}
		s.push(b as char);
	}
	s
}
