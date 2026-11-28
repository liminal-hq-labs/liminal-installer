// A `Platform` over a host directory, with scripted keys, for tests and for the headless host run.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::gui::{Gui, GuiEvent, GuiPage};
use crate::install::Progress;
use crate::model::Widget;
use crate::platform::{GroupAdd, Key, Platform, Shell, ShortcutKind};
use crate::progman;
use crate::ui::Cells;

/// DOS paths below `root`: `C:\FIZZ\SOUND` is `root/FIZZ/SOUND`, whatever the drive.
pub struct HostPlatform {
	pub root: PathBuf,
	/// The keys to deliver, in order.
	pub keys: VecDeque<Key>,
	pub env: BTreeMap<String, String>,
	/// The size of the pretend drive, in bytes.
	pub capacity: u64,
	/// `SETUP.LPK`.
	pub data: Vec<u8>,
	/// The focus descriptions announced, in order.
	pub announcements: Vec<String>,
	/// What the runtime printed to the console, in order.
	pub reports: Vec<String>,
	/// The text of every screen shown, in order.
	pub frames: Vec<String>,
	/// The last cells presented.
	pub last: Option<Cells>,
	/// Writes fail after this many bytes have been written, to test rollback.
	pub fail_after: Option<u64>,
	/// The tones played: (Hz, ms).
	pub beeps: Vec<(u32, u32)>,
	/// What reading each port gives, for the sound probes.
	pub ports: BTreeMap<u16, u8>,
	/// The ports written, in order.
	pub port_writes: Vec<(u16, u8)>,
	/// Behave as a Windows machine: long names keep their case, shortcuts, an uninstall entry.
	pub native: bool,
	/// Behave as a Windows 3.1 machine: Program Manager group items, 8.3 names, no uninstall entry.
	pub program_manager: bool,
	/// The Program Manager items made: group, name and command.
	pub groups: Vec<(String, String, String)>,
	/// The Program Manager groups that exist. A group an install makes is added; one a test puts
	/// here first is a group the install finds and must leave.
	pub group_names: Vec<String>,
	/// The commands the Windows 3.1 runtime would send to Program Manager, in order.
	pub progman_log: Vec<String>,
	/// Program Manager is not there (another shell): `add_group_item` says so.
	pub progman_absent: bool,
	/// The answers a person gives a native window.
	pub gui: Option<ScriptGui>,
	/// The ids of the steps entered, in order.
	pub steps: Vec<String>,
	/// The Add/Remove Programs entries: key to values.
	pub registry: BTreeMap<String, Vec<(String, String)>>,
	/// The file types the machine knows, extension (no dot) to owner: a ProgID on a Windows 95
	/// machine, a command line (`PROGRAM ^.ext`) on a Windows 3.1 one. A test seeds it with what
	/// owned an extension before the install.
	pub associations: BTreeMap<String, String>,
	/// The ProgID names registered (Windows 95): ProgID to its description.
	pub prog_ids: BTreeMap<String, String>,
	/// Behave as a machine with a Windows folder (`C:\WINDOWS`) and a system folder
	/// (`C:\WINDOWS\SYSTEM`); see [`HostPlatform::with_windows`].
	pub windows: bool,
	/// The lines the runtime sent to its log ([`Platform::notice`]), in order.
	pub notices: Vec<String>,
	/// Host paths a program holds open: removing one fails until the entry is taken out.
	pub locked: Vec<String>,
	/// What the machine was asked to remove at its next restart, in order.
	pub restart_removals: Vec<String>,
	/// The programs run, in order: the path, the arguments and the wait.
	pub programs: Vec<(String, String, u32)>,
	/// The exit code each program run answers with, by program name (the part after the last
	/// backslash, upper case); a program not named exits 0. A name in `releases` lets go of the
	/// file in `locked` it names first.
	pub exit_codes: BTreeMap<String, u32>,
	/// A program (by upper-case name) whose run lets go of these locked files.
	pub releases: BTreeMap<String, Vec<String>>,
	/// What `uninstall_project` returns, once: the compiled project kept in the install folder.
	pub kept_project: Option<Vec<u8>>,
	clock: u32,
	idle: u32,
	written: u64,
	handles: BTreeMap<u32, fs::File>,
	next: u32,
}

