// WIN.INI through `GetProfileString` and `WriteProfileString`, and the broadcast that says it changed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::vec;
use w16::*;

use crate::dosio::far_of;
use crate::ui::ansi;

const WM_WININICHANGE: u32 = 0x001A;
const HWND_BROADCAST: u32 = 0xFFFF;
/// A default no value can be: how `get` tells a missing line from an empty one.
const MISSING: &[u8] = b"\x01\0";

/// The value of `key` in `[section]` of WIN.INI, or `None` when there is no such line.
pub fn get(section: &str, key: &str) -> Option<String> {
	let (s, k) = (ansi(section), ansi(key));
	let mut buf = vec![0u8; 512];
	let n = unsafe {
		GetProfileString(
			s.as_ptr() as u32,
			k.as_ptr() as u32,
			MISSING.as_ptr() as u32,
			buf.as_mut_ptr() as u32,
			buf.len() as u32,
		)
	} as usize;
	let text = &buf[..n.min(buf.len() - 1)];
	if text == &MISSING[..1] {
		return None;
	}
	Some(text.iter().map(|&b| b as char).collect())
}

/// Writes the line, or deletes it when `value` is `None`. False when Windows refused.
pub fn set(section: &str, key: &str, value: Option<&str>) -> bool {
	let (s, k) = (ansi(section), ansi(key));
	unsafe {
		match value {
			Some(v) => {
				let v = ansi(v);
				WriteProfileString(s.as_ptr() as u32, k.as_ptr() as u32, v.as_ptr() as u32) != 0
			}
			None => WriteProfileString_delete(s.as_ptr() as u32, k.as_ptr() as u32, 0) != 0,
		}
	}
}

/// Tells every window that `section` of WIN.INI changed, as the Control Panel does.
pub fn changed(section: &str) {
	let s = ansi(section);
	unsafe { SendMessage(HWND_BROADCAST, WM_WININICHANGE, 0, far_of(s.as_ptr())) };
}
