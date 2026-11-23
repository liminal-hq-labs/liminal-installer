// The packer: lays files out as chunks across one or more volumes and writes the directory.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Builds a pack in memory. Chunk and volume layout is deterministic: the same files, options
//! and times always give the same bytes, so a pack can be compared or signed.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use miniz_oxide::deflate::compress_to_vec;

use crate::crc::{Crc, crc32};
use crate::format::{
	CHUNK_HEADER_LEN, FOOTER_LEN, FOOTER_MAGIC, HEADER_LEN, MAGIC, MAX_CHUNK_LOG2, MAX_ENTRY,
	MAX_LONG, MAX_SHORT, METHOD_DEFLATE, METHOD_NEXT_VOLUME, METHOD_STORED, MIN_CHUNK_LOG2,
	VERSION_MAJOR, VERSION_MINOR, entry_len, flags, safe_path, short_path,
};

/// Why a pack could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackError {
	/// A short name is not an upper-case 8.3 path.
	ShortName(String),
	/// A long name is not a safe relative path, or is longer than 255 bytes.
	LongName(String),
	/// Two entries have the same short name (compared without regard to case).
	Duplicate(String),
	/// A directory entry carries data.
	DirectoryWithData(String),
	/// The chunk size is outside 512 bytes to 32 KiB.
	ChunkSize(u8),
	/// A volume is too small to hold the header, a chunk and the footer.
	VolumeTooSmall,
	/// The set would need more than 65 535 volumes, or a size does not fit 32 bits.
	TooLarge,
}

impl fmt::Display for PackError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			PackError::ShortName(n) => write!(f, "`{n}` is not an upper-case 8.3 path"),
			PackError::LongName(n) => write!(f, "`{n}` is not a usable long name"),
			PackError::Duplicate(n) => write!(f, "`{n}` is in the pack twice"),
			PackError::DirectoryWithData(n) => write!(f, "`{n}` is a directory and has data"),
			PackError::ChunkSize(l) => write!(f, "a chunk size of 2^{l} is outside 2^9 to 2^15"),
			PackError::VolumeTooSmall => f.write_str("a volume is too small for one chunk"),
			PackError::TooLarge => f.write_str("the pack is too large for the format"),
		}
	}
}

impl core::error::Error for PackError {}

/// One file, or an empty directory, to put in the pack.
#[derive(Debug, Clone)]
pub struct FileSpec {
	/// The 8.3 path, upper case, `/` between parts.
	pub short_name: String,
	/// The long path in UTF-8, or empty to use the short one.
	pub long_name: String,
	pub flags: u16,
	/// DOS attribute bits.
	pub attr: u8,
	/// Target bits, or 0 for every target.
	pub targets: u8,
	pub dos_time: u16,
	pub dos_date: u16,
	pub data: Vec<u8>,
}

/// 1980-01-01, which DOS counts as the start of time.
pub const DOS_EPOCH_DATE: u16 = 0x0021;

impl FileSpec {
	/// A file named `short_name` with the DOS epoch as its time.
	pub fn new(short_name: impl Into<String>, data: Vec<u8>) -> FileSpec {
		FileSpec {
			short_name: short_name.into(),
			long_name: String::new(),
			flags: 0,
			attr: 0x20,
			targets: 0,
			dos_time: 0,
			dos_date: DOS_EPOCH_DATE,
			data,
		}
	}

	/// An empty directory to make.
	pub fn directory(short_name: impl Into<String>) -> FileSpec {
		FileSpec {
			flags: flags::DIRECTORY,
			attr: 0x10,
			..FileSpec::new(short_name, Vec::new())
		}
	}
}

/// How to lay the pack out.
#[derive(Debug, Clone)]
pub struct Options {
	/// The chunk size as a power of two, 9 to 15. 15 is 32 KiB.
	pub chunk_log2: u8,
	/// The most bytes each volume may take, header and footer included. The last value repeats, so
	/// `[u32::MAX]` is one volume of any size. Make the first smaller to leave room for a
	/// self-extracting stub in front of it.
	pub volume_sizes: Vec<u32>,
	/// The set id; by default a checksum of the directory.
	pub set_id: Option<u32>,
	/// The deflate effort, 1 to 10.
	pub level: u8,
}

impl Default for Options {
	fn default() -> Self {
		Options {
			chunk_log2: MAX_CHUNK_LOG2,
			volume_sizes: alloc::vec![u32::MAX],
			set_id: None,
			level: 9,
		}
	}
}

/// What [`pack`] made.
#[derive(Debug, Clone)]
pub struct Packed {
	/// The volumes in order, each a complete file.
	pub volumes: Vec<Vec<u8>>,
	/// The sum of the files' sizes.
	pub raw_bytes: u64,
	/// The bytes of chunk data, chunk headers included.
	pub chunk_bytes: u64,
}

fn put16(v: &mut Vec<u8>, x: u16) {
	v.extend_from_slice(&x.to_le_bytes());
}

