// The keyboard through the BIOS (INT 16h), and the Ctrl-C and Ctrl-Break guard.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// INT 16h AH=00h/01h (the 83-key services, which every BIOS has) give an ASCII code in AL and a scan code in AH; AL is 0 for the
// extended keys. Ctrl-C and Ctrl-Break would end the program through DOS's default handler and leave the screen in whatever state
// it was in, so INT 23h (Ctrl-C) and INT 1Bh (Ctrl-Break) are hooked with a stub that only sets a flag. The runtime reads the flag
// as the cancel key.

use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicU32, Ordering};

use li_retro::Key;
use rt::{cleanup, dpmi};

/// Set by the interrupt stub; read and cleared by `take_break`.
#[no_mangle]
static LI_BREAK: AtomicU32 = AtomicU32::new(0);
/// The data selector the stub loads before it touches `LI_BREAK`.
#[no_mangle]
static LI_DS: AtomicU32 = AtomicU32::new(0);

// The stub for INT 23h and INT 1Bh: note the break and return. The flat data segment has the same base as the code segment, so
// the selector is read through CS.
global_asm!(
	".section .text.li_break,\"ax\"",
	".global li_break_stub",
	"li_break_stub:",
	"push ds",
	"push eax",
	"mov ax, cs:[LI_DS]",
	"mov ds, ax",
	"mov dword ptr [LI_BREAK], 1",
	"pop eax",
	"pop ds",
	"iretd",
);

unsafe extern "C" {
	fn li_break_stub();
}

static mut OLD_23: (u16, u32) = (0, 0);
static mut OLD_1B: (u16, u32) = (0, 0);
static mut HOOKED: bool = false;

/// Hooks Ctrl-C and Ctrl-Break, and registers the unhooking with the clean-up list.
pub fn guard() {
	// SAFETY: single-threaded; the statics are touched here and in `unguard`, which runs at exit.
	unsafe {
		if HOOKED {
			return;
		}
		LI_DS.store(u32::from(rt::start::entry_regs().ds as u16), Ordering::SeqCst);
		let cs = rt::start::entry_regs().cs as u16;
		let (Ok(a), Ok(b)) = (dpmi::get_pm_vector(0x23), dpmi::get_pm_vector(0x1B)) else {
			return;
		};
		OLD_23 = a;
		OLD_1B = b;
		let stub = li_break_stub as *const () as usize as u32;
		if dpmi::set_pm_vector(0x23, cs, stub).is_ok() && dpmi::set_pm_vector(0x1B, cs, stub).is_ok() {
			HOOKED = true;
			cleanup::register(unguard);
		}
	}
}

/// Puts the old handlers back.
pub fn unguard() {
	// SAFETY: see `guard`.
	unsafe {
		if HOOKED {
			HOOKED = false;
			let _ = dpmi::set_pm_vector(0x23, OLD_23.0, OLD_23.1);
			let _ = dpmi::set_pm_vector(0x1B, OLD_1B.0, OLD_1B.1);
		}
	}
	cleanup::forget(unguard);
}

/// Whether Ctrl-C or Ctrl-Break was pressed since the last call.
pub fn take_break() -> bool {
	LI_BREAK.swap(0, Ordering::SeqCst) != 0
}

fn int16(ax: u32) -> (u32, bool) {
	let (out, zero): (u32, u32);
	// SAFETY: INT 16h is a BIOS service with register arguments only.
	unsafe {
		asm!(
			"int 0x16",
			"setz {z:l}",
			"movzx {z:e}, {z:l}",
			z = out(reg_abcd) zero,
			inout("eax") ax => out,
		);
	}
	(out, zero != 0)
}

/// The next key, if one is waiting.
pub fn poll() -> Option<Key> {
	if take_break() {
		return Some(Key::F(3));
	}
	let (_, empty) = int16(0x0100);
	if empty {
		return None;
	}
	let (ax, _) = int16(0x0000);
	Some(decode(ax as u8, (ax >> 8) as u8))
}

/// Turns a BIOS key (ASCII code, scan code) into a `Key`. Keys the installer has no use for come out as `Char(0)`.
pub fn decode(ascii: u8, scan: u8) -> Key {
	match (ascii, scan) {
		(0, 0x0F) => Key::BackTab,
		(0, 0x48) => Key::Up,
		(0, 0x50) => Key::Down,
		(0, 0x4B) => Key::Left,
		(0, 0x4D) => Key::Right,
		(0, 0x47) => Key::Home,
		(0, 0x4F) => Key::End,
		(0, 0x49) => Key::PageUp,
		(0, 0x51) => Key::PageDown,
		(0, 0x53) => Key::Delete,
		(0, 0x3B..=0x44) => Key::F(scan - 0x3B + 1),
		(0xE0, 0x48) => Key::Up,
		(0xE0, 0x50) => Key::Down,
		(0xE0, 0x4B) => Key::Left,
		(0xE0, 0x4D) => Key::Right,
		(0xE0, 0x47) => Key::Home,
		(0xE0, 0x4F) => Key::End,
		(0xE0, 0x49) => Key::PageUp,
		(0xE0, 0x51) => Key::PageDown,
		(0xE0, 0x53) => Key::Delete,
		(13, _) => Key::Enter,
		(27, _) => Key::Esc,
		(9, _) => Key::Tab,
		(8, _) => Key::Backspace,
		(3, _) => Key::F(3),
		(c, _) if c >= 0x20 => Key::Char(c),
		_ => Key::Char(0),
	}
}