impl HostPlatform {
	pub fn new(root: &Path, data: Vec<u8>) -> HostPlatform {
		HostPlatform {
			root: root.to_path_buf(),
			keys: VecDeque::new(),
			env: BTreeMap::new(),
			capacity: 64 * 1024 * 1024,
			data,
			announcements: Vec::new(),
			reports: Vec::new(),
			frames: Vec::new(),
			last: None,
			fail_after: None,
			beeps: Vec::new(),
			ports: BTreeMap::new(),
			port_writes: Vec::new(),
			native: false,
			program_manager: false,
			groups: Vec::new(),
			group_names: Vec::new(),
			progman_log: Vec::new(),
			progman_absent: false,
			gui: None,
			steps: Vec::new(),
			registry: BTreeMap::new(),
			associations: BTreeMap::new(),
			prog_ids: BTreeMap::new(),
			kept_project: None,
			windows: false,
			notices: Vec::new(),
			locked: Vec::new(),
			restart_removals: Vec::new(),
			programs: Vec::new(),
			exit_codes: BTreeMap::new(),
			releases: BTreeMap::new(),
			clock: 0,
			idle: 0,
			written: 0,
			handles: BTreeMap::new(),
			next: 1,
		}
	}

	/// Gives the machine a Windows folder and a system folder, made on the host and empty.
	pub fn with_windows(&mut self) -> &mut Self {
		self.windows = true;
		let _ = fs::create_dir_all(self.host("C:\\WINDOWS\\SYSTEM"));
		self
	}

	pub fn keys(&mut self, keys: &[Key]) -> &mut Self {
		self.keys.extend(keys.iter().copied());
		self
	}

	/// Types text, then the keys after it.
	pub fn type_text(&mut self, text: &str) -> &mut Self {
		self.keys.extend(text.bytes().map(Key::Char));
		self
	}

	/// The host path of a DOS path.
	pub fn host(&self, dos: &str) -> PathBuf {
		let rest = dos.split_once(':').map_or(dos, |(_, r)| r);
		let mut p = self.root.clone();
		for part in rest.split(['\\', '/']).filter(|s| !s.is_empty()) {
			p.push(part);
		}
		p
	}

	fn used(dir: &Path) -> u64 {
		let mut n = 0;
		if let Ok(rd) = fs::read_dir(dir) {
			for e in rd.flatten() {
				match e.metadata() {
					Ok(m) if m.is_dir() => n += Self::used(&e.path()),
					Ok(m) => n += m.len(),
					Err(_) => {}
				}
			}
		}
		n
	}

	fn io(path: &str, e: std::io::Error) -> Error {
		Error::Io(alloc::format!("{path}: {e}"))
	}

	/// Every file below the root, as `DIR\NAME` paths with their contents' lengths, sorted.
	pub fn tree(&self) -> Vec<String> {
		fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
			if let Ok(rd) = fs::read_dir(dir) {
				for e in rd.flatten() {
					let p = e.path();
					let rel = p
						.strip_prefix(base)
						.unwrap()
						.to_string_lossy()
						.replace('/', "\\");
					if p.is_dir() {
						out.push(alloc::format!("{rel}\\"));
						walk(base, &p, out);
					} else {
						out.push(rel);
					}
				}
			}
		}
		let mut out = Vec::new();
		walk(&self.root, &self.root, &mut out);
		out.sort();
		out
	}
}

impl Platform for HostPlatform {
	fn present(&mut self, cells: &Cells) {
		self.last = Some(cells.clone());
	}

	fn cursor(&mut self, _at: Option<(u8, u8)>) {}

	fn poll_key(&mut self) -> Option<Key> {
		self.keys.pop_front()
	}

	fn cancel_requested(&mut self) -> bool {
		if self.keys.front() == Some(&Key::Esc) {
			self.keys.pop_front();
			return true;
		}
		false
	}

	fn wait_key(&mut self, ms: u32) -> Option<Key> {
		if let Some(k) = self.keys.pop_front() {
			return Some(k);
		}
		self.clock = self.clock.wrapping_add(ms.max(1));
		self.idle += 1;
		assert!(
			self.idle < 100_000,
			"the runtime is waiting for a key the test did not script"
		);
		None
	}

