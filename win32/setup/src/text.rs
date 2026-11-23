// Text between the runtime's UTF-8 and the machine's ANSI code page, which every `...A` call speaks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::sys;

/// The ANSI bytes for `s`, ending in a NUL. A character the code page lacks becomes `?`.
pub fn ansi(s: &str) -> Vec<u8> {
	if s.is_ascii() {
		let mut v = Vec::with_capacity(s.len() + 1);
		v.extend_from_slice(s.as_bytes());
		v.push(0);
		return v;
	}
	let wide: Vec<u16> = s.encode_utf16().collect();
	// SAFETY: the buffers are sized by the first call.
	unsafe {
		let n = sys::WideCharToMultiByte(
			0,
			0,
			wide.as_ptr(),
			wide.len() as i32,
			core::ptr::null_mut(),
			0,
			core::ptr::null(),
			core::ptr::null_mut(),
		);
		let mut out = vec![0u8; n.max(0) as usize + 1];
		sys::WideCharToMultiByte(
			0,
			0,
			wide.as_ptr(),
			wide.len() as i32,
			out.as_mut_ptr(),
			n,
			core::ptr::null(),
			core::ptr::null_mut(),
		);
		out
	}
}

/// The text of ANSI bytes, up to a NUL if there is one.
pub fn utf8(bytes: &[u8]) -> String {
	let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
	let bytes = &bytes[..end];
	if bytes.is_ascii() {
		return String::from_utf8_lossy(bytes).into_owned();
	}
	// SAFETY: the buffer is sized by the first call.
	let wide = unsafe {
		let n = sys::MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, core::ptr::null_mut(), 0);
		let mut w = vec![0u16; n.max(0) as usize];
		sys::MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, w.as_mut_ptr(), n);
		w
	};
	char::decode_utf16(wide)
		.map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
		.collect()
}

/// UTF-16 for `s`, ending in a NUL (what OLE wants).
pub fn wide(s: &str) -> Vec<u16> {
	let mut w: Vec<u16> = s.encode_utf16().collect();
	w.push(0);
	w
}
