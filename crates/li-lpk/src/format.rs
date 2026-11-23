// The byte-level structures of a pack: header, footer, directory entries and chunk headers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::Error;
use crate::crc::crc32;

/// The first four bytes of every volume: `LPK` and 0x1A, which stops `TYPE` printing the binary.
pub const MAGIC: [u8; 4] = *b"LPK\x1a";
/// The first four bytes of a volume's footer.
pub const FOOTER_MAGIC: [u8; 4] = *b"LPKE";
/// The major version this crate reads and writes.
pub const VERSION_MAJOR: u8 = 1;
/// The minor version this crate writes. A reader accepts any minor of its major.
pub const VERSION_MINOR: u8 = 0;
/// The header's size in version 1.0.
pub const HEADER_LEN: usize = 48;
/// The footer's size.
pub const FOOTER_LEN: usize = 32;
/// A chunk header's size.
pub const CHUNK_HEADER_LEN: usize = 12;
/// The fixed part of a directory entry, before the names.
pub const ENTRY_FIXED: usize = 26;
/// The most bytes a directory entry may take. A buffer of this size holds any one entry.
pub const MAX_ENTRY: usize = 512;
/// The longest short name, in bytes.
pub const MAX_SHORT: usize = 96;
/// The longest long name, in bytes.
pub const MAX_LONG: usize = 255;
/// The smallest and largest chunk size, as a power of two.
pub const MIN_CHUNK_LOG2: u8 = 9;
pub const MAX_CHUNK_LOG2: u8 = 15;

/// Chunk methods.
pub const METHOD_STORED: u8 = 0;
pub const METHOD_DEFLATE: u8 = 1;
/// Not a chunk: the stream continues at the start of the next volume.
pub const METHOD_NEXT_VOLUME: u8 = 0xff;

/// Per-file flags.
pub mod flags {
	/// The file should be made executable where the system has the notion.
	pub const EXECUTABLE: u16 = 0x0001;
	/// An existing newer file is kept.
	pub const REPLACE_IF_NEWER: u16 = 0x0002;
	/// The file is shared between products and is not removed while another product uses it.
	pub const SHARED: u16 = 0x0004;
	/// The entry is an empty directory to make; it has no data.
	pub const DIRECTORY: u16 = 0x0008;
	/// A reader that sees any of these bits set and does not know them refuses the pack.
	pub const MUST_UNDERSTAND: u16 = 0xf000;
}

/// Target bits; a mask of 0 means every target.
pub mod targets {
	pub const DOS: u8 = 0x01;
	pub const WIN16: u8 = 0x02;
	pub const WIN32: u8 = 0x04;
	pub const POSIX: u8 = 0x08;
}

// The offsets are constants and the arrays fixed, so none of these checks bounds at run time.
#[inline(always)]
fn u16le<const N: usize>(b: &[u8; N], i: usize) -> u16 {
	u16::from_le_bytes([b[i], b[i + 1]])
}

#[inline(always)]
fn u32le<const N: usize>(b: &[u8; N], i: usize) -> u32 {
	u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// A volume's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
	pub minor: u8,
	/// Where the data starts; 48 in version 1.0, larger when a later minor adds fields.
	pub header_size: u16,
	pub flags: u16,
	/// This volume's number, from 1.
	pub volume: u16,
	pub volume_count: u16,
	/// The chunk size is `1 << chunk_log2`.
	pub chunk_log2: u8,
	pub set_id: u32,
	pub file_count: u32,
	/// Where the directory is, in volume 1; 0 in the others.
	pub dir_offset: u32,
	pub dir_len: u32,
	pub dir_crc: u32,
	/// The sum of the files' sizes.
	pub total_size: u32,
	/// This volume's length from its header to the end of its footer.
	pub volume_len: u32,
	/// The header's own checksum, as stored.
	pub crc: u32,
}