	fn millis(&mut self) -> u32 {
		self.clock
	}

	fn getenv(&self, name: &str) -> Option<String> {
		self.env.get(name).cloned()
	}

	fn free_space(&mut self, _path: &str) -> Option<u64> {
		Some(self.capacity.saturating_sub(Self::used(&self.root)))
	}

	fn mkdir(&mut self, path: &str) -> Result<bool, Error> {
		let h = self.host(path);
		if h.is_dir() {
			return Ok(false);
		}
		fs::create_dir(&h).map_err(|e| Self::io(path, e))?;
		Ok(true)
	}

	fn is_dir(&mut self, path: &str) -> bool {
		self.host(path).is_dir()
	}

	fn exists(&mut self, path: &str) -> bool {
		self.host(path).is_file()
	}

	fn create(&mut self, path: &str) -> Result<u32, Error> {
		let f = fs::File::create(self.host(path)).map_err(|e| Self::io(path, e))?;
		let id = self.next;
		self.next += 1;
		self.handles.insert(id, f);
		Ok(id)
	}

	fn write(&mut self, handle: u32, data: &[u8]) -> Result<(), Error> {
		self.written += data.len() as u64;
		if self.fail_after.is_some_and(|n| self.written > n) {
			return Err(Error::Io("the disk is full".to_string()));
		}
		let f = self
			.handles
			.get_mut(&handle)
			.ok_or_else(|| Error::Io("bad handle".to_string()))?;
		f.write_all(data).map_err(|e| Error::Io(e.to_string()))
	}

	fn close(&mut self, handle: u32) -> Result<(), Error> {
		self.handles.remove(&handle);
		Ok(())
	}

	fn read_file(&mut self, path: &str) -> Result<Vec<u8>, Error> {
		fs::read(self.host(path)).map_err(|e| Self::io(path, e))
	}

	fn remove_file(&mut self, path: &str) -> Result<(), Error> {
		if self.locked.iter().any(|l| l.eq_ignore_ascii_case(path)) {
			return Err(Error::Io(alloc::format!("{path}: in use")));
		}
		fs::remove_file(self.host(path)).map_err(|e| Self::io(path, e))
	}

	fn remove_dir(&mut self, path: &str) -> Result<(), Error> {
		fs::remove_dir(self.host(path)).map_err(|e| Self::io(path, e))
	}

	fn data_len(&mut self) -> u32 {
		self.data.len() as u32
	}

	fn data_read(&mut self, offset: u32, buf: &mut [u8]) -> Result<usize, Error> {
		let off = offset as usize;
		if off >= self.data.len() {
			return Ok(0);
		}
		let n = buf.len().min(self.data.len() - off);
		buf[..n].copy_from_slice(&self.data[off..off + n]);
		Ok(n)
	}

	fn port_in(&mut self, port: u16) -> u8 {
		self.ports.get(&port).copied().unwrap_or(0xff)
	}

	fn port_out(&mut self, port: u16, value: u8) {
		self.port_writes.push((port, value));
	}

	fn beep(&mut self, hz: u32, ms: u32) {
		self.beeps.push((hz, ms));
	}

	fn announce(&mut self, text: &str) {
		self.announcements.push(text.to_string());
	}

	fn report(&mut self, text: &str) {
		self.reports.push(text.to_string());
	}

	fn notice(&mut self, text: &str) {
		self.notices.push(text.to_string());
	}

	fn step(&mut self, id: &str) {
		self.steps.push(id.to_string());
	}

	fn gui(&mut self) -> Option<&mut dyn Gui> {
		self.gui.as_mut().map(|g| g as &mut dyn Gui)
	}

	fn shell(&mut self) -> Option<&mut dyn Shell> {
		if self.native || self.program_manager || self.windows {
			Some(self)
		} else {
			None
		}
	}

	fn dump(&mut self, text: &str) {
		if std::env::var_os("LI_SIM_TRACE").is_some() {
			std::eprintln!("{text}");
		}
		self.frames.push(text.to_string());
	}
}

