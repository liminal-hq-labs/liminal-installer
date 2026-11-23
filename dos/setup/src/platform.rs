// The DOS `Platform`: text-mode video, BIOS keys, INT 21h files, the port bus and the PC speaker.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::arch::asm;

use li_retro::ui::Cells;
use li_retro::{Error, Key, Platform};
use rt::dos::{self, File, Mode};
use rt::{dpmi, env};

use crate::{keys, video};

/// Milliseconds per BIOS timer tick (18.2065 Hz).
const TICK_MS: u32 = 55;

pub struct DosPlatform {
	data: Option<File>,
	data_len: u32,
	files: Vec<Option<File>>,
	dump: Option<File>,
	frames: u32,
}

fn io(path: &str, e: dos::DosError) -> Error {
	Error::Io(format!("{path}: DOS error {}", e.0))
}

/// The drive number INT 21h AH=36h wants for `path` (0 is the current drive).
fn drive_of(path: &str) -> u8 {
	match path.as_bytes() {
		[d, b':', ..] if d.is_ascii_alphabetic() => d.to_ascii_uppercase() - b'A' + 1,
		_ => 0,
	}
}

/// `SETUP.DAT` beside the running program, else in the current directory.
fn data_path() -> String {
	match env::program_path() {
		Some(p) => match p.rfind('\\') {
			Some(i) => format!("{}SETUP.DAT", &p[..=i]),
			None => "SETUP.DAT".to_string(),
		},
		None => "SETUP.DAT".to_string(),
	}
}

impl DosPlatform {
	pub fn new() -> DosPlatform {
		let (data, data_len) = match File::open(&data_path(), Mode::Read) {
			Ok(mut f) => {
				let n = f.len().unwrap_or(0);
				(Some(f), n)
			}
			Err(_) => (None, 0),
		};
		// A screen dump for tests: every frame shown is appended to the file `LI_DUMP` names.
		let dump = env::var("LI_DUMP").and_then(|p| File::open(&p, Mode::Create).ok());
		keys::guard();
		DosPlatform {
			data,
			data_len,
			files: Vec::new(),
			dump,
			frames: 0,
		}
	}

	/// Gives the screen back and closes what is open.
	pub fn finish(&mut self) {
		self.dump = None;
		self.files.clear();
		video::leave();
		keys::unguard();
	}

	fn ports_delay() {
		// SAFETY: a read of port 80h, the POST code port, which has no effect.
		unsafe { asm!("in al, 0x80", out("al") _, options(nomem, nostack)) };
	}
}

impl Default for DosPlatform {
	fn default() -> Self {
		Self::new()
	}
}

fn ticks() -> u32 {
	// SAFETY: the BIOS data area is in conventional memory; 0040:006C is the 32-bit tick counter.
	unsafe { dpmi::linear_to_ptr::<u32>(0x46C).read_volatile() }
}

impl Platform for DosPlatform {
	fn present(&mut self, cells: &Cells) {
		video::present(cells);
	}

	fn cursor(&mut self, at: Option<(u8, u8)>) {
		video::cursor(at);
	}

	fn poll_key(&mut self) -> Option<Key> {
		keys::poll()
	}

	fn cancel_requested(&mut self) -> bool {
		let mut cancel = keys::take_break();
		while let Some(k) = keys::poll() {
			cancel |= matches!(k, Key::Esc | Key::F(3));
		}
		cancel
	}

	fn wait_key(&mut self, ms: u32) -> Option<Key> {
		let start = ticks();
		loop {
			if let Some(k) = keys::poll() {
				return Some(k);
			}
			if ms != 0 && ticks().wrapping_sub(start).saturating_mul(TICK_MS) >= ms {
				return None;
			}
			// Give the time slice back to a multitasker (INT 2Fh AX=1680h); a plain DOS ignores it.
			Self::ports_delay();
		}
	}

	fn millis(&mut self) -> u32 {
		ticks().wrapping_mul(TICK_MS)
	}

	fn getenv(&self, name: &str) -> Option<String> {
		env::var(name)
	}

	fn free_space(&mut self, path: &str) -> Option<u64> {
		dos::free_space(drive_of(path)).ok()
	}

	fn mkdir(&mut self, path: &str) -> Result<bool, Error> {
		if self.is_dir(path) {
			return Ok(false);
		}
		dos::mkdir(path).map_err(|e| io(path, e))?;
		Ok(true)
	}

