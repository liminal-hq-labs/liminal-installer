// SETUP.EXE for Windows 95 and later: the Liminal Installer's native runtime.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `no_std` and `no_main`, so Rust's standard library (which needs Windows 7) and the C runtime never
//! come into it, and only the ANSI (`...A`) entry points of kernel32, user32, gdi32, comctl32,
//! shell32, advapi32 and ole32 are imported, because Windows 9x lacks most of the wide ones. The
//! flow VM, the install engine and the receipt are `li-retro`'s, the same code the DOS runtime runs;
//! this crate is the machine: files, the registry, the Start menu and the wizard window.
//!
//! `SETUP` installs from `SETUP.LPK` beside it, `SETUP /U [folder]` removes, `SETUP /V` shows the
//! versions. The testing flags are `/PLAY script`, `/DUMP file`, `/LPK file`; `/X` and `/DAT file`
//! are what the uninstaller passes to its own copy in the temporary folder.

#![no_std]
#![no_main]
// The Win32 calls take NUL-ended byte strings; a `c""` literal is the wrong pointer type for them. Every call here is
// unsafe and says why, so the unsafe-in-unsafe lint adds nothing.
#![allow(unsafe_op_in_unsafe_fn, clippy::manual_c_str_literals)]

extern crate alloc;

mod gui;
mod platform;
mod rt;
mod shell;
mod sys;
mod text;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ptr::{null, null_mut};

use li_retro::app::{self, Mode};

use crate::gui::Wizard;
use crate::platform::{Win32, exe_dir, exe_path};
use crate::sys::*;
use crate::text::{ansi, utf8};

/// The command line after the program's own name.
fn command_tail() -> String {
	// SAFETY: the command line is a NUL-ended string that lives as long as the process.
	let line = unsafe {
		let p = GetCommandLineA();
		let mut n = 0;
		while *p.add(n) != 0 {
			n += 1;
		}
		utf8(core::slice::from_raw_parts(p, n))
	};
	let line = line.trim_start();
	let rest = match line.strip_prefix('"') {
		Some(r) => r.split_once('"').map_or("", |(_, t)| t),
		None => line.split_once(char::is_whitespace).map_or("", |(_, t)| t),
	};
	rest.trim().to_string()
}

fn quote(w: &str) -> String {
	if w.contains(' ') { format!("\"{w}\"") } else { w.to_string() }
}

struct Flags {
	play: Option<String>,
	dump: Option<String>,
	lpk: Option<String>,
	shot: Option<String>,
	dat: Option<String>,
	relaunched: bool,
	/// The rest, for the runner.
	rest: String,
}

fn parse(args: &str) -> Flags {
	let words = app::words_of(args);
	let mut f = Flags {
		play: None,
		dump: None,
		lpk: None,
		shot: None,
		dat: None,
		relaunched: false,
		rest: String::new(),
	};
	let mut rest: Vec<String> = Vec::new();
	let mut it = words.into_iter();
	while let Some(w) = it.next() {
		match w.to_ascii_uppercase().as_str() {
			"/PLAY" => f.play = it.next(),
			"/DUMP" => f.dump = it.next(),
			"/LPK" => f.lpk = it.next(),
			"/SHOT" => f.shot = it.next(),
			"/DAT" => f.dat = it.next(),
			"/X" => f.relaunched = true,
			_ => rest.push(quote(&w)),
		}
	}
	f.rest = rest.join(" ");
	f
}

