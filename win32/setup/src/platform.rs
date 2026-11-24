// The Windows machine: files, drives, the registry, the Start menu and the window, behind li-retro's `Platform`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::ptr::{null, null_mut};

use li_retro::gui::Gui;
use li_retro::platform::{Key, Shell};
use li_retro::ui::Cells;
use li_retro::{Error, Platform};

use crate::gui::Wizard;
use crate::shell;
use crate::sys::*;
use crate::text::{ansi, utf8};

pub struct Win32 {
	/// `SETUP.LPK`, open for reading.
	data: Handle,
	data_len: u32,
	/// Where the compiled project is kept for an uninstall run from the install folder.
	pub kept: Option<String>,
	pub wizard: Wizard,
	/// The files opened for writing, by the handle the runtime was given.
	files: Vec<Handle>,
	disk_free_ex: Option<DiskFreeEx>,
}

type DiskFreeEx = unsafe extern "system" fn(*const u8, *mut u64, *mut u64, *mut u64) -> i32;

fn io(what: &str, path: &str) -> Error {
	// SAFETY: reads the thread's last error.
	let code = unsafe { GetLastError() };
	Error::Io(format!("{what} {path} (Windows error {code})"))
}

/// The folder the program runs from, with a trailing backslash.
pub fn exe_dir() -> String {
	let mut buf = [0u8; 300];
	// SAFETY: the buffer is as long as the length passed.
	let n = unsafe { GetModuleFileNameA(NULL, buf.as_mut_ptr(), buf.len() as u32) } as usize;
	let path = utf8(&buf[..n]);
	match path.rfind('\\') {
		Some(i) => path[..=i].to_string(),
		None => String::new(),
	}
}

/// The path of the program itself.
pub fn exe_path() -> String {
	let mut buf = [0u8; 300];
	// SAFETY: the buffer is as long as the length passed.
	let n = unsafe { GetModuleFileNameA(NULL, buf.as_mut_ptr(), buf.len() as u32) } as usize;
	utf8(&buf[..n])
}

fn read_whole(path: &str) -> Result<Vec<u8>, Error> {
	let p = ansi(path);
	// SAFETY: plain file calls on a path and buffers owned here.
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
			return Err(io("cannot open", path));
		}
		let len = GetFileSize(h, null_mut());
		let mut out = vec![0u8; len as usize];
		let mut done = 0usize;
		while done < out.len() {
			let mut n = 0u32;
			let want = (out.len() - done).min(1 << 20) as u32;
			if ReadFile(h, out.as_mut_ptr().add(done), want, &mut n, null_mut()) == 0 || n == 0 {
				CloseHandle(h);
				return Err(io("cannot read", path));
			}
			done += n as usize;
		}
		CloseHandle(h);
		Ok(out)
	}
}

impl Win32 {
	/// Opens `SETUP.LPK` beside the program, or the file named by `pack`.
	pub fn open(pack: Option<&str>, wizard: Wizard) -> Win32 {
		let path = match pack {
			Some(p) => p.to_string(),
			None => format!("{}SETUP.LPK", exe_dir()),
		};
		let p = ansi(&path);
		// SAFETY: a file open on a NUL-ended path; the handle is kept until the process ends.
		let (data, data_len) = unsafe {
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
				(NULL, 0)
			} else {
				(h, GetFileSize(h, null_mut()))
			}
		};
		// GetDiskFreeSpaceEx is not in the first Windows 95, so it is looked up.
		// SAFETY: a symbol looked up by name in kernel32, called with the signature it has.
		let disk_free_ex = unsafe {
			let k = GetModuleHandleA(b"kernel32.dll\0".as_ptr());
			let f = GetProcAddress(k, b"GetDiskFreeSpaceExA\0".as_ptr());
			if f.is_null() {
				None
			} else {
				Some(core::mem::transmute::<*const core::ffi::c_void, DiskFreeEx>(f))
			}
		};
		Win32 {
			data,
			data_len,
			kept: None,
			wizard,
			files: Vec::new(),
			disk_free_ex,
		}
	}

	/// Whether the compiled project is kept beside the program, which is what the installed uninstaller finds.
	pub fn uninstall_project_exists(&mut self) -> bool {
		let p = ansi(&format!("{}UNINST.DAT", exe_dir()));
		// SAFETY: an attribute query on a NUL-ended path.
		unsafe { GetFileAttributesA(p.as_ptr()) != INVALID_FILE_ATTRIBUTES }
	}
}

impl Platform for Win32 {
	fn present(&mut self, _cells: &Cells) {}

	fn cursor(&mut self, _at: Option<(u8, u8)>) {}

	fn poll_key(&mut self) -> Option<Key> {
		None
	}

	fn cancel_requested(&mut self) -> bool {
		false
	}

	fn wait_key(&mut self, ms: u32) -> Option<Key> {
		// SAFETY: sleeps the thread.
		unsafe { Sleep(ms.min(50)) };
		None
	}

	fn millis(&mut self) -> u32 {
		// SAFETY: reads the tick count.
		unsafe { GetTickCount() }
	}

