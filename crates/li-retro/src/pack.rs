// `SETUP.LPK`: the compiled project and the payload, read through `li-lpk`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `SETUP.LPK` is a Liminal Pack (`docs/lpk-format.md`). Its first entry is the compiled project
//! ([`PROJECT_FILE`], a `postcard` encoding of [`crate::model::Project`]); every other entry is a
//! file to install, named by its 8.3 destination with `/` for `\`, in the order the install
//! copies them. [`Pack`] reads it through [`Platform::data_read`], one chunk at a time, in a
//! 32 KiB window, whatever the size of the file.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use li_lpk::{Error as LpkError, Extract, Reader, Source, SourceError};

use crate::Error;
use crate::platform::Platform;

pub use li_lpk::{Crc, crc32};

/// The directory entry that holds the compiled project. It is first in the pack, and a payload
/// file may not have this destination.
pub const PROJECT_FILE: &str = "SETUP.PRJ";

/// The pack's volume 1 is `SETUP.LPK` beside `SETUP.EXE`; a set of disks is not supported yet.
struct PlatformSource<'a, P: Platform>(&'a mut P);

impl<P: Platform> Source for PlatformSource<'_, P> {
	fn open_volume(&mut self, volume: u16) -> Result<(), SourceError> {
		if volume == 1 {
			Ok(())
		} else {
			Err(SourceError::NoVolume)
		}
	}

	fn read_at(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), SourceError> {
		let mut done = 0;
		while done < buf.len() {
			match self.0.data_read(offset + done as u32, &mut buf[done..]) {
				Ok(0) => return Err(SourceError::Eof),
				Ok(n) => done += n,
				Err(_) => return Err(SourceError::Io),
			}
		}
		Ok(())
	}
}

/// Turns a pack error into the runtime's.
pub fn from_lpk(e: LpkError) -> Error {
	match e {
		LpkError::BadMagic | LpkError::UnsupportedVersion | LpkError::UnsupportedFeature => {
			Error::Format(e.message().to_string())
		}
		LpkError::Source(SourceError::Cancelled) | LpkError::Sink => Error::Cancelled,
		e => Error::Corrupt(e.message().to_string()),
	}
}

/// An open `SETUP.LPK`: the reader, the directory and the decoded-chunk window.
pub struct Pack {
	reader: Reader,
	dir: Vec<u8>,
	window: Vec<u8>,
}

/// One file being unpacked. See [`Pack::begin`].
pub struct Unpack(Extract);

impl Pack {
	/// Opens volume 1 and reads the directory.
	pub fn open<P: Platform>(p: &mut P) -> Result<Pack, Error> {
		let mut src = PlatformSource(p);
		let mut reader = Reader::open(&mut src).map_err(from_lpk)?;
		let mut dir = vec![0u8; reader.directory_len()];
		reader
			.read_directory(&mut src, &mut dir)
			.map_err(from_lpk)?;
		let window = vec![0u8; reader.header().chunk_size()];
		Ok(Pack {
			reader,
			dir,
			window,
		})
	}

	/// Starts unpacking the entry named `name` (`/` between parts), or fails when the pack has no
	/// such file.
	pub fn begin<P: Platform>(&mut self, p: &mut P, name: &str) -> Result<Unpack, Error> {
		let dir = self.reader.directory(&self.dir).map_err(from_lpk)?;
		let entry = dir
			.find(name.as_bytes())
			.map_err(from_lpk)?
			.ok_or_else(|| Error::Corrupt(alloc::format!("{name} is not in the pack")))?;
		let mut src = PlatformSource(p);
		let x = self.reader.begin(&mut src, &entry).map_err(from_lpk)?;
		Ok(Unpack(x))
	}

	/// The next decoded chunk of the file, or `None` when it is complete and its checksum matched.
	/// The bytes are valid until the next call.
	pub fn next<P: Platform>(&mut self, p: &mut P, x: &mut Unpack) -> Result<Option<&[u8]>, Error> {
		let mut src = PlatformSource(p);
		let n = self
			.reader
			.step(&mut src, &mut x.0, &mut self.window)
			.map_err(from_lpk)?;
		Ok(n.map(|n| &self.window[..n]))
	}

	/// The compiled project, which is the first entry.
	pub fn project_bytes<P: Platform>(&mut self, p: &mut P) -> Result<Vec<u8>, Error> {
		let mut x = self.begin(p, PROJECT_FILE)?;
		let mut out = Vec::new();
		while let Some(chunk) = self.next(p, &mut x)? {
			out.extend_from_slice(chunk);
		}
		Ok(out)
	}

	/// The destination of the file for `dest` (`SOUND\ADLIB.DRV`) as the pack names it.
	pub fn name_of(dest: &str) -> String {
		dest.replace('\\', "/")
	}
}