fn read_text(path: &str) -> Option<String> {
	let p = ansi(path);
	// SAFETY: plain file calls on a NUL-ended path and a buffer sized by the file.
	unsafe {
		let h = CreateFileA(
			p.as_ptr(),
			GENERIC_READ,
			FILE_SHARE_READ,
			null(),
			OPEN_EXISTING,
			FILE_ATTRIBUTE_NORMAL,
			NULL,
		);
		if h == INVALID_HANDLE_VALUE {
			return None;
		}
		let len = GetFileSize(h, null_mut()) as usize;
		let mut buf = alloc::vec![0u8; len];
		let mut n = 0u32;
		let ok = ReadFile(h, buf.as_mut_ptr(), len as u32, &mut n, null_mut());
		CloseHandle(h);
		(ok != 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
	}
}

/// Starts a copy of the program in the temporary folder to do the uninstall, so that `UNINST.EXE` is
/// not running when it is deleted. Returns true when the copy started.
fn relaunch_from_temp(dir: &str, flags: &Flags) -> bool {
	let mut tmp = [0u8; 300];
	// SAFETY: file and process calls on NUL-ended strings and local buffers.
	unsafe {
		let n = GetTempPathA(tmp.len() as u32, tmp.as_mut_ptr()) as usize;
		let copy = format!("{}LISETUP{:X}.EXE", utf8(&tmp[..n]), GetTickCount());
		let (from, to) = (ansi(&exe_path()), ansi(&copy));
		if CopyFileA(from.as_ptr(), to.as_ptr(), 0) == 0 {
			return false;
		}
		let mut cmd = format!("\"{copy}\" /U \"{dir}\" /X /DAT \"{}UNINST.DAT\"", exe_dir());
		// The testing flags go along, so a scripted run is scripted in the copy too.
		if let Some(p) = &flags.play {
			cmd.push_str(&format!(" /PLAY \"{p}\""));
		}
		if let Some(d) = &flags.dump {
			cmd.push_str(&format!(" /DUMP \"{d}\""));
		}
		let mut cmd = ansi(&cmd);
		let si = core::mem::zeroed::<StartupInfoA>();
		let si = StartupInfoA {
			cb: core::mem::size_of::<StartupInfoA>() as u32,
			..si
		};
		let mut pi = core::mem::zeroed::<ProcessInformation>();
		let ok = CreateProcessA(
			to.as_ptr(),
			cmd.as_mut_ptr(),
			null(),
			null(),
			0,
			0,
			null(),
			// The copy works from the temporary folder, so the install folder is not its current
			// directory (a process's current directory cannot be removed).
			tmp.as_ptr(),
			&si,
			&mut pi,
		) != 0;
		if ok {
			CloseHandle(pi.process);
			CloseHandle(pi.thread);
		}
		ok
	}
}

fn real_main() -> i32 {
	let flags = parse(&command_tail());
	let mut wizard = Wizard::new();
	if let Some(path) = &flags.play {
		wizard.play(&read_text(path).unwrap_or_default());
	}
	wizard.shots = flags.shot.clone();
	if let Some(path) = &flags.dump {
		let p = ansi(path);
		// SAFETY: creates the dump file, which stays open for the run.
		wizard.dump = unsafe {
			CreateFileA(
				p.as_ptr(),
				GENERIC_WRITE,
				FILE_SHARE_READ,
				null(),
				CREATE_ALWAYS,
				FILE_ATTRIBUTE_NORMAL,
				NULL,
			)
		};
		if wizard.dump == INVALID_HANDLE_VALUE {
			wizard.dump = NULL;
		}
	}
	let mode = Mode::from_args(&flags.rest);
	// An uninstall started from the install folder does its work from a copy in the temporary folder.
	if let Ok(Mode::Uninstall(dir)) = &mode
		&& !flags.relaunched
	{
		let here = exe_dir();
		let mut me = Win32::open(flags.lpk.as_deref(), Wizard::new());
		let from_folder = me.uninstall_project_exists();
		if from_folder {
			let dir = dir.clone().unwrap_or_else(|| here.trim_end_matches('\\').to_string());
			// The copy opens the dump file itself.
			if !wizard.dump.is_null() {
				// SAFETY: closes the file this run opened.
				unsafe { CloseHandle(wizard.dump) };
				wizard.dump = NULL;
			}
			if relaunch_from_temp(&dir, &flags) {
				return 0;
			}
		}
	}
	let mut p = Win32::open(flags.lpk.as_deref(), wizard);
	p.kept = flags.dat.clone();
	let code = app::run(&mut p, &flags.rest);
	p.wizard.close();
	if flags.relaunched {
		// SAFETY: asks Windows NT to delete this copy at the next restart; Windows 9x ignores it.
		unsafe {
			let me = ansi(&exe_path());
			MoveFileExA(me.as_ptr(), null(), MOVEFILE_DELAY_UNTIL_REBOOT);
		}
	}
	code
}

#[unsafe(no_mangle)]
pub extern "C" fn start() -> ! {
	let code = real_main();
	// SAFETY: ends the process with the runner's exit code.
	unsafe { ExitProcess(code as u32) }
}
