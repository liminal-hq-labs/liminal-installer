// What the runtime needs from the machine: screen, keys, files, free space, environment and time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::vec::Vec;

use crate::Error;
use crate::gui::Gui;
use crate::ui::Cells;

/// A key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
	/// A printable character (ASCII, or a code page 437 byte for the extended set).
	Char(u8),
	Enter,
	Esc,
	Tab,
	BackTab,
	Backspace,
	Delete,
	Up,
	Down,
	Left,
	Right,
	Home,
	End,
	PageUp,
	PageDown,
	/// A function key, 1 to 10.
	F(u8),
}

/// The machine. On DOS this is `li-dos-rt`; on the host it is [`crate::sim::HostPlatform`].
///
/// Paths are DOS paths, `C:\FIZZ\SOUND\ADLIB.DRV`. The runtime builds them itself and never
/// uses a name that is not 8.3.
pub trait Platform {
	/// Shows the 80 by 25 cells.
	fn present(&mut self, cells: &Cells);
	/// Puts the text cursor at (column, row), or hides it.
	fn cursor(&mut self, at: Option<(u8, u8)>);
	/// The next key, if one is waiting.
	fn poll_key(&mut self) -> Option<Key>;
	/// Whether Esc or F3 was pressed since the last call. The default drains the keyboard, as a
	/// real machine's type-ahead is dropped while files are copied.
	fn cancel_requested(&mut self) -> bool {
		let mut cancel = false;
		while let Some(k) = self.poll_key() {
			cancel |= matches!(k, Key::Esc | Key::F(3));
		}
		cancel
	}
	/// Waits up to `ms` milliseconds for a key.
	fn wait_key(&mut self, ms: u32) -> Option<Key>;
	/// Milliseconds since some fixed moment; wraps after about 49 days.
	fn millis(&mut self) -> u32;
	fn getenv(&self, name: &str) -> Option<String>;
	/// The free bytes on the drive that holds `path`.
	fn free_space(&mut self, path: &str) -> Option<u64>;
	/// Creates a directory. `Ok(true)` when it was made, `Ok(false)` when it already existed.
	fn mkdir(&mut self, path: &str) -> Result<bool, Error>;
	fn is_dir(&mut self, path: &str) -> bool;
	fn exists(&mut self, path: &str) -> bool;
	/// Creates or truncates a file for writing and returns its handle.
	fn create(&mut self, path: &str) -> Result<u32, Error>;
	fn write(&mut self, handle: u32, data: &[u8]) -> Result<(), Error>;
	fn close(&mut self, handle: u32) -> Result<(), Error>;
	fn read_file(&mut self, path: &str) -> Result<Vec<u8>, Error>;
	fn remove_file(&mut self, path: &str) -> Result<(), Error>;
	/// Removes an empty directory.
	fn remove_dir(&mut self, path: &str) -> Result<(), Error>;
	/// The length of `SETUP.LPK`.
	fn data_len(&mut self) -> u32;
	/// Reads `SETUP.LPK` at `offset`; fewer bytes than asked for are allowed, and 0 means the end.
	fn data_read(&mut self, offset: u32, buf: &mut [u8]) -> Result<usize, Error>;
	fn port_in(&mut self, port: u16) -> u8;
	fn port_out(&mut self, port: u16, value: u8);
	/// A tone on the PC speaker for `ms` milliseconds.
	fn beep(&mut self, _hz: u32, _ms: u32) {}
	/// Prints a line to the console after the screen is given back: usage errors and `/V`.
	fn report(&mut self, _text: &str) {}
	/// What the focused control is, for a screen reader or a test. Not drawn on the screen.
	fn announce(&mut self, _text: &str) {}
	/// A hook for tests: called with the step's id each time a page is entered.
	fn step(&mut self, _id: &str) {}
	/// A hook for tests: called with the screen's text whenever a screen is shown to a person.
	fn dump(&mut self, _text: &str) {}

