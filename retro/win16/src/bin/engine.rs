// The Win16 engine proof: install or uninstall from SETUP.LPK with the shared `li-retro` engine, no pages.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `ENGINE I dir` unpacks `SETUP.LPK` (beside the program) into `dir`, checking every file's CRC-32,
//! running the plan operations and writing `INSTALL.LIR`. `ENGINE U dir` removes what the receipt
//! lists. `/LOG=path` writes what happened. This is the install and uninstall half of the runtime,
//! the part of `SETUP.EXE` that does not draw: it fits one 64 KB code segment where the whole
//! runtime does not (see `docs/retro-win16.md`).
#![no_std]
#![no_main]

extern crate alloc;

#[path = "lisetup/dosio.rs"]
mod dosio;
#[path = "lisetup/heap.rs"]
mod heap;
#[path = "lisetup/plat.rs"]
mod plat;
#[path = "lisetup/ui.rs"]
mod ui;

use alloc::string::String;
use alloc::vec::Vec;
use li_retro::install;
use li_retro::ui::Values;
use w16::*;

#[unsafe(no_mangle)]
pub extern "C" fn w16_main(_hinst: u32, _hprev: u32, _show: u32) -> u32 {
	unsafe { SetErrorMode(1) };
	let tail = command_line();
	let mut log = None;
	let mut words: Vec<&str> = Vec::new();
	for w in tail.split_whitespace() {
		match w.strip_prefix("/LOG=") {
			Some(p) => log = Some(String::from(p)),
			None => words.push(w),
		}
	}
	let mut p = plat::Win16Platform::new(log);
	let code = run(&mut p, &words);
	p.finish(code);
	code as u32
}

fn run(p: &mut plat::Win16Platform, words: &[&str]) -> i32 {
	let (mode, dir) = match words {
		[m, d] => (*m, *d),
		_ => return 2,
	};
	p.note("opening SETUP.LPK");
	let (project, mut pack) = match li_retro::app::load_project(p) {
		Ok(x) => x,
		Err(_) => return 2,
	};
	p.note("project decoded");
	if mode.eq_ignore_ascii_case("U") {
		return match install::uninstall(p, dir, &mut |p, pr| {
			p.note(&pr.file);
			true
		}) {
			Ok(_) => 0,
			Err(_) => 1,
		};
	}
	let all: Vec<String> = project.components.iter().map(|c| c.id.clone()).collect();
	let v = Values::default();
	match install::install(p, &project, &mut pack, dir, &all, &v, &mut |p, pr| {
		p.note(&pr.file);
		true
	}) {
		Ok(()) => 0,
		Err(li_retro::Error::Cancelled) => 3,
		Err(_) => 1,
	}
}

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