fn put32(v: &mut Vec<u8>, x: u32) {
	v.extend_from_slice(&x.to_le_bytes());
}

struct Chunk {
	method: u8,
	raw_len: u16,
	stored: Vec<u8>,
}

fn chunks_of(data: &[u8], opts: &Options) -> Vec<Chunk> {
	let size = 1usize << opts.chunk_log2;
	data.chunks(size)
		.map(|raw| {
			let packed = compress_to_vec(raw, opts.level);
			if packed.len() < raw.len() {
				Chunk {
					method: METHOD_DEFLATE,
					raw_len: raw.len() as u16,
					stored: packed,
				}
			} else {
				Chunk {
					method: METHOD_STORED,
					raw_len: raw.len() as u16,
					stored: raw.to_vec(),
				}
			}
		})
		.collect()
}

fn chunk_bytes(method: u8, raw_len: u16, stored: &[u8], arg: u16) -> Vec<u8> {
	let mut b = Vec::with_capacity(CHUNK_HEADER_LEN + stored.len());
	b.push(method);
	b.push(0);
	put16(&mut b, raw_len);
	put16(&mut b, stored.len() as u16);
	put16(&mut b, arg);
	let mut crc = Crc::default();
	crc.update(&b);
	crc.update(stored);
	put32(&mut b, crc.value());
	b.extend_from_slice(stored);
	b
}

fn volume_cap(opts: &Options, volume: usize) -> u32 {
	opts.volume_sizes
		.get(volume)
		.or(opts.volume_sizes.last())
		.copied()
		.unwrap_or(u32::MAX)
}

struct Placement {
	volume: u16,
	offset: u32,
}

/// Packs `files` in the order given. The directory keeps that order, and so does the data: a
/// reader that unpacks in directory order only ever moves forward through a set of disks.
pub fn pack(files: &[FileSpec], opts: &Options) -> Result<Packed, PackError> {
	if !(MIN_CHUNK_LOG2..=MAX_CHUNK_LOG2).contains(&opts.chunk_log2) {
		return Err(PackError::ChunkSize(opts.chunk_log2));
	}
	let mut seen: Vec<String> = Vec::new();
	let mut dir_len = 0usize;
	for f in files {
		if !short_path(f.short_name.as_bytes()) || f.short_name.len() > MAX_SHORT {
			return Err(PackError::ShortName(f.short_name.clone()));
		}
		if !f.long_name.is_empty()
			&& (f.long_name.len() > MAX_LONG || !safe_path(f.long_name.as_bytes()))
		{
			return Err(PackError::LongName(f.long_name.clone()));
		}
		if f.flags & flags::DIRECTORY != 0 && !f.data.is_empty() {
			return Err(PackError::DirectoryWithData(f.short_name.clone()));
		}
		if seen.iter().any(|s| s.eq_ignore_ascii_case(&f.short_name)) {
			return Err(PackError::Duplicate(f.short_name.clone()));
		}
		seen.push(f.short_name.clone());
		dir_len += entry_len(f.short_name.len(), f.long_name.len());
		if f.data.len() > u32::MAX as usize || entry_len(MAX_SHORT, MAX_LONG) > MAX_ENTRY {
			return Err(PackError::TooLarge);
		}
	}
	if dir_len > u32::MAX as usize / 2 || files.len() > u32::MAX as usize / 2 {
		return Err(PackError::TooLarge);
	}

	// Lay the chunks out volume by volume. Volume 1 starts with room for the directory.
	let mut volumes: Vec<Vec<u8>> = alloc::vec![alloc::vec![0u8; HEADER_LEN + dir_len]];
	let mut places: Vec<Placement> = Vec::with_capacity(files.len());
	let mut total = 0u64;
	let mut chunk_total = 0u64;
	let marker_room = CHUNK_HEADER_LEN as u64;
	for f in files {
		total += f.data.len() as u64;
		let mut first = true;
		let mut start = None;
		for c in chunks_of(&f.data, opts) {
			let bytes = chunk_bytes(c.method, c.raw_len, &c.stored, 0);
			let need = |cur: &Vec<u8>| {
				cur.len() as u64 + bytes.len() as u64 + marker_room + FOOTER_LEN as u64
			};
			let cap = |v: usize| u64::from(volume_cap(opts, v));
			let mut v = volumes.len() - 1;
			if need(&volumes[v]) > cap(v) {
				if !first {
					let marker = chunk_bytes(METHOD_NEXT_VOLUME, 0, &[], (v + 2) as u16);
					chunk_total += marker.len() as u64;
					volumes[v].extend_from_slice(&marker);
				}
				volumes.push(alloc::vec![0u8; HEADER_LEN]);
				v += 1;
				if volumes.len() > usize::from(u16::MAX) {
					return Err(PackError::TooLarge);
				}
				if need(&volumes[v]) > cap(v) {
					return Err(PackError::VolumeTooSmall);
				}
			}
			if start.is_none() {
				start = Some(Placement {
					volume: (v + 1) as u16,
					offset: volumes[v].len() as u32,
				});
			}
			first = false;
			chunk_total += bytes.len() as u64;
			volumes[v].extend_from_slice(&bytes);
		}
		places.push(start.unwrap_or(Placement {
			volume: 0,
			offset: 0,
		}));
	}

	// The directory.
	let mut dir = Vec::with_capacity(dir_len);
	for (f, p) in files.iter().zip(&places) {
		let len = entry_len(f.short_name.len(), f.long_name.len());
		put16(&mut dir, len as u16);
		put16(&mut dir, f.flags);
		put32(&mut dir, f.data.len() as u32);
		put32(&mut dir, crc32(&f.data));
		put16(&mut dir, f.dos_time);
		put16(&mut dir, f.dos_date);
		dir.push(f.attr);
		dir.push(f.targets);
		put16(&mut dir, p.volume);
		put32(&mut dir, p.offset);
		dir.push(f.short_name.len() as u8);
		dir.push(f.long_name.len() as u8);
		dir.extend_from_slice(f.short_name.as_bytes());
		dir.extend_from_slice(f.long_name.as_bytes());
	}
	debug_assert_eq!(dir.len(), dir_len);
	let dir_crc = crc32(&dir);
	let set_id = opts.set_id.unwrap_or(dir_crc);
	let count = volumes.len() as u16;
	let file_count = files.len() as u32;
	let total32 = u32::try_from(total).map_err(|_| PackError::TooLarge)?;

	volumes[0][HEADER_LEN..HEADER_LEN + dir_len].copy_from_slice(&dir);
	for (i, v) in volumes.iter_mut().enumerate() {
		let first_vol = i == 0;
		let volume_len = (v.len() + FOOTER_LEN) as u32;
		let mut h = Vec::with_capacity(HEADER_LEN);
		h.extend_from_slice(&MAGIC);
		h.push(VERSION_MAJOR);
		h.push(VERSION_MINOR);
		put16(&mut h, HEADER_LEN as u16);
		put16(&mut h, 0);
		put16(&mut h, (i + 1) as u16);
		put16(&mut h, count);
		h.push(opts.chunk_log2);
		h.push(0);
		put32(&mut h, set_id);
		put32(&mut h, file_count);
		put32(&mut h, if first_vol { HEADER_LEN as u32 } else { 0 });
		put32(&mut h, if first_vol { dir_len as u32 } else { 0 });
		put32(&mut h, if first_vol { dir_crc } else { 0 });
		put32(&mut h, total32);
		put32(&mut h, volume_len);
		let hcrc = crc32(&h);
		put32(&mut h, hcrc);
		v[..HEADER_LEN].copy_from_slice(&h);
		let mut f = Vec::with_capacity(FOOTER_LEN);
		f.extend_from_slice(&FOOTER_MAGIC);
		put32(&mut f, volume_len);
		put32(&mut f, set_id);
		put16(&mut f, (i + 1) as u16);
		put16(&mut f, count);
		put32(&mut f, hcrc);
		put32(&mut f, 0);
		put32(&mut f, 0);
		let fcrc = crc32(&f);
		put32(&mut f, fcrc);
		v.extend_from_slice(&f);
	}
	Ok(Packed {
		volumes,
		raw_bytes: total,
		chunk_bytes: chunk_total,
	})
}

