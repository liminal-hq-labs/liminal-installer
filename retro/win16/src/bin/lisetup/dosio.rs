// DOS services from a Windows 3.x task: INT 21h calls and the KERNEL file functions.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Each program in this folder uses part of this module, so what one does not use is not dead code.
#![allow(dead_code)]
//! Windows lets a protected-mode task call INT 21h, with DS:DX naming a string in the task's data
//! segment. Registers that DOS or the extender may change (the high halves of ESP, EBX, ESI, EDI,
//! and ES) are rebuilt after every call, as the import stubs do. Paths are copied into a static
//! buffer first, as ANSI bytes with a NUL.
use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;
use core::ptr::addr_of_mut;
use w16::*;

const PATH_MAX: usize = 144;
static mut PATH: [u8; PATH_MAX] = [0; PATH_MAX];

/// The registers an INT 21h call returns.
pub struct Out {
	pub ax: u16,
	pub bx: u16,
	pub cx: u16,
	pub dx: u16,
	pub carry: bool,
}

/// One INT 21h call with AX, BX, CX, DX set.
pub fn int21(ax: u16, bx: u16, cx: u16, dx: u16) -> Out {
	let (oax, ocx, odx): (u16, u16, u16);
	let bxc: u32;
	unsafe {
		asm!(
			"push ebx",
			"mov bx, di",
			"int 0x21",
			"mov di, bx",
			"sbb ebx, ebx",
			"shl ebx, 16",
			"movzx edi, di",
			"or edi, ebx",
			"movzx esp, sp",
			"pop ebx",
			"push ds",
			"pop es",
			inout("ax") ax => oax,
			inout("edi") u32::from(bx) => bxc,
			inout("cx") cx => ocx,
			inout("dx") dx => odx,
			options(nostack),
		);
	}
	Out {
		ax: oax,
		bx: bxc as u16,
		cx: ocx,
		dx: odx,
		carry: bxc >> 16 != 0,
	}
}

/// Copies `path` into the shared buffer as an ANSI, NUL-terminated string and returns its offset.
pub fn path_ptr(path: &str) -> u16 {
	let buf = unsafe { &mut *addr_of_mut!(PATH) };
	let mut n = 0;
	for c in path.chars() {
		if n + 1 >= PATH_MAX {
			break;
		}
		buf[n] = if (c as u32) < 0x80 { c as u8 } else { b'?' };
		n += 1;
	}
	buf[n] = 0;
	buf.as_ptr() as u16
}

fn call_path(ax: u16, path: &str, cx: u16) -> Out {
	let p = path_ptr(path);
	int21(ax, 0, cx, p)
}

/// Makes a directory. `Ok(true)` when made, `Ok(false)` when it was already there.
pub fn mkdir(path: &str) -> Result<bool, u16> {
	let o = call_path(0x3900, path, 0);
	if !o.carry {
		return Ok(true);
	}
	if is_dir(path) { Ok(false) } else { Err(o.ax) }
}

pub fn rmdir(path: &str) -> Result<(), u16> {
	let o = call_path(0x3A00, path, 0);
	if o.carry { Err(o.ax) } else { Ok(()) }
}

pub fn delete(path: &str) -> Result<(), u16> {
	let o = call_path(0x4100, path, 0);
	if o.carry { Err(o.ax) } else { Ok(()) }
}

/// The attributes of `path`, or None when it does not exist.
pub fn attributes(path: &str) -> Option<u16> {
	let o = call_path(0x4300, path, 0);
	if o.carry { None } else { Some(o.cx) }
}

pub fn is_dir(path: &str) -> bool {
	let root = path.len() <= 3 && path.as_bytes().get(1) == Some(&b':');
	if root {
		return free_space(path).is_some();
	}
	attributes(path).is_some_and(|a| a & 0x10 != 0)
}

pub fn exists(path: &str) -> bool {
	attributes(path).is_some_and(|a| a & 0x10 == 0)
}

/// Free bytes on the drive that holds `path` (INT 21h AH=36h), at most 2 GiB.
pub fn free_space(path: &str) -> Option<u64> {
	let drive = match path.as_bytes() {
		[d, b':', ..] if d.is_ascii_alphabetic() => d.to_ascii_uppercase() - b'A' + 1,
		_ => 0,
	};
	let o = int21(0x3600, 0, 0, u16::from(drive));
	if o.ax == 0xFFFF {
		return None;
	}
	Some(u64::from(o.ax) * u64::from(o.bx) * u64::from(o.cx))
}

pub const HFILE_ERROR: u32 = 0xFFFF;

fn ansi_ptr(path: &str) -> u32 {
	path_ptr(path) as u32
}

/// Opens a file for reading.
pub fn open(path: &str) -> Option<u32> {
	let h = unsafe { _lopen(ansi_ptr(path), 0) };
	if h == HFILE_ERROR { None } else { Some(h) }
}

pub fn create(path: &str) -> Option<u32> {
	let h = unsafe { _lcreat(ansi_ptr(path), 0) };
	if h == HFILE_ERROR { None } else { Some(h) }
}

pub fn close(h: u32) {
	unsafe { _lclose(h) };
}

/// The far pointer of a slice in the data segment.
pub fn far_of(p: *const u8) -> u32 {
	far(data_selector(), p as u32)
}

pub fn write(h: u32, data: &[u8]) -> bool {
	let mut off = 0;
	while off < data.len() {
		let n = (data.len() - off).min(0x4000);
		let got = unsafe { _lwrite_far(h, far_of(data[off..].as_ptr()), n as u32) };
		if got as usize != n {
			return false;
		}
		off += n;
	}
	true
}

/// Reads up to `buf.len()` bytes at `offset`. Returns the count, 0 at the end.
pub fn read_at(h: u32, offset: u32, buf: &mut [u8]) -> Option<usize> {
	unsafe {
		if _llseek(h, offset, 0) == u32::MAX {
			return None;
		}
		let n = buf.len().min(0x4000);
		let got = _lread(h, far_of(buf.as_ptr()), n as u32);
		if got == 0xFFFF {
			None
		} else {
			Some(got as usize)
		}
	}
}

pub fn len(h: u32) -> u32 {
	unsafe { _llseek(h, 0, 2) }
}

/// The whole file.
pub fn read_file(path: &str) -> Option<Vec<u8>> {
	let h = open(path)?;
	let n = len(h) as usize;
	let mut out = alloc::vec![0u8; n];
	let mut at = 0;
	while at < n {
		match read_at(h, at as u32, &mut out[at..]) {
			Some(0) | None => break,
			Some(k) => at += k,
		}
	}
	close(h);
	if at == n { Some(out) } else { None }
}

/// The value of the DOS environment variable `name`.
pub fn getenv(name: &str) -> Option<String> {
	let env = unsafe { GetDOSEnvironment() };
	let mut i = 0u32;
	loop {
		let mut line = String::new();
		loop {
			let b = far_byte(env, i);
			i += 1;
			if b == 0 {
				break;
			}
			line.push(b as char);
		}
		if line.is_empty() {
			return None;
		}
		if let Some((k, v)) = line.split_once('=')
			&& k.eq_ignore_ascii_case(name)
		{
			return Some(v.into());
		}
	}
}