	fn getenv(&self, name: &str) -> Option<String> {
		let n = ansi(name);
		let mut buf = [0u8; 512];
		// SAFETY: the buffer is as long as the length passed.
		let len = unsafe { GetEnvironmentVariableA(n.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
		(len > 0 && len < buf.len()).then(|| utf8(&buf[..len]))
	}

	fn free_space(&mut self, path: &str) -> Option<u64> {
		let b = path.as_bytes();
		if b.len() < 2 || b[1] != b':' || !b[0].is_ascii_alphabetic() {
			return None;
		}
		let root = [b[0], b':', b'\\', 0];
		// SAFETY: out-parameters are locals; the function pointer has the signature of GetDiskFreeSpaceExA.
		unsafe {
			let old = SetErrorMode(SEM_FAILCRITICALERRORS);
			let mut r = None;
			if let Some(f) = self.disk_free_ex {
				let (mut free, mut total, mut total_free) = (0u64, 0u64, 0u64);
				if f(root.as_ptr(), &mut free, &mut total, &mut total_free) != 0 {
					r = Some(free);
				}
			} else {
				let (mut spc, mut bps, mut free, mut total) = (0u32, 0u32, 0u32, 0u32);
				if GetDiskFreeSpaceA(root.as_ptr(), &mut spc, &mut bps, &mut free, &mut total) != 0 {
					r = Some(u64::from(spc) * u64::from(bps) * u64::from(free));
				}
			}
			SetErrorMode(old);
			r
		}
	}

	fn mkdir(&mut self, path: &str) -> Result<bool, Error> {
		let p = ansi(path);
		// SAFETY: a directory call on a NUL-ended path.
		unsafe {
			if CreateDirectoryA(p.as_ptr(), null()) != 0 {
				return Ok(true);
			}
			if GetLastError() == ERROR_ALREADY_EXISTS {
				return Ok(false);
			}
		}
		Err(io("cannot create the folder", path))
	}

	fn is_dir(&mut self, path: &str) -> bool {
		let p = ansi(path);
		// SAFETY: an attribute query on a NUL-ended path.
		let a = unsafe { GetFileAttributesA(p.as_ptr()) };
		a != INVALID_FILE_ATTRIBUTES && a & FILE_ATTRIBUTE_DIRECTORY != 0
	}

	fn exists(&mut self, path: &str) -> bool {
		let p = ansi(path);
		// SAFETY: an attribute query on a NUL-ended path.
		let a = unsafe { GetFileAttributesA(p.as_ptr()) };
		a != INVALID_FILE_ATTRIBUTES && a & FILE_ATTRIBUTE_DIRECTORY == 0
	}

	fn create(&mut self, path: &str) -> Result<u32, Error> {
		let p = ansi(path);
		// SAFETY: a file create on a NUL-ended path.
		let h = unsafe {
			CreateFileA(
				p.as_ptr(),
				GENERIC_WRITE,
				0,
				null(),
				CREATE_ALWAYS,
				FILE_ATTRIBUTE_NORMAL,
				NULL,
			)
		};
		if h == INVALID_HANDLE_VALUE {
			return Err(io("cannot create", path));
		}
		// Handles are small integers to the runtime: an index into the open files.
		let slot = match self.files.iter().position(|f| f.is_null()) {
			Some(i) => {
				self.files[i] = h;
				i
			}
			None => {
				self.files.push(h);
				self.files.len() - 1
			}
		};
		Ok(slot as u32)
	}

	fn write(&mut self, handle: u32, data: &[u8]) -> Result<(), Error> {
		let h = *self
			.files
			.get(handle as usize)
			.ok_or_else(|| Error::Io("bad file handle".to_string()))?;
		let mut left = data;
		while !left.is_empty() {
			let mut n = 0u32;
			let chunk = left.len().min(1 << 20);
			// SAFETY: the buffer holds at least `chunk` bytes.
			let ok = unsafe { WriteFile(h, left.as_ptr(), chunk as u32, &mut n, null_mut()) };
			if ok == 0 || n == 0 {
				return Err(io("cannot write", "the file (is the disk full?)"));
			}
			left = &left[n as usize..];
		}
		Ok(())
	}

	fn close(&mut self, handle: u32) -> Result<(), Error> {
		let slot = self
			.files
			.get_mut(handle as usize)
			.ok_or_else(|| Error::Io("bad file handle".to_string()))?;
		let h = core::mem::replace(slot, NULL);
		// SAFETY: closes a handle this runtime opened.
		unsafe { CloseHandle(h) };
		Ok(())
	}

	fn read_file(&mut self, path: &str) -> Result<Vec<u8>, Error> {
		read_whole(path)
	}

	fn remove_file(&mut self, path: &str) -> Result<(), Error> {
		let p = ansi(path);
		// SAFETY: file calls on a NUL-ended path. A program that has only just exited can still hold its
		// own file for a moment, so a sharing violation is retried.
		unsafe {
			SetFileAttributesA(p.as_ptr(), FILE_ATTRIBUTE_NORMAL);
			for _ in 0..40 {
				if DeleteFileA(p.as_ptr()) != 0 {
					return Ok(());
				}
				match GetLastError() {
					ERROR_SHARING_VIOLATION | ERROR_ACCESS_DENIED => Sleep(100),
					_ => break,
				}
			}
		}
		Err(io("cannot remove", path))
	}

	fn remove_dir(&mut self, path: &str) -> Result<(), Error> {
		let p = ansi(path);
		// SAFETY: a directory call on a NUL-ended path.
		if unsafe { RemoveDirectoryA(p.as_ptr()) } != 0 {
			Ok(())
		} else {
			Err(io("cannot remove the folder", path))
		}
	}

	fn data_len(&mut self) -> u32 {
		self.data_len
	}

	fn data_read(&mut self, offset: u32, buf: &mut [u8]) -> Result<usize, Error> {
		if self.data.is_null() {
			return Err(Error::Io("SETUP.LPK was not found beside SETUP.EXE".to_string()));
		}
		let mut n = 0u32;
		// SAFETY: a read into the caller's buffer from the pack's handle.
		unsafe {
			if SetFilePointer(self.data, offset as i32, null_mut(), FILE_BEGIN) == INVALID_SET_FILE_POINTER {
				return Err(Error::Io("cannot seek in SETUP.LPK".to_string()));
			}
			if ReadFile(self.data, buf.as_mut_ptr(), buf.len() as u32, &mut n, null_mut()) == 0 {
				return Err(Error::Io("cannot read SETUP.LPK".to_string()));
			}
		}
		Ok(n as usize)
	}

	fn port_in(&mut self, _port: u16) -> u8 {
		0xff
	}

	fn port_out(&mut self, _port: u16, _value: u8) {}

	fn report(&mut self, text: &str) {
		if !self.wizard.dump.is_null() {
			self.wizard.log(&format!("report: {}\n", text.trim_end()));
			return;
		}
		let t = ansi(text.trim_end());
		// SAFETY: a message box with NUL-ended text.
		unsafe { MessageBoxA(NULL, t.as_ptr(), b"Setup\0".as_ptr(), MB_OK | MB_ICONINFORMATION) };
	}

	fn gui(&mut self) -> Option<&mut dyn Gui> {
		Some(&mut self.wizard)
	}

	fn shell(&mut self) -> Option<&mut dyn Shell> {
		Some(self)
	}
}

impl Shell for Win32 {
	fn start_menu_programs(&mut self) -> Option<String> {
		shell::special_folder(CSIDL_PROGRAMS)
	}

	fn program_files(&mut self) -> Option<String> {
		shell::registry_string(
			HKEY_LOCAL_MACHINE,
			"SOFTWARE\\Microsoft\\Windows\\CurrentVersion",
			"ProgramFilesDir",
		)
	}

	fn make_shortcut(&mut self, lnk: &str, target: &str, work_dir: &str, description: &str) -> Result<(), Error> {
		shell::make_shortcut(lnk, target, work_dir, description)
			.map_err(|c| Error::Io(format!("cannot write the shortcut {lnk} (COM error {c:08X})")))
	}

	fn file_types(&self) -> bool {
		true
	}

	fn register_file_type(&mut self, ext: &str, prog_id: &str, name: &str, exe: &str) -> Result<Option<String>, Error> {
		shell::register_file_type(ext, prog_id, name, exe)
			.map_err(|c| Error::Io(format!("cannot register the .{ext} file type (registry error {c})")))
	}

	fn unregister_file_type(&mut self, ext: &str, prog_id: &str, _exe: &str, previous: Option<&str>) {
		shell::unregister_file_type(ext, prog_id, previous);
	}

	fn register_uninstall(&mut self, key: &str, values: &[(&str, &str)]) -> Result<(), Error> {
		shell::register_uninstall(key, values)
			.map_err(|c| Error::Io(format!("cannot write the uninstall entry (registry error {c})")))
	}

	fn unregister_uninstall(&mut self, key: &str) {
		shell::unregister_uninstall(key);
	}

	fn install_uninstaller(&mut self, dir: &str, project: &[u8]) -> Result<Vec<String>, Error> {
		let me = ansi(&exe_path());
		let to = ansi(&format!("{dir}\\UNINST.EXE"));
		// SAFETY: copies one file between NUL-ended paths.
		if unsafe { CopyFileA(me.as_ptr(), to.as_ptr(), 0) } == 0 {
			return Err(io("cannot copy the uninstaller to", dir));
		}
		let dat = format!("{dir}\\UNINST.DAT");
		let h = self.create(&dat)?;
		let w = self.write(h, project);
		let c = self.close(h);
		w?;
		c?;
		Ok(vec!["UNINST.EXE".to_string(), "UNINST.DAT".to_string()])
	}

	fn uninstall_project(&mut self) -> Option<Vec<u8>> {
		let path = self.kept.clone().unwrap_or_else(|| format!("{}UNINST.DAT", exe_dir()));
		read_whole(&path).ok()
	}
}
