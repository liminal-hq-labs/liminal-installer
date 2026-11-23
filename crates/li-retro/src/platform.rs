// What the runtime needs from the machine: screen, keys, files, free space, environment and time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::vec::Vec;

use crate::Error;
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
	/// The length of `SETUP.DAT`.
	fn data_len(&mut self) -> u32;
	/// Reads `SETUP.DAT` at `offset`.
	fn data_read(&mut self, offset: u32, buf: &mut [u8]) -> Result<usize, Error>;
	fn port_in(&mut self, port: u16) -> u8;
	fn port_out(&mut self, port: u16, value: u8);
	/// A tone on the PC speaker for `ms` milliseconds.
	fn beep(&mut self, _hz: u32, _ms: u32) {}
	/// A hook for tests: called with the screen's text whenever a screen is shown to a person.
	fn dump(&mut self, _text: &str) {}
}
