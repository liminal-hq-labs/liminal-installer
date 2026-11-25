// The Windows 3.1 `Platform`: INT 21h files, Windows controls for pages, the port bus.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Each program in this folder uses part of this module, so what one does not use is not dead code.
#![allow(dead_code)]
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::arch::asm;
use core::ptr::addr_of_mut;
use li_retro::gui::Gui;
use li_retro::platform::{GroupAdd, Shell, ShortcutKind};
use li_retro::progman;
use li_retro::ui::Cells;
use li_retro::{Error, Key, Platform};
use w16::*;

use crate::dde;
use crate::dosio;
use crate::profile;
use crate::ui::{Ui, ansi};

pub struct Win16Platform {
	data: Option<u32>,
	files: Vec<u32>,
	ui: Ui,
	/// `/LOG=path`: a text log of the pages shown, for the tests.
	log_path: Option<String>,
	log: String,
	dump_path: Option<String>,
	/// `/DAT=path`: the kept project an uninstall started from `UNINST.EXE` reads, in place of `UNINST.DAT` beside the program.
	kept: Option<String>,
}

fn io(path: &str, what: &str) -> Error {
	Error::Io(format!("{path}: {what}"))
}

/// `SETUP.LPK` beside the running program.
fn data_path() -> String {
	let mut buf = [0u8; 144];
	let n = unsafe { GetModuleFileName(HINST, addr_of_mut!(buf) as u32, 143) } as usize;
	let p = String::from_utf8_lossy(&buf[..n.min(143)]).into_owned();
	match p.rfind('\\') {
		Some(i) => format!("{}SETUP.LPK", &p[..=i]),
		None => "SETUP.LPK".to_string(),
	}
}

impl Win16Platform {
	pub fn new(log_path: Option<String>) -> Win16Platform {
		Win16Platform {
			data: dosio::open(&data_path()),
			files: Vec::new(),
			ui: Ui::new(),
			log_path,
			log: String::new(),
			dump_path: None,
			kept: None,
		}
	}

	/// `/PLAY script`: presses the real controls as the script says; `/DUMP file`: the page text, written when the run ends.
	pub fn testing(&mut self, play: Option<&str>, dump: Option<String>, pause: u32) {
		self.ui.set_pause(pause);
		if let Some(text) = play {
			self.ui.play(text);
		}
		self.dump_path = dump;
	}

	pub fn set_kept(&mut self, path: Option<String>) {
		self.kept = path;
	}

	/// Writes the log file and takes the window down.
	pub fn finish(&mut self, code: i32) {
		self.ui.close();
		if let Some(p) = &self.dump_path
			&& let Some(h) = dosio::create(p)
		{
			dosio::write(h, self.ui.dump().as_bytes());
			dosio::close(h);
		}
		self.log.push_str("heap peak ");
		self.log.push_str(&crate::heap::HEAP.peak().to_string());
		self.log.push_str(" of ");
		self.log.push_str(&crate::heap::HEAP_SIZE.to_string());
		self.log.push_str(" bytes\r\n");
		self.log.push_str("exit ");
		self.log.push_str(&code.to_string());
		self.log.push_str("\r\n");
		self.save_log();
		for h in self.files.drain(..) {
			dosio::close(h);
		}
		if let Some(h) = self.data.take() {
			dosio::close(h);
		}
	}

	/// Adds a line to the log and writes it out at once, so a crash leaves the trail up to it.
	pub fn note(&mut self, line: &str) {
		self.log.push_str(line);
		self.log.push_str("\r\n");
		self.save_log();
	}

	fn save_log(&mut self) {
		if let Some(p) = &self.log_path
			&& let Some(h) = dosio::create(p)
		{
			dosio::write(h, self.log.as_bytes());
			dosio::close(h);
		}
	}
}