/// Packs a DOS date and time. Years outside 1980 to 2107 are clamped.
pub fn dos_datetime(
	year: u32,
	month: u32,
	day: u32,
	hour: u32,
	minute: u32,
	second: u32,
) -> (u16, u16) {
	let y = year.clamp(1980, 2107) - 1980;
	let date = (y << 9) | (month.clamp(1, 12) << 5) | day.clamp(1, 31);
	let time = (hour.min(23) << 11) | (minute.min(59) << 5) | (second.min(59) / 2);
	(time as u16, date as u16)
}

/// A DOS time and date from seconds since 1970-01-01 UTC.
pub fn dos_from_unix(secs: i64) -> (u16, u16) {
	let days = secs.div_euclid(86_400);
	let rem = secs.rem_euclid(86_400) as u32;
	// Days since 0000-03-01 to a civil date (Howard Hinnant's algorithm).
	let z = days + 719_468;
	let era = z.div_euclid(146_097);
	let doe = z.rem_euclid(146_097);
	let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
	let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
	let mp = (5 * doy + 2) / 153;
	let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
	let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
	let y = (yoe + era * 400 + i64::from(m <= 2)).clamp(0, 9999) as u32;
	dos_datetime(y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn dos_time_of_known_instants() {
		assert_eq!(dos_from_unix(315_532_800), (0, DOS_EPOCH_DATE));
		// 2026-11-23 12:34:56 UTC
		let (t, d) = dos_from_unix(1_795_437_296);
		assert_eq!(d >> 9, 2026 - 1980);
		assert_eq!((d >> 5) & 15, 11);
		assert_eq!(d & 31, 23);
		assert_eq!(t >> 11, 12);
		assert_eq!((t >> 5) & 63, 34);
		assert_eq!(t & 31, 28);
	}
}