/// One thing a person does on a native page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiOp {
	/// Types a value into the field bound to `bind`.
	Set(String, String),
	/// Ticks or unticks the checklist item `id`.
	Toggle(String),
	/// Chooses the radio item `id` of the group bound to `bind`.
	Choose(String, String),
	/// Presses the button whose action is the value.
	Click(String),
}

/// A native window played by a script, for tests: every page it shows is recorded, and each call
/// applies the next [`GuiOp`] queued for that page's step. A page with nothing queued is left
/// by pressing its default button.
#[derive(Default)]
pub struct ScriptGui {
	/// What to do on each step, in order.
	pub script: BTreeMap<String, VecDeque<GuiOp>>,
	/// `(step, title, the text of its widgets)` for every page first shown.
	pub pages: Vec<(String, String, Vec<String>)>,
	/// `GuiPage::describe` of every page first shown, in order.
	pub described: Vec<String>,
	/// `(step, number, total)`: the "Step n of m" a page was given.
	pub numbers: Vec<(String, usize, usize)>,
	/// The buttons seen on each page with whether they were enabled.
	pub buttons: Vec<(String, Vec<(String, bool)>)>,
	/// The file names seen while working, and the total percentage of the last update.
	pub files_seen: Vec<String>,
	pub last_total: u32,
	pub messages: Vec<String>,
	/// Cancel the copy after this many progress updates.
	pub cancel_after: Option<u32>,
	updates: u32,
}

impl ScriptGui {
	pub fn new() -> ScriptGui {
		ScriptGui::default()
	}

	/// Queues what to do on `step`.
	pub fn on(&mut self, step: &str, ops: Vec<GuiOp>) -> &mut Self {
		self.script.entry(step.to_string()).or_default().extend(ops);
		self
	}
}

impl Gui for ScriptGui {
	fn page(&mut self, page: &mut GuiPage<'_>) -> GuiEvent {
		if page.fresh {
			self.described.push(page.describe());
			self.numbers
				.push((page.step.to_string(), page.number, page.total));
			let mut text = Vec::new();
			let mut buttons = Vec::new();
			for w in &page.screen.widgets {
				match w {
					Widget::Text { template, .. } => text.push(page.text(template)),
					Widget::Doc { text: t, .. } => text.push(t.clone()),
					Widget::List { label, items, .. } => {
						text.push(page.text(label));
						text.extend(items.iter().map(|i| page.text(&i.label)));
					}
					Widget::Field { label, .. } => text.push(page.text(label)),
					Widget::Button { label, .. } => {
						buttons.push((page.text(label), page.enabled(w)));
					}
					_ => {}
				}
			}
			self.pages
				.push((page.step.to_string(), page.title.clone(), text));
			self.buttons.push((page.step.to_string(), buttons));
		}
		let op = self.script.get_mut(page.step).and_then(VecDeque::pop_front);
		match op {
			Some(GuiOp::Set(bind, v)) => {
				page.values.set_str(&bind, &v);
				GuiEvent::Changed
			}
			Some(GuiOp::Toggle(id)) => {
				for w in &page.screen.widgets {
					if let Widget::List {
						check: true,
						bind,
						items,
						..
					} = w && let Some(item) = items.iter().find(|i| i.id == id)
						&& !item.required
					{
						let mut on = page.values.list(bind).cloned().unwrap_or_default();
						match on.iter().position(|x| *x == id) {
							Some(p) => {
								on.remove(p);
							}
							None => on.push(id.clone()),
						}
						page.values
							.state
							.insert(bind.clone(), crate::ui::Value::List(on));
					}
				}
				GuiEvent::Changed
			}
			Some(GuiOp::Choose(bind, id)) => {
				page.values.set_str(&bind, &id);
				GuiEvent::Changed
			}
			Some(GuiOp::Click(a)) => GuiEvent::Action(a),
			None => {
				// The default button, else "next".
				let pick = page
					.screen
					.widgets
					.iter()
					.filter_map(|w| match w {
						Widget::Button {
							action, default, ..
						} if page.enabled(w) => Some((action.clone(), *default)),
						_ => None,
					})
					.find(|(_, d)| *d)
					.map(|(a, _)| a)
					.unwrap_or_else(|| "next".to_string());
				GuiEvent::Action(pick)
			}
		}
	}