impl Platform for Win16Platform {
	fn present(&mut self, _cells: &Cells) {}
	fn cursor(&mut self, _at: Option<(u8, u8)>) {}
	fn poll_key(&mut self) -> Option<Key> {
		None
	}
	fn wait_key(&mut self, _ms: u32) -> Option<Key> {
		None
	}
	fn millis(&mut self) -> u32 {
		unsafe { GetTickCount() }
	}
	fn getenv(&self, name: &str) -> Option<String> {
		dosio::getenv(name)
	}
	fn free_space(&mut self, path: &str) -> Option<u64> {
		dosio::free_space(path)
	}
	fn mkdir(&mut self, path: &str) -> Result<bool, Error> {
		dosio::mkdir(path).map_err(|e| io(path, &format!("DOS error {e}")))
	}
	fn is_dir(&mut self, path: &str) -> bool {
		dosio::is_dir(path)
	}
	fn exists(&mut self, path: &str) -> bool {
		dosio::exists(path)
	}
	fn create(&mut self, path: &str) -> Result<u32, Error> {
		let h = dosio::create(path).ok_or_else(|| io(path, "cannot create the file"))?;
		self.files.push(h);
		Ok(h)
	}
	fn write(&mut self, handle: u32, data: &[u8]) -> Result<(), Error> {
		if dosio::write(handle, data) {
			Ok(())
		} else {
			Err(Error::Io(
				"the disk is full or the write failed".to_string(),
			))
		}
	}
	fn close(&mut self, handle: u32) -> Result<(), Error> {
		self.files.retain(|h| *h != handle);
		dosio::close(handle);
		Ok(())
	}
	fn read_file(&mut self, path: &str) -> Result<Vec<u8>, Error> {
		dosio::read_file(path).ok_or_else(|| io(path, "cannot read the file"))
	}
	fn remove_file(&mut self, path: &str) -> Result<(), Error> {
		dosio::delete(path).map_err(|e| io(path, &format!("DOS error {e}")))
	}
	fn remove_dir(&mut self, path: &str) -> Result<(), Error> {
		dosio::rmdir(path).map_err(|e| io(path, &format!("DOS error {e}")))
	}
	fn data_len(&mut self) -> u32 {
		self.data.map_or(0, dosio::len)
	}
	fn data_read(&mut self, offset: u32, buf: &mut [u8]) -> Result<usize, Error> {
		let h = self
			.data
			.ok_or_else(|| Error::Io("SETUP.LPK was not found".to_string()))?;
		dosio::read_at(h, offset, buf).ok_or_else(|| Error::Io("SETUP.LPK cannot be read".into()))
	}
	fn port_in(&mut self, port: u16) -> u8 {
		let v: u8;
		unsafe {
			asm!("in al, dx", in("dx") port, out("al") v, options(nomem, nostack, preserves_flags))
		};
		v
	}
	fn port_out(&mut self, port: u16, value: u8) {
		unsafe {
			asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags))
		};
	}
	fn beep(&mut self, _hz: u32, _ms: u32) {
		unsafe { MessageBeep(0) };
	}
	fn report(&mut self, text: &str) {
		unsafe {
			MessageBox(
				0,
				self.ui_ptr(&ansi(text)),
				cs!("Setup"),
				MB_ICONINFORMATION,
			);
		}
	}

	fn step(&mut self, id: &str) {
		self.note(&alloc::format!("page {id}"));
	}

	fn gui(&mut self) -> Option<&mut dyn Gui> {
		Some(&mut self.ui)
	}

	fn shell(&mut self) -> Option<&mut dyn Shell> {
		Some(self)
	}
}

/// Windows 3.1's shell is Program Manager: 8.3 names, shortcuts as items in a group, no uninstall list.
/// Items are made by a DDE conversation with `PROGMAN` (`dde.rs`) in the commands `li_retro::progman`
/// builds; when there is no Program Manager to talk to, `add_group_item` says so and the install
/// writes the batch launcher as DOS does.
impl Shell for Win16Platform {
	fn long_names(&self) -> bool {
		false
	}

	fn shortcut_kind(&self) -> ShortcutKind {
		ShortcutKind::GroupItem
	}

	fn registers_uninstall(&self) -> bool {
		false
	}

	fn keeps_uninstaller(&self) -> bool {
		true
	}

	fn streams_project(&self) -> bool {
		true
	}

	/// Copies the running program to `UNINST.EXE`; the install streams `UNINST.DAT` from the pack.
	fn install_uninstaller(&mut self, dir: &str, _project: &[u8]) -> Result<Vec<String>, Error> {
		if !dosio::copy_file(&dosio::exe_path(), &format!("{dir}\\UNINST.EXE")) {
			return Err(io(dir, "cannot copy the uninstaller"));
		}
		Ok(alloc::vec!["UNINST.EXE".to_string()])
	}

	fn uninstall_project(&mut self) -> Option<Vec<u8>> {
		let path = self
			.kept
			.clone()
			.unwrap_or_else(|| format!("{}UNINST.DAT", dosio::exe_dir()));
		dosio::read_file(&path)
	}