impl Header {
	/// Parses and validates the first 48 bytes of a volume.
	pub fn parse(b: &[u8; HEADER_LEN]) -> Result<Header, Error> {
		if b[..4] != MAGIC {
			return Err(Error::BadMagic);
		}
		if b[4] != VERSION_MAJOR {
			return Err(Error::UnsupportedVersion);
		}
		let crc = u32le(b, 44);
		if crc32(&b[..44]) != crc {
			return Err(Error::HeaderChecksum);
		}
		let h = Header {
			minor: b[5],
			header_size: u16le(b, 6),
			flags: u16le(b, 8),
			volume: u16le(b, 10),
			volume_count: u16le(b, 12),
			chunk_log2: b[14],
			set_id: u32le(b, 16),
			file_count: u32le(b, 20),
			dir_offset: u32le(b, 24),
			dir_len: u32le(b, 28),
			dir_crc: u32le(b, 32),
			total_size: u32le(b, 36),
			volume_len: u32le(b, 40),
			crc,
		};
		if h.flags & 0x00ff != 0 {
			return Err(Error::UnsupportedFeature);
		}
		let start = u32::from(h.header_size);
		if !(HEADER_LEN as u32..=1024).contains(&start)
			|| h.volume == 0
			|| h.volume > h.volume_count
			|| !(MIN_CHUNK_LOG2..=MAX_CHUNK_LOG2).contains(&h.chunk_log2)
			|| h.volume_len < start + FOOTER_LEN as u32
		{
			return Err(Error::BadHeader);
		}
		let end = h.volume_len - FOOTER_LEN as u32;
		if h.volume == 1 {
			let dir_end = h.dir_offset.checked_add(h.dir_len);
			let entries_fit = h.file_count <= h.dir_len / ENTRY_FIXED as u32 + 1
				&& (h.file_count == 0) == (h.dir_len == 0);
			if h.dir_offset < start || dir_end.is_none_or(|e| e > end) || !entries_fit {
				return Err(Error::BadHeader);
			}
		}
		Ok(h)
	}

	/// The offset of the first chunk byte a volume can hold.
	pub fn data_start(&self) -> u32 {
		u32::from(self.header_size)
	}

	/// The offset of this volume's footer.
	pub fn footer_at(&self) -> u32 {
		self.volume_len - FOOTER_LEN as u32
	}

	/// The most a chunk can decode to.
	pub fn chunk_size(&self) -> usize {
		1usize << self.chunk_log2
	}
}

/// A volume's footer. A self-extracting stub reads it from the last 32 bytes of the file, and
/// `pack_len` says how far back the pack starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footer {
	pub pack_len: u32,
	pub set_id: u32,
	pub volume: u16,
	pub volume_count: u16,
	pub header_crc: u32,
}

impl Footer {
	/// Parses a footer without a header to compare with: for a stub that only needs `pack_len`.
	pub fn parse(b: &[u8; FOOTER_LEN]) -> Result<Footer, Error> {
		if b[..4] != FOOTER_MAGIC || crc32(&b[..28]) != u32le(b, 28) {
			return Err(Error::BadFooter);
		}
		Ok(Footer {
			pack_len: u32le(b, 4),
			set_id: u32le(b, 8),
			volume: u16le(b, 12),
			volume_count: u16le(b, 14),
			header_crc: u32le(b, 16),
		})
	}

	/// Whether this footer belongs to `h`.
	pub fn matches(&self, h: &Header) -> bool {
		self.pack_len == h.volume_len
			&& self.set_id == h.set_id
			&& self.volume == h.volume
			&& self.volume_count == h.volume_count
			&& self.header_crc == h.crc
	}
}

/// A chunk's header: a stored or deflated piece of a file, or the marker for the next volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkHeader {
	pub method: u8,
	pub raw_len: u16,
	pub stored_len: u16,
	/// For the marker: the volume the stream continues in.
	pub arg: u16,
	pub crc: u32,
}

impl ChunkHeader {
	pub fn parse(b: &[u8; CHUNK_HEADER_LEN]) -> ChunkHeader {
		ChunkHeader {
			method: b[0],
			raw_len: u16le(b, 2),
			stored_len: u16le(b, 4),
			arg: u16le(b, 6),
			crc: u32le(b, 8),
		}
	}

	/// The first eight bytes, which the chunk's checksum starts from.
	pub fn prefix(b: &[u8; CHUNK_HEADER_LEN]) -> &[u8] {
		&b[..8]
	}
}

