// A DDE conversation with Program Manager: the group list, then commands, over raw `WM_DDE_*` messages.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Each program in this folder uses part of this module, so what one does not use is not dead code.
#![allow(dead_code)]
//! Windows 3.1 has no API for Program Manager groups: the way in is a DDE conversation with the
//! `PROGMAN` server (topic `PROGMAN`), which takes commands such as `[AddItem(...)]` in a
//! `WM_DDE_EXECUTE` and answers a `WM_DDE_REQUEST` for the item `PROGMAN` with the group names.
//! DDEML is not used: it needs a callback through `MakeProcInstance`, string handles and another
//! imported module, where the raw messages need a window and its procedure.
//!
//! The sequence, with Win16's `lParam` packing:
//!
//! 1. A hidden window of class `LiminalDde`, with its own procedure (not the wizard's).
//! 2. `SendMessage(-1, WM_DDE_INITIATE, hwnd, MAKELONG(app, topic))` with the atom `PROGMAN` for
//!    both. Each `WM_DDE_ACK` that comes back names a server window in `wParam`; the first is
//!    kept, any other gets a `WM_DDE_TERMINATE`, and the atoms in every ACK are deleted. No ACK
//!    means no Program Manager (another shell, or none), and the caller falls back to a batch file.
//! 3. A command: a global block of `GMEM_MOVEABLE | GMEM_DDESHARE` memory holds the NUL-ended
//!    string, `PostMessage(server, WM_DDE_EXECUTE, hwnd, MAKELONG(0, hCommands))`, then the loop
//!    pumps messages until the server's ACK (bit 15 of the low word is "acknowledged") or five
//!    seconds pass. The block is ours to free when the ACK arrives, whichever its status.
//! 4. The group list: `WM_DDE_REQUEST` with `CF_TEXT` and the atom `PROGMAN` brings a
//!    `WM_DDE_DATA` whose block is a `DDEDATA` (a flags word, `cfFormat`, then the text). It is
//!    freed when `fRelease` is set and acknowledged when `fAckReq` is, else its atom is deleted.
//! 5. `WM_DDE_TERMINATE` to the server, a wait for its own, and the window goes.
//!
//! The procedure never allocates (the heap is not reentrant), and the state is a few statics,
//! because the data segment is nearly full: the group list is searched as it is read, and a short
//! slice of the group's own listing is kept for the log.
use alloc::vec::Vec;
use core::arch::asm;
use core::ptr::addr_of_mut;
use w16::*;

use crate::ui::ansi;

export_proc!(DdeProc, dde_proc);

const WM_DDE_INITIATE: u32 = 0x3E0;
const WM_DDE_TERMINATE: u32 = 0x3E1;
const WM_DDE_ACK: u32 = 0x3E4;
const WM_DDE_DATA: u32 = 0x3E5;
const WM_DDE_REQUEST: u32 = 0x3E6;
const WM_DDE_EXECUTE: u32 = 0x3E8;
const CF_TEXT: u32 = 1;
const HWND_BROADCAST: u32 = 0xFFFF;
const WS_POPUP: u32 = 0x8000_0000;
const GMEM_MOVEABLE_DDESHARE: u32 = 0x2002;
const DDE_ACK: u32 = 0x8000;
const DDE_RELEASE: u32 = 0x2000;
const DDE_ACKREQ: u32 = 0x8000;
const WM_QUIT: u32 = 0x0012;

/// How long one step may take before the conversation is given up.
const TIMEOUT_MS: u32 = 5000;
/// How long a server gets to answer `WM_DDE_TERMINATE` with its own.
const TERMINATE_MS: u32 = 1000;
/// How long a posted `WM_DDE_ACK` to the broadcast may take to arrive.
const INITIATE_MS: u32 = 250;
/// The most of a reply read: a group list is a few hundred bytes.
const MAX_REPLY: u32 = 4096;

const IDLE: u8 = 0;
const INIT: u8 = 1;
const EXEC: u8 = 2;
const REQUEST: u8 = 3;

