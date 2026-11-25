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

mod dde;
mod dosio;
mod heap;
mod plat;
mod profile;
mod ui;

use alloc::format;
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
	let mut kept = None;
	let mut relaunched = false;
	let mut passed = String::new();
	for w in tail.split_whitespace() {
		if let Some(p) = w.strip_prefix("/LOG=").or_else(|| w.strip_prefix("/log=")) {
			log = Some(String::from(p));
			passed.push_str(&format!(" {w}"));
		} else if let Some(p) = w.strip_prefix("/PLAY=") {
			play = dosio::read_file(p).map(|b| String::from_utf8_lossy(&b).into_owned());
			passed.push_str(&format!(" {w}"));
		} else if let Some(p) = w.strip_prefix("/PAUSE=") {
			pause = p.parse().unwrap_or(0);
			passed.push_str(&format!(" {w}"));
		} else if let Some(p) = w.strip_prefix("/DUMP=") {
			dump = Some(String::from(p));
			passed.push_str(&format!(" {w}"));
		} else if let Some(p) = w.strip_prefix("/DAT=") {
			kept = Some(String::from(p));
		} else if w.eq_ignore_ascii_case("/X") {
			relaunched = true;
		} else {
			// A silent uninstall stays silent in the copy that does the work.
			if w.eq_ignore_ascii_case("/S") {
				passed.push_str(" /S");
			}
			if !args.is_empty() {
				args.push(' ');
			}
			args.push_str(w);
		}
	}
	// An uninstall started from the install folder runs from a copy in the temporary folder: Windows will not
	// delete a program that is running, and `UNINST.EXE` is one of the files to remove.
	if !relaunched
		&& let Ok(li_retro::app::Mode::Uninstall(dir)) = li_retro::app::Mode::from_args(&args)
		&& dosio::exists(&format!("{}UNINST.DAT", dosio::exe_dir()))
	{
		let here = dosio::exe_dir();
		let dir = dir.unwrap_or_else(|| String::from(here.trim_end_matches('\\')));
		let copy = format!("{}\\LIUNINST.EXE", dosio::temp_dir());
		if dosio::copy_file(&dosio::exe_path(), &copy)
			&& dosio::win_exec(&format!("{copy} /U {dir} /X /DAT={here}UNINST.DAT{passed}"))
		{
			return 0;
		}
	}
	let mut p = plat::Win16Platform::new(log);
	p.set_kept(kept);
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