/// Checks a relative path in one pass: `/` between parts, no empty part, no `.` or `..`, no
/// control character, none of `\ : * ? " < > |`, and, when `dos` is set, every part an
/// upper-case 8.3 name.
fn path_ok(name: &[u8], dos: bool) -> bool {
	if name.is_empty() {
		return false;
	}
	for part in name.split(|&b| b == b'/') {
		if part.is_empty() || part == b"." || part == b".." {
			return false;
		}
		let (mut stem, mut ext, mut dot) = (0u8, 0u8, false);
		for &b in part {
			if b < 0x20
				|| b == 0x7f || matches!(b, b'\\' | b':' | b'*' | b'?' | b'"' | b'<' | b'>' | b'|')
			{
				return false;
			}
			if dos {
				if b == b'.' {
					if dot || stem == 0 {
						return false;
					}
					dot = true;
					continue;
				}
				if !(b.is_ascii_uppercase()
					|| b.is_ascii_digit()
					|| matches!(
						b,
						b'!' | b'#'..=b')' | b'-' | b'@' | b'^' | b'_' | b'{' | b'}' | b'~'
					)) {
					return false;
				}
				if dot {
					ext += 1;
				} else {
					stem += 1;
				}
			}
		}
		if dos && (stem > 8 || ext > 3 || (dot && ext == 0)) {
			return false;
		}
	}
	true
}

/// Whether `name` is a safe relative path: `/` between parts, no empty part, no `.` or `..`, no
/// control character and none of `\ : * ? " < > |`.
pub fn safe_path(name: &[u8]) -> bool {
	path_ok(name, false)
}

/// Whether `name` is a safe path whose every part is an upper-case 8.3 name.
pub fn short_path(name: &[u8]) -> bool {
	path_ok(name, true)
}

/// A directory entry, borrowing the directory bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry<'a> {
	pub flags: u16,
	pub size: u32,
	pub crc: u32,
	pub dos_time: u16,
	pub dos_date: u16,
	pub attr: u8,
	pub targets: u8,
	pub first_volume: u16,
	pub first_offset: u32,
	/// The 8.3 path, upper case, `/` between parts.
	pub short_name: &'a [u8],
	/// The long path in UTF-8, or empty when it is the same as the short one.
	pub long_name: &'a [u8],
}

impl Entry<'_> {
	pub fn is_dir(&self) -> bool {
		self.flags & flags::DIRECTORY != 0
	}

	pub fn is_executable(&self) -> bool {
		self.flags & flags::EXECUTABLE != 0
	}

	/// Whether the entry is for a target; a mask of 0 is every target.
	pub fn for_target(&self, target: u8) -> bool {
		self.targets == 0 || self.targets & target != 0
	}

	/// The long name when there is one, else the short one.
	pub fn name(&self) -> &[u8] {
		if self.long_name.is_empty() {
			self.short_name
		} else {
			self.long_name
		}
	}
}

/// The size of the entry that `short` and `long` take.
pub const fn entry_len(short: usize, long: usize) -> usize {
	ENTRY_FIXED + short + long
}

/// The directory: the entries of every file, in stream order.
#[derive(Debug, Clone, Copy)]
pub struct Directory<'a> {
	bytes: &'a [u8],
	count: u32,
	volume_count: u16,
}

impl<'a> Directory<'a> {
	pub(crate) fn new(bytes: &'a [u8], h: &Header) -> Directory<'a> {
		Directory {
			bytes,
			count: h.file_count,
			volume_count: h.volume_count,
		}
	}

	/// The entries, each checked as it is reached. After an error the iterator ends.
	pub fn entries(&self) -> Entries<'a> {
		Entries {
			bytes: self.bytes,
			left: self.count,
			volume_count: self.volume_count,
		}
	}

	/// The number of entries.
	pub fn len(&self) -> u32 {
		self.count
	}

	pub fn is_empty(&self) -> bool {
		self.count == 0
	}