struct Conv {
	hwnd: u32,
	server: u32,
	phase: u8,
	/// The last reply to a command or request: 0 none, 1 acknowledged, 2 refused.
	ack: u8,
	/// A `WM_DDE_DATA` arrived.
	got: bool,
	/// The group list names the group.
	found: bool,
	/// The server has ended the conversation.
	gone: bool,
	/// The reply to the request on the way is the group's own listing, kept in `INFO`.
	want_info: bool,
	name_len: usize,
	info_len: usize,
	registered: bool,
}

static mut C: Conv = Conv {
	hwnd: 0,
	server: 0,
	phase: IDLE,
	ack: 0,
	got: false,
	found: false,
	gone: false,
	want_info: false,
	name_len: 0,
	info_len: 0,
	registered: false,
};
/// The group's name in Windows ANSI, for comparing it with the list.
static mut NAME: [u8; 96] = [0; 96];
/// The start of the group's listing, with the line ends turned into bars, for the log.
static mut INFO: [u8; 160] = [0; 160];
static mut DMSG: Msg = Msg {
	hwnd: 0,
	message: 0,
	wparam: 0,
	lparam: 0,
	time: 0,
	pt: [0; 2],
};
static mut DCLASS: WndClass = WndClass {
	style: 0,
	wndproc: 0,
	cls_extra: 0,
	wnd_extra: 0,
	instance: 0,
	icon: 0,
	cursor: 0,
	background: 0,
	menu: 0,
	class_name: 0,
};

fn c() -> &'static mut Conv {
	unsafe { &mut *addr_of_mut!(C) }
}

fn makelong(lo: u32, hi: u32) -> u32 {
	(lo & 0xFFFF) | (hi << 16)
}

/// Writes one byte through a far pointer (selector in the high half).
fn far_set(p: u32, i: u32, v: u8) {
	unsafe {
		asm!(
			"mov es, dx",
			"mov es:[bx], al",
			"push ds",
			"pop es",
			in("dx") (p >> 16) as u16,
			in("bx") (p as u16).wrapping_add(i as u16),
			in("al") v,
			options(nostack, preserves_flags),
		);
	}
}

/// The procedure of the hidden window: it only records what Program Manager sends.
extern "C" fn dde_proc(hwnd: u32, msg: u32, wparam: u32, lparam: u32) -> u32 {
	match msg {
		WM_DDE_ACK => on_ack(wparam, lparam),
		WM_DDE_DATA => on_data(wparam, lparam),
		WM_DDE_TERMINATE => {
			if wparam == c().server {
				c().gone = true;
			}
		}
		_ => return unsafe { DefWindowProc(hwnd, msg, wparam, lparam) },
	}
	0
}

fn on_ack(from: u32, lparam: u32) {
	let (lo, hi) = (lparam & 0xFFFF, lparam >> 16);
	let s = c();
	match s.phase {
		INIT => {
			if s.server == 0 {
				s.server = from;
			} else if from != s.server {
				unsafe { PostMessage(from, WM_DDE_TERMINATE, s.hwnd, 0) };
			}
			// Both atoms are the server's own references; ours to delete.
			unsafe {
				GlobalDeleteAtom(lo);
				GlobalDeleteAtom(hi);
			}
		}
		EXEC if from == s.server => {
			s.ack = if lo & DDE_ACK != 0 { 1 } else { 2 };
			if hi != 0 {
				unsafe { GlobalFree(hi) };
			}
		}
		REQUEST if from == s.server => {
			s.ack = 2;
			if hi != 0 {
				unsafe { GlobalDeleteAtom(hi) };
			}
		}
		_ => {}
	}
}