	fn is_dir(&mut self, path: &str) -> bool {
		// A drive root has no attributes of its own.
		if path.len() == 2 && path.ends_with(':') {
			return true;
		}
		dos::attributes(path).is_ok_and(|a| a & 0x10 != 0)
	}

	fn exists(&mut self, path: &str) -> bool {
		dos::attributes(path).is_ok_and(|a| a & 0x10 == 0)
	}

	fn create(&mut self, path: &str) -> Result<u32, Error> {
		let f = File::open(path, Mode::Create).map_err(|e| io(path, e))?;
		let slot = self.files.iter().position(Option::is_none);
		match slot {
			Some(i) => {
				self.files[i] = Some(f);
				Ok(i as u32)
			}
			None => {
				self.files.push(Some(f));
				Ok(self.files.len() as u32 - 1)
			}
		}
	}

	fn write(&mut self, handle: u32, data: &[u8]) -> Result<(), Error> {
		let f = self
			.files
			.get_mut(handle as usize)
			.and_then(Option::as_mut)
			.ok_or_else(|| Error::Io("bad handle".to_string()))?;
		f.write_all(data)
			.map_err(|e| Error::Io(format!("write: DOS error {} (the disk may be full)", e.0)))
	}

	fn close(&mut self, handle: u32) -> Result<(), Error> {
		if let Some(slot) = self.files.get_mut(handle as usize) {
			*slot = None;
		}
		Ok(())
	}

	fn read_file(&mut self, path: &str) -> Result<Vec<u8>, Error> {
		let mut f = File::open(path, Mode::Read).map_err(|e| io(path, e))?;
		f.read_to_vec(0).map_err(|e| io(path, e))
	}

	fn remove_file(&mut self, path: &str) -> Result<(), Error> {
		dos::remove(path).map_err(|e| io(path, e))
	}

	fn remove_dir(&mut self, path: &str) -> Result<(), Error> {
		dos::rmdir(path).map_err(|e| io(path, e))
	}

	fn data_len(&mut self) -> u32 {
		self.data_len
	}

	fn data_read(&mut self, offset: u32, buf: &mut [u8]) -> Result<usize, Error> {
		let f = self
			.data
			.as_mut()
			.ok_or_else(|| Error::Io("SETUP.DAT was not found beside SETUP.EXE".to_string()))?;
		f.seek_start(offset)
			.map_err(|e| Error::Io(format!("SETUP.DAT: DOS error {}", e.0)))?;
		f.read(buf).map_err(|e| Error::Io(format!("SETUP.DAT: DOS error {}", e.0)))
	}

	fn port_in(&mut self, port: u16) -> u8 {
		let v: u8;
		// SAFETY: a port read; DPMI clients may use the I/O ports, as sound drivers do.
		unsafe { asm!("in al, dx", in("dx") port, out("al") v, options(nomem, nostack)) };
		v
	}

	fn port_out(&mut self, port: u16, value: u8) {
		// SAFETY: a port write, as above.
		unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack)) };
	}

	fn beep(&mut self, hz: u32, ms: u32) {
		if hz == 0 {
			return;
		}
		let div = (1_193_180 / hz).clamp(20, 0xFFFF) as u16;
		// SAFETY: the PIT and the speaker gate, ports 43h, 42h and 61h: channel 2 as a square wave, then on and off.
		unsafe {
			asm!("out 0x43, al", in("al") 0xB6u8, options(nomem, nostack));
			asm!("out 0x42, al", in("al") div as u8, options(nomem, nostack));
			asm!("out 0x42, al", in("al") (div >> 8) as u8, options(nomem, nostack));
			let mut gate: u8;
			asm!("in al, 0x61", out("al") gate, options(nomem, nostack));
			asm!("out 0x61, al", in("al") gate | 3, options(nomem, nostack));
			let start = ticks();
			while ticks().wrapping_sub(start).saturating_mul(TICK_MS) < ms {}
			asm!("in al, 0x61", out("al") gate, options(nomem, nostack));
			asm!("out 0x61, al", in("al") gate & !3, options(nomem, nostack));
		}
	}

	fn report(&mut self, text: &str) {
		rt::print!("{}", text.replace('\n', "\r\n"));
	}

	fn dump(&mut self, text: &str) {
		self.frames += 1;
		if let Some(f) = self.dump.as_mut() {
			let head = format!("=== screen {} ===\n", self.frames);
			let _ = f.write_all(head.as_bytes());
			let _ = f.write_all(text.as_bytes());
		}
	}
}