	/// The first entry whose short name is `name`, compared without regard to ASCII case.
	pub fn find(&self, name: &[u8]) -> Result<Option<Entry<'a>>, Error> {
		for e in self.entries() {
			let e = e?;
			if e.short_name.eq_ignore_ascii_case(name) {
				return Ok(Some(e));
			}
		}
		Ok(None)
	}
}

/// The iterator over a [`Directory`].
#[derive(Debug, Clone, Copy)]
pub struct Entries<'a> {
	bytes: &'a [u8],
	left: u32,
	volume_count: u16,
}

impl<'a> Entries<'a> {
	fn one(&mut self) -> Result<Entry<'a>, Error> {
		let b = self.bytes;
		let (f, _) = b
			.split_first_chunk::<ENTRY_FIXED>()
			.ok_or(Error::BadEntry)?;
		let len = usize::from(u16le(f, 0));
		if !(ENTRY_FIXED..=MAX_ENTRY).contains(&len) || len > b.len() {
			return Err(Error::BadEntry);
		}
		let (short, long) = (usize::from(f[24]), usize::from(f[25]));
		if short == 0 || short > MAX_SHORT || ENTRY_FIXED + short + long > len {
			return Err(Error::BadEntry);
		}
		let at = ENTRY_FIXED + short;
		let e = Entry {
			flags: u16le(f, 2),
			size: u32le(f, 4),
			crc: u32le(f, 8),
			dos_time: u16le(f, 12),
			dos_date: u16le(f, 14),
			attr: f[16],
			targets: f[17],
			first_volume: u16le(f, 18),
			first_offset: u32le(f, 20),
			short_name: &b[ENTRY_FIXED..at],
			long_name: &b[at..at + long],
		};
		if e.flags & flags::MUST_UNDERSTAND != 0 {
			return Err(Error::UnsupportedFeature);
		}
		if !short_path(e.short_name) || (long != 0 && !safe_path(e.long_name)) {
			return Err(Error::BadName);
		}
		if e.is_dir() && e.size != 0 {
			return Err(Error::BadEntry);
		}
		if e.size != 0
			&& (e.first_volume == 0
				|| e.first_volume > self.volume_count
				|| e.first_offset < HEADER_LEN as u32)
		{
			return Err(Error::BadEntry);
		}
		self.bytes = &b[len..];
		Ok(e)
	}
}

impl<'a> Iterator for Entries<'a> {
	type Item = Result<Entry<'a>, Error>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.left == 0 {
			let rest = core::mem::take(&mut self.bytes);
			return (!rest.is_empty()).then_some(Err(Error::BadEntry));
		}
		self.left -= 1;
		let r = self.one();
		if r.is_err() {
			self.left = 0;
			self.bytes = &[];
		}
		Some(r)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn short_names_are_upper_case_8_3() {
		for ok in [
			"A",
			"A.B",
			"README.TXT",
			"SOUND/ADLIB.DRV",
			"A_1-2!#$.X1",
			"%&'()@^{.}~",
			"ABCDEFGH.XYZ",
		] {
			assert!(short_path(ok.as_bytes()), "{ok}");
		}
		for bad in [
			"",
			"/",
			"A/",
			"/A",
			"A//B",
			".",
			"..",
			"A/../B",
			".HID",
			"A.",
			"A..B",
			"A.B.C",
			"lower.txt",
			"ABCDEFGHI",
			"ABC.WXYZ",
			"A B",
			"A:B",
			"A\\B",
			"A*",
			"A?",
			"A\"",
			"A<",
			"A>",
			"A|",
			"A`B",
			"A\u{7f}",
			"\u{e9}",
		] {
			assert!(!short_path(bad.as_bytes()), "{bad:?}");
		}
	}

	#[test]
	fn long_names_are_safe_relative_paths() {
		for ok in [
			"sound/AdLib driver.drv",
			"a",
			"é/ü.txt",
			"Program Files/x.exe",
		] {
			assert!(safe_path(ok.as_bytes()), "{ok}");
		}
		for bad in [
			"", "/a", "a/", "a//b", "a/./b", "../a", "a/..", "a\\b", "c:x", "a\tb", "a|b",
		] {
			assert!(!safe_path(bad.as_bytes()), "{bad:?}");
		}
	}
}