fn on_data(from: u32, lparam: u32) {
	let (h, atom) = (lparam & 0xFFFF, lparam >> 16);
	let s = c();
	if from != s.server || h == 0 {
		return;
	}
	let flags = unsafe {
		let p = GlobalLock(h);
		let flags = u32::from(far_byte(p, 0)) | (u32::from(far_byte(p, 1)) << 8);
		if s.want_info {
			scan_info(p);
		} else {
			scan_list(p);
		}
		GlobalUnlock(h);
		flags
	};
	if flags & DDE_RELEASE != 0 {
		unsafe { GlobalFree(h) };
	}
	if flags & DDE_ACKREQ != 0 {
		unsafe { PostMessage(from, WM_DDE_ACK, s.hwnd, makelong(DDE_ACK, atom)) };
	} else {
		unsafe { GlobalDeleteAtom(atom) };
	}
	s.got = true;
}

/// Reads the text of a `DDEDATA` block (it starts four bytes in) line by line and notes whether
/// one line is the group's name, ignoring case.
fn scan_list(p: u32) {
	let s = c();
	let name = unsafe { &*addr_of_mut!(NAME) };
	let (mut at, mut ok, mut len) = (0usize, true, 0usize);
	let mut i = 0u32;
	loop {
		let b = if i < MAX_REPLY { far_byte(p, 4 + i) } else { 0 };
		i += 1;
		if b == 0 || b == b'\r' || b == b'\n' {
			if ok && len == s.name_len && len > 0 {
				s.found = true;
			}
			(at, ok, len) = (0, true, 0);
			if b == 0 {
				break;
			}
			continue;
		}
		if at < s.name_len && b.eq_ignore_ascii_case(&name[at]) {
			at += 1;
		} else {
			ok = false;
		}
		len += 1;
	}
}

/// Keeps the start of the text, a line end as a bar, for the log.
fn scan_info(p: u32) {
	let s = c();
	let info = unsafe { &mut *addr_of_mut!(INFO) };
	s.info_len = 0;
	let mut i = 0u32;
	while i < MAX_REPLY && s.info_len < info.len() {
		let b = far_byte(p, 4 + i);
		i += 1;
		if b == 0 {
			break;
		}
		if b == b'\r' {
			continue;
		}
		info[s.info_len] = if b == b'\n' { b'|' } else { b };
		s.info_len += 1;
	}
}

/// Lets Windows deliver what is queued: dispatches every message, so the wizard still paints.
fn pump() {
	unsafe {
		let m = addr_of_mut!(DMSG) as u32;
		while PeekMessage(m, 0, 0, 0, PM_REMOVE) != 0 {
			if (*addr_of_mut!(DMSG)).message as u32 == WM_QUIT {
				continue;
			}
			TranslateMessage(m);
			DispatchMessage(m);
		}
	}
}

/// Pumps messages until `done` or `ms` pass. True when `done` came first.
fn wait(done: fn(&Conv) -> bool, ms: u32) -> bool {
	let t0 = unsafe { GetTickCount() };
	loop {
		pump();
		if done(c()) {
			return true;
		}
		if unsafe { GetTickCount() }.wrapping_sub(t0) > ms {
			return false;
		}
		unsafe { Yield() };
	}
}

fn register() -> bool {
	let s = c();
	if s.registered {
		return true;
	}
	unsafe {
		let k = addr_of_mut!(DCLASS);
		(*k).wndproc = far(code_selector(), DdeProc as *const () as usize as u32);
		(*k).instance = HINST as u16;
		(*k).class_name = far(data_selector(), cs!("LiminalDde"));
		s.registered = RegisterClass(k as u32) != 0;
	}
	s.registered
}

/// Starts a conversation with Program Manager about `group`. False when there is no Program
/// Manager to talk to.
pub fn open(group: &str) -> bool {
	if !register() {
		return false;
	}
	let a = ansi(group);
	let n = (a.len() - 1).min(95);
	unsafe {
		let name = &mut *addr_of_mut!(NAME);
		name[..n].copy_from_slice(&a[..n]);
		name[n] = 0;
	}
	let hwnd = unsafe {
		CreateWindow(
			cs!("LiminalDde"),
			cs!(""),
			WS_POPUP,
			0,
			0,
			0,
			0,
			0,
			0,
			HINST,
			0,
		)
	};
	if hwnd == 0 {
		return false;
	}
	let s = c();
	let registered = s.registered;
	*s = Conv {
		hwnd,
		server: 0,
		phase: INIT,
		ack: 0,
		got: false,
		found: false,
		gone: false,
		want_info: false,
		name_len: n,
		info_len: 0,
		registered,
	};
	unsafe {
		let atom = GlobalAddAtom(cs!("PROGMAN"));
		SendMessage(HWND_BROADCAST, WM_DDE_INITIATE, hwnd, makelong(atom, atom));
		GlobalDeleteAtom(atom);
	}
	// A server may answer by posting its ACK instead.
	if s.server == 0 {
		wait(|s| s.server != 0, INITIATE_MS);
	}
	s.phase = IDLE;
	if s.server == 0 {
		unsafe { DestroyWindow(hwnd) };
		s.hwnd = 0;
		return false;
	}
	true
}