	/// The native window that shows the pages, when there is one. The text screens are used when
	/// this returns `None`.
	fn gui(&mut self) -> Option<&mut dyn Gui> {
		None
	}
	/// What the machine's shell adds to an install: shortcuts, an uninstall entry, an uninstaller
	/// kept in the install folder. DOS has none of it, so the default is `None`; the Windows 95
	/// runtime answers with its `IShellLink` and registry code and the Windows 3.1 runtime with
	/// Program Manager.
	fn shell(&mut self) -> Option<&mut dyn Shell> {
		None
	}
}

/// How a shell makes a shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutKind {
	/// A `.lnk` file in a Start menu folder (Windows 95 and later).
	Link,
	/// An item in a Program Manager group (Windows 3.x).
	GroupItem,
}

/// What [`Shell::add_group_item`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupAdd {
	/// There is no shell to talk to (Program Manager is not running, or another shell replaced it).
	Unavailable,
	/// The item was added; `made_group` is true when the group did not exist and this call made it.
	Added { made_group: bool },
}

/// The parts of an install that depend on the machine's shell. It is separate from [`Gui`]: a page
/// presenter is the same on Windows 3.1 and Windows 95, a shell is not. The defaults are the
/// Windows 95 behaviour except where a method says otherwise.
pub trait Shell {
	/// Whether file names keep their case and length (Windows 95) or are 8.3 in upper case (DOS and
	/// Windows 3.1).
	fn long_names(&self) -> bool {
		true
	}
	/// How a shortcut is made here.
	fn shortcut_kind(&self) -> ShortcutKind {
		ShortcutKind::Link
	}
	/// Whether the product is listed for removal (Add/Remove Programs) with an uninstaller kept in
	/// the install folder. Windows 3.1 has neither.
	fn registers_uninstall(&self) -> bool {
		true
	}
	/// The folder that holds the programs on the Start menu, without a trailing backslash.
	fn start_menu_programs(&mut self) -> Option<String> {
		None
	}
	/// The folder programs are installed under by default (`C:\Program Files`).
	fn program_files(&mut self) -> Option<String> {
		None
	}
	/// Writes a shortcut file `lnk` that starts `target` in `work_dir`.
	fn make_shortcut(
		&mut self,
		_lnk: &str,
		_target: &str,
		_work_dir: &str,
		_description: &str,
	) -> Result<(), Error> {
		Ok(())
	}
	/// Adds an item to the group `group` (made when it is missing) that starts `command` in
	/// `work_dir`. [`GroupAdd::Unavailable`] says the shell could not, and the install falls back
	/// to the batch launcher.
	fn add_group_item(
		&mut self,
		_group: &str,
		_name: &str,
		_command: &str,
		_work_dir: &str,
	) -> Result<GroupAdd, Error> {
		Ok(GroupAdd::Unavailable)
	}
	/// Removes the items `items` that [`Shell::add_group_item`] made, in one conversation with the
	/// shell, and then the group itself when `made_group` says the install made it. A group that
	/// was there before is left, with whatever else it holds.
	fn remove_group(&mut self, _group: &str, _items: &[&str], _made_group: bool) {}
	/// Adds the product to Add/Remove Programs: the key's name, its values and the command that
	/// removes it.
	fn register_uninstall(&mut self, _key: &str, _values: &[(&str, &str)]) -> Result<(), Error> {
		Ok(())
	}
	/// Removes the entry [`Shell::register_uninstall`] made.
	fn unregister_uninstall(&mut self, _key: &str) {}
	/// Keeps a copy of the runtime and the compiled project in `dir` so the product can be
	/// removed later; returns the names (below `dir`) of the files it wrote.
	fn install_uninstaller(&mut self, _dir: &str, _project: &[u8]) -> Result<Vec<String>, Error> {
		Ok(Vec::new())
	}
	/// The compiled project for an uninstall that runs from the install folder, when this
	/// machine keeps one there.
	fn uninstall_project(&mut self) -> Option<Vec<u8>> {
		None
	}
}
