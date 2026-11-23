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
use crate::platform::{Key, Platform};
use crate::ui::Cells;

/// DOS paths below `root`: `C:\FIZZ\SOUND` is `root/FIZZ/SOUND`, whatever the drive.
pub struct HostPlatform {
	pub root: PathBuf,
	/// The keys to deliver, in order.
	pub keys: VecDeque<Key>,
	pub env: BTreeMap<String, String>,
	/// The size of the pretend drive, in bytes.
	pub capacity: u64,
	/// `SETUP.DAT`.
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
			clock: 0,
			idle: 0,
			written: 0,
			handles: BTreeMap::new(),
			next: 1,
		}
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

	fn dump(&mut self, text: &str) {
		if std::env::var_os("LI_SIM_TRACE").is_some() {
			std::eprintln!("{text}");
		}
		self.frames.push(text.to_string());
	}
}