/// Sends one command. True when Program Manager acknowledged it; false when it refused, did not
/// answer in time or went away.
pub fn execute(cmd: &str) -> bool {
	let s = c();
	if s.server == 0 || s.gone || unsafe { IsWindow(s.server) } == 0 {
		return false;
	}
	let text: Vec<u8> = ansi(cmd);
	let h = unsafe { GlobalAlloc(GMEM_MOVEABLE_DDESHARE, text.len() as u32) };
	if h == 0 {
		return false;
	}
	unsafe {
		let p = GlobalLock(h);
		for (i, b) in text.iter().enumerate() {
			far_set(p, i as u32, *b);
		}
		GlobalUnlock(h);
	}
	s.phase = EXEC;
	s.ack = 0;
	let posted = unsafe { PostMessage(s.server, WM_DDE_EXECUTE, s.hwnd, makelong(0, h)) } != 0;
	if !posted {
		unsafe { GlobalFree(h) };
		s.phase = IDLE;
		return false;
	}
	let answered = wait(|s| s.ack != 0 || s.gone, TIMEOUT_MS);
	s.phase = IDLE;
	answered && s.ack == 1
}

/// Asks for the group list (`item` `PROGMAN`) or, with `want_info`, the group's own listing. True
/// when the server answered with data.
fn request(want_info: bool) -> bool {
	let s = c();
	if s.server == 0 || s.gone || unsafe { IsWindow(s.server) } == 0 {
		return false;
	}
	s.phase = REQUEST;
	s.ack = 0;
	s.got = false;
	s.want_info = want_info;
	let item = if want_info {
		addr_of_mut!(NAME) as u32
	} else {
		cs!("PROGMAN")
	};
	let atom = unsafe { GlobalAddAtom(item) };
	let posted =
		unsafe { PostMessage(s.server, WM_DDE_REQUEST, s.hwnd, makelong(CF_TEXT, atom)) } != 0;
	if !posted {
		unsafe { GlobalDeleteAtom(atom) };
		s.phase = IDLE;
		return false;
	}
	let answered = wait(|s| s.got || s.ack != 0 || s.gone, TIMEOUT_MS);
	s.phase = IDLE;
	answered && s.got
}

/// Whether the group exists: `Some` when the server said, `None` when it did not.
pub fn group_listed() -> Option<bool> {
	c().found = false;
	if request(false) {
		Some(c().found)
	} else {
		None
	}
}

/// The start of the group's own listing (its path, item count and items) for the log, or
/// nothing when the server would not give it.
pub fn group_info() -> Option<&'static str> {
	if !request(true) {
		return None;
	}
	let info = unsafe { &*addr_of_mut!(INFO) };
	core::str::from_utf8(&info[..c().info_len]).ok()
}

/// Ends the conversation and takes the window down.
pub fn close() {
	let s = c();
	if s.hwnd == 0 {
		return;
	}
	if s.server != 0 && !s.gone && unsafe { IsWindow(s.server) } != 0 {
		unsafe { PostMessage(s.server, WM_DDE_TERMINATE, s.hwnd, 0) };
		wait(|s| s.gone, TERMINATE_MS);
	}
	unsafe { DestroyWindow(s.hwnd) };
	s.hwnd = 0;
	s.server = 0;
	s.phase = IDLE;
}
