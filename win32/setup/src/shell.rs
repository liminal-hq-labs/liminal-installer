// The shell: special folders, the registry, shortcut files and the folder picker.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Start menu shortcuts are `.lnk` files written through `IShellLink` and `IPersistFile` (COM, from
//! `ole32`, which Windows 95 has). The vtables are called by hand: there is no `windows` crate here and
//! no C runtime. See D55 for why this and not Program Manager DDE or writing the `.lnk` format.

use alloc::string::String;
use core::ffi::c_void;
use core::ptr::{null, null_mut};

use crate::sys::*;
use crate::text::{ansi, utf8, wide};

const CLSID_SHELL_LINK: Guid = Guid {
	d1: 0x0002_1401,
	d2: 0,
	d3: 0,
	d4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};
const IID_ISHELL_LINK_A: Guid = Guid {
	d1: 0x0002_14EE,
	d2: 0,
	d3: 0,
	d4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};
const IID_IPERSIST_FILE: Guid = Guid {
	d1: 0x0000_010B,
	d2: 0,
	d3: 0,
	d4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};

type Method = *const c_void;

/// The `n`th entry of a COM object's vtable.
///
/// # Safety
/// `obj` must be a live COM interface pointer with at least `n + 1` methods.
unsafe fn method(obj: *mut c_void, n: usize) -> Method {
	let vtbl = *(obj as *const *const Method);
	*vtbl.add(n)
}

unsafe fn release(obj: *mut c_void) {
	let f: unsafe extern "system" fn(*mut c_void) -> u32 = core::mem::transmute(method(obj, 2));
	f(obj);
}

/// Writes `lnk`, a shortcut to `target` that starts in `work_dir`. Returns the COM error code on failure.
pub fn make_shortcut(lnk: &str, target: &str, work_dir: &str, description: &str) -> Result<(), u32> {
	// SAFETY: standard IShellLink use. Every interface pointer is checked, used within its vtable's
	// signatures and released.
	unsafe {
		CoInitialize(null());
		let mut link: *mut c_void = null_mut();
		let hr = CoCreateInstance(
			&CLSID_SHELL_LINK,
			null_mut(),
			CLSCTX_INPROC_SERVER,
			&IID_ISHELL_LINK_A,
			&mut link,
		);
		if hr < 0 || link.is_null() {
			CoUninitialize();
			return Err(hr as u32);
		}
		let result = (|| {
			let (t, w, d) = (ansi(target), ansi(work_dir), ansi(description));
			type SetStr = unsafe extern "system" fn(*mut c_void, *const u8) -> i32;
			let set_path: SetStr = core::mem::transmute(method(link, 20));
			let set_dir: SetStr = core::mem::transmute(method(link, 9));
			let set_desc: SetStr = core::mem::transmute(method(link, 7));
			let hr = set_path(link, t.as_ptr());
			if hr < 0 {
				return Err(hr as u32);
			}
			set_dir(link, w.as_ptr());
			set_desc(link, d.as_ptr());
			let mut file: *mut c_void = null_mut();
			let query: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32 =
				core::mem::transmute(method(link, 0));
			let hr = query(link, &IID_IPERSIST_FILE, &mut file);
			if hr < 0 || file.is_null() {
				return Err(hr as u32);
			}
			let save: unsafe extern "system" fn(*mut c_void, *const u16, i32) -> i32 = core::mem::transmute(method(file, 6));
			let name = wide(lnk);
			let hr = save(file, name.as_ptr(), 1);
			release(file);
			if hr < 0 { Err(hr as u32) } else { Ok(()) }
		})();
		release(link);
		CoUninitialize();
		result
	}
}

/// The path of a special folder such as the Programs folder of the Start menu.
pub fn special_folder(csidl: i32) -> Option<String> {
	let mut buf = [0u8; 300];
	// SAFETY: the shell fills the buffer (at least 260 bytes) and returns an id list to free.
	unsafe {
		let mut pidl: *mut c_void = null_mut();
		if SHGetSpecialFolderLocation(NULL, csidl, &mut pidl) != 0 || pidl.is_null() {
			return None;
		}
		let ok = SHGetPathFromIDListA(pidl, buf.as_mut_ptr()) != 0;
		CoTaskMemFree(pidl);
		ok.then(|| utf8(&buf).trim_end_matches('\\').into())
	}
}

/// A string value, or `None`.
pub fn registry_string(root: usize, sub: &str, name: &str) -> Option<String> {
	let (s, n) = (ansi(sub), ansi(name));
	let mut buf = [0u8; 300];
	// SAFETY: registry calls on NUL-ended names and a local buffer.
	unsafe {
		let mut key = 0usize;
		if RegOpenKeyExA(root, s.as_ptr(), 0, KEY_READ, &mut key) != 0 {
			return None;
		}
		let (mut kind, mut len) = (0u32, buf.len() as u32);
		let ok = RegQueryValueExA(key, n.as_ptr(), null_mut(), &mut kind, buf.as_mut_ptr(), &mut len) == 0 && kind == REG_SZ;
		RegCloseKey(key);
		ok.then(|| utf8(&buf[..(len as usize).min(buf.len())]))
	}
}

const UNINSTALL: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\";

/// Adds the product to Add/Remove Programs under `HKEY_LOCAL_MACHINE`, or under the current user when
/// the machine key is closed to this account.
pub fn register_uninstall(key: &str, values: &[(&str, &str)]) -> Result<(), i32> {
	let sub = ansi(&alloc::format!("{UNINSTALL}{key}"));
	let mut last = 0;
	for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
		// SAFETY: registry calls on NUL-ended names and values.
		unsafe {
			let mut k = 0usize;
			let mut disp = 0u32;
			let r = RegCreateKeyExA(root, sub.as_ptr(), 0, null(), 0, KEY_ALL_ACCESS, null(), &mut k, &mut disp);
			if r != 0 {
				last = r;
				continue;
			}
			for (name, value) in values {
				let (n, v) = (ansi(name), ansi(value));
				RegSetValueExA(k, n.as_ptr(), 0, REG_SZ, v.as_ptr(), v.len() as u32);
			}
			RegCloseKey(k);
			return Ok(());
		}
	}
	Err(last)
}

/// Removes the entry from both places it may have been written.
pub fn unregister_uninstall(key: &str) {
	let sub = ansi(&alloc::format!("{UNINSTALL}{key}"));
	for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
		// SAFETY: a registry call on a NUL-ended name.
		unsafe { RegDeleteKeyA(root, sub.as_ptr()) };
	}
}

/// The folder picker (`SHBrowseForFolder`): the chosen path, or `None` when cancelled.
pub fn browse_folder(owner: Handle, title: &str) -> Option<String> {
	let t = ansi(title);
	let mut name = [0u8; 300];
	let mut path = [0u8; 300];
	let mut info = BrowseInfoA {
		owner,
		root: null(),
		display_name: name.as_mut_ptr(),
		title: t.as_ptr(),
		flags: BIF_RETURNONLYFSDIRS,
		callback: null(),
		lparam: 0,
		image: 0,
	};
	// SAFETY: the shell fills the buffers (at least 260 bytes) and returns an id list to free.
	unsafe {
		let pidl = SHBrowseForFolderA(&mut info);
		if pidl.is_null() {
			return None;
		}
		let ok = SHGetPathFromIDListA(pidl, path.as_mut_ptr()) != 0;
		CoTaskMemFree(pidl);
		ok.then(|| utf8(&path))
	}
}