	fn file_types(&self) -> bool {
		true
	}

	/// An association is a line in WIN.INI's `[Extensions]` section: `ext=PROGRAM ^.ext`. The line
	/// that was there is returned, and a change is announced with `WM_WININICHANGE` so File Manager
	/// reads the section again.
	fn register_file_type(
		&mut self,
		ext: &str,
		_prog_id: &str,
		_name: &str,
		exe: &str,
	) -> Result<Option<String>, Error> {
		let previous = profile::get(EXTENSIONS, ext);
		if !profile::set(EXTENSIONS, ext, Some(&owner(exe, ext))) {
			return Err(Error::Io(format!(
				"cannot write the .{ext} line in WIN.INI"
			)));
		}
		profile::changed(EXTENSIONS);
		Ok(previous)
	}

	/// Puts `previous` back, or deletes the line when there was none, only while the line is still
	/// ours.
	fn unregister_file_type(
		&mut self,
		ext: &str,
		_prog_id: &str,
		exe: &str,
		previous: Option<&str>,
	) {
		let ours = profile::get(EXTENSIONS, ext)
			.is_some_and(|now| now.eq_ignore_ascii_case(&owner(exe, ext)));
		if ours {
			profile::set(EXTENSIONS, ext, previous.filter(|p| !p.trim().is_empty()));
			// A line that is still there, even with nothing after the `=`, is deleted when there
			// was no owner before.
			if previous.is_none() && profile::get(EXTENSIONS, ext).is_some() {
				profile::set(EXTENSIONS, ext, None);
			}
			profile::changed(EXTENSIONS);
		}
	}

	fn add_group_item(
		&mut self,
		group: &str,
		name: &str,
		command: &str,
		work_dir: &str,
	) -> Result<GroupAdd, Error> {
		// Every command is built first: a name Program Manager cannot take stops the install here.
		let create = progman::create_group(group)?;
		let show = progman::show_group(group)?;
		let replace = progman::replace_item(name)?;
		let add = progman::add_item(command, name, work_dir)?;
		let short = progman::add_item_short(command, name)?;
		if !dde::open(group) {
			self.note("progman: unavailable");
			return Ok(GroupAdd::Unavailable);
		}
		// A group that cannot be listed is taken to exist, so it is never deleted by mistake.
		let listed = dde::group_listed();
		self.note(match listed {
			Some(true) => "progman: group exists",
			Some(false) => "progman: group missing",
			None => "progman: group list unavailable",
		});
		let made_group = listed == Some(false);
		let mut added = dde::execute(&create);
		if added {
			dde::execute(&show);
			// Refused when there is no such item yet, which is fine.
			dde::execute(&replace);
			added = dde::execute(&add) || dde::execute(&short);
		}
		if !added {
			self.note("progman: add refused");
			if made_group && let Ok(delete) = progman::delete_group(group) {
				dde::execute(&delete);
			}
			dde::close();
			return Ok(GroupAdd::Unavailable);
		}
		self.note("progman: added");
		if let Some(info) = dde::group_info() {
			let line = format!("progman: group: {info}");
			self.note(&line);
		}
		dde::close();
		Ok(GroupAdd::Added { made_group })
	}

	fn remove_group(&mut self, group: &str, items: &[&str], made_group: bool) {
		if !dde::open(group) {
			self.note("progman: unavailable on removal");
			return;
		}
		if let Ok(show) = progman::show_group(group) {
			dde::execute(&show);
		}
		for item in items {
			if let Ok(c) = progman::delete_item(item) {
				dde::execute(&c);
			}
		}
		if made_group && let Ok(c) = progman::delete_group(group) {
			dde::execute(&c);
		}
		let line = match dde::group_listed() {
			Some(true) => "progman: removed; group still listed",
			Some(false) => "progman: removed; group gone",
			None => "progman: removed",
		};
		self.note(line);
		dde::close();
	}
}

/// WIN.INI's section of file associations.
const EXTENSIONS: &str = "Extensions";

/// What names this program as the owner of `ext` in `[Extensions]`.
fn owner(exe: &str, ext: &str) -> String {
	format!("{exe} ^.{ext}")
}

impl Win16Platform {
	fn ui_ptr(&self, v: &[u8]) -> u32 {
		v.as_ptr() as u32
	}
}