	fn working(&mut self, page: &mut GuiPage<'_>, p: &Progress) -> bool {
		if page.fresh {
			self.described.push(page.describe());
			self.pages
				.push((page.step.to_string(), page.title.clone(), Vec::new()));
		}
		if self.files_seen.last() != Some(&p.file) && !p.file.is_empty() {
			self.files_seen.push(p.file.clone());
		}
		self.last_total = page.values.total_pct;
		self.updates += 1;
		!self.cancel_after.is_some_and(|n| self.updates > n)
	}

	fn message(&mut self, text: &str) {
		self.messages.push(text.to_string());
	}
}

impl HostPlatform {
	/// What names this program as the owner of `ext` here.
	fn owner(&self, prog_id: &str, exe: &str, ext: &str) -> String {
		if self.program_manager {
			alloc::format!("{exe} ^.{ext}")
		} else {
			prog_id.to_string()
		}
	}
}

impl Shell for HostPlatform {
	fn long_names(&self) -> bool {
		self.native
	}

	fn shortcut_kind(&self) -> ShortcutKind {
		if self.program_manager {
			ShortcutKind::GroupItem
		} else {
			ShortcutKind::Link
		}
	}

	fn registers_uninstall(&self) -> bool {
		self.native
	}

	fn keeps_uninstaller(&self) -> bool {
		self.native || self.program_manager
	}

	fn add_group_item(
		&mut self,
		group: &str,
		name: &str,
		command: &str,
		work_dir: &str,
	) -> Result<GroupAdd, Error> {
		if self.progman_absent {
			return Ok(GroupAdd::Unavailable);
		}
		let made_group = !self.group_names.iter().any(|g| g == group);
		let log = &mut self.progman_log;
		log.push(progman::create_group(group)?);
		log.push(progman::show_group(group)?);
		log.push(progman::replace_item(name)?);
		log.push(progman::add_item(command, name, work_dir)?);
		if made_group {
			self.group_names.push(group.to_string());
		}
		// ReplaceItem: an item of that name is replaced, not doubled.
		self.groups.retain(|(g, n, _)| !(g == group && n == name));
		self.groups
			.push((group.to_string(), name.to_string(), command.to_string()));
		Ok(GroupAdd::Added { made_group })
	}

	fn remove_group(&mut self, group: &str, items: &[&str], made_group: bool) {
		let Ok(show) = progman::show_group(group) else {
			return;
		};
		self.progman_log.push(show);
		for name in items {
			if let Ok(c) = progman::delete_item(name) {
				self.progman_log.push(c);
			}
			self.groups.retain(|(g, n, _)| !(g == group && n == name));
		}
		if made_group {
			if let Ok(c) = progman::delete_group(group) {
				self.progman_log.push(c);
			}
			self.group_names.retain(|g| g != group);
			self.groups.retain(|(g, _, _)| g != group);
		}
	}

	fn start_menu_programs(&mut self) -> Option<String> {
		let dir = "C:\\WINDOWS\\Start Menu\\Programs";
		fs::create_dir_all(self.host(dir)).ok()?;
		Some(dir.to_string())
	}

	fn windows_dir(&mut self) -> Option<String> {
		self.windows.then(|| "C:\\WINDOWS".to_string())
	}

	fn system_dir(&mut self) -> Option<String> {
		self.windows.then(|| "C:\\WINDOWS\\SYSTEM".to_string())
	}

	fn copy_file(&mut self, from: &str, to: &str) -> Result<(), Error> {
		fs::copy(self.host(from), self.host(to))
			.map(|_| ())
			.map_err(|e| Self::io(from, e))
	}

	fn program_files(&mut self) -> Option<String> {
		Some("C:\\Program Files".to_string())
	}

	fn make_shortcut(
		&mut self,
		lnk: &str,
		target: &str,
		work_dir: &str,
		description: &str,
	) -> Result<(), Error> {
		let body = alloc::format!(
			"target={target}\r\nstart_in={work_dir}\r\ndescription={description}\r\n"
		);
		fs::write(self.host(lnk), body).map_err(|e| Self::io(lnk, e))
	}

	fn startup_folder(&mut self) -> Option<String> {
		let dir = "C:\\WINDOWS\\Start Menu\\Programs\\StartUp";
		fs::create_dir_all(self.host(dir)).ok()?;
		Some(dir.to_string())
	}

	fn make_shortcut_args(
		&mut self,
		lnk: &str,
		target: &str,
		args: &str,
		work_dir: &str,
		description: &str,
	) -> Result<(), Error> {
		let body = alloc::format!(
			"target={target}\r\narguments={args}\r\nstart_in={work_dir}\r\ndescription={description}\r\n"
		);
		fs::write(self.host(lnk), body).map_err(|e| Self::io(lnk, e))
	}

	fn run_program(
		&mut self,
		program: &str,
		args: &str,
		wait_ms: u32,
	) -> Result<crate::platform::ProgramExit, Error> {
		use crate::platform::ProgramExit;
		self.programs
			.push((program.to_string(), args.to_string(), wait_ms));
		let name = program
			.rsplit('\\')
			.next()
			.unwrap_or(program)
			.to_ascii_uppercase();
		if let Some(freed) = self.releases.get(&name).cloned() {
			self.locked
				.retain(|l| !freed.iter().any(|f| f.eq_ignore_ascii_case(l)));
		}
		if wait_ms == 0 {
			return Ok(ProgramExit::Started);
		}
		match self.exit_codes.get(&name) {
			Some(&u32::MAX) => Ok(ProgramExit::TimedOut),
			Some(&code) => Ok(ProgramExit::Exited(code)),
			None => Ok(ProgramExit::Exited(0)),
		}
	}

	fn delete_at_restart(&mut self, path: &str) -> bool {
		self.restart_removals.push(path.to_string());
		true
	}

	fn file_types(&self) -> bool {
		self.native || self.program_manager
	}

	fn register_file_type(
		&mut self,
		ext: &str,
		prog_id: &str,
		name: &str,
		exe: &str,
	) -> Result<Option<String>, Error> {
		let own = self.owner(prog_id, exe, ext);
		if self.program_manager {
			self.progman_log
				.push(alloc::format!("[Extensions] {ext}={own}"));
		} else {
			self.prog_ids.insert(prog_id.to_string(), name.to_string());
		}
		Ok(self.associations.insert(ext.to_string(), own))
	}

	fn unregister_file_type(
		&mut self,
		ext: &str,
		prog_id: &str,
		exe: &str,
		previous: Option<&str>,
	) {
		let own = self.owner(prog_id, exe, ext);
		// Only an extension that still names this program goes back; one changed since stays.
		if self.associations.get(ext) == Some(&own) {
			match previous {
				Some(p) => self.associations.insert(ext.to_string(), p.to_string()),
				None => self.associations.remove(ext),
			};
		}
		self.prog_ids.remove(prog_id);
	}

	fn register_uninstall(&mut self, key: &str, values: &[(&str, &str)]) -> Result<(), Error> {
		self.registry.insert(
			key.to_string(),
			values
				.iter()
				.map(|(k, v)| (k.to_string(), v.to_string()))
				.collect(),
		);
		Ok(())
	}

	fn unregister_uninstall(&mut self, key: &str) {
		self.registry.remove(key);
	}

	/// A Windows 3.1 machine streams the project, as the real runtime does, so its host test
	/// exercises `Pack::write_project` too.
	fn streams_project(&self) -> bool {
		self.program_manager
	}

	fn install_uninstaller(&mut self, dir: &str, project: &[u8]) -> Result<Vec<String>, Error> {
		fs::write(self.host(&alloc::format!("{dir}\\UNINST.EXE")), b"MZ stub")
			.map_err(|e| Self::io(dir, e))?;
		if self.streams_project() {
			return Ok(alloc::vec!["UNINST.EXE".to_string()]);
		}
		fs::write(self.host(&alloc::format!("{dir}\\UNINST.DAT")), project)
			.map_err(|e| Self::io(dir, e))?;
		Ok(alloc::vec![
			"UNINST.EXE".to_string(),
			"UNINST.DAT".to_string()
		])
	}

	fn uninstall_project(&mut self) -> Option<Vec<u8>> {
		self.kept_project.take()
	}
}
