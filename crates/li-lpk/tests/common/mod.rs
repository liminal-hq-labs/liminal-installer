// Shared helpers for the pack tests: unpacking through the public API and repairing checksums.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use li_lpk::writer::{FileSpec, Options, pack};
use li_lpk::{Error, Reader, SinkError, SliceSource, Source, SourceError, crc32};

/// One unpacked file.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Got {
	pub short: Vec<u8>,
	pub long: Vec<u8>,
	pub flags: u16,
	pub data: Vec<u8>,
}

/// Unpacks every entry of the volumes, in directory order.
pub fn unpack_all(volumes: &[&[u8]]) -> Result<Vec<Got>, Error> {
	let mut src = SliceSource::new(volumes);
	unpack_source(&mut src)
}

pub fn unpack_source(src: &mut dyn Source) -> Result<Vec<Got>, Error> {
	let mut r = Reader::open(src)?;
	let mut dir_bytes = vec![0u8; r.directory_len()];
	let dir = r.read_directory(src, &mut dir_bytes)?;
	let mut window = vec![0u8; 32768];
	let mut out = Vec::new();
	let mut total = 0u64;
	for e in dir.entries() {
		let e = e?;
		let mut data = Vec::new();
		r.extract(src, &e, &mut window, &mut |d: &[u8]| {
			data.extend_from_slice(d);
			Ok::<(), SinkError>(())
		})?;
		total += data.len() as u64;
		out.push(Got {
			short: e.short_name.to_vec(),
			long: e.long_name.to_vec(),
			flags: e.flags,
			data,
		});
	}
	if total != u64::from(r.header().total_size) {
		return Err(Error::SizeMismatch);
	}
	Ok(out)
}

/// Packs and unpacks.
pub fn round_trip(files: &[FileSpec], opts: &Options) -> Vec<Got> {
	let packed = pack(files, opts).unwrap();
	let views: Vec<&[u8]> = packed.volumes.iter().map(Vec::as_slice).collect();
	unpack_all(&views).unwrap()
}

/// A deterministic byte stream: a mix of runs, repeats and noise so some chunks deflate and some
/// do not.
pub fn bytes(seed: u32, n: usize) -> Vec<u8> {
	let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
	let mut out = Vec::with_capacity(n);
	while out.len() < n {
		x ^= x << 13;
		x ^= x >> 17;
		x ^= x << 5;
		let run = (x % 97) as usize + 1;
		match x % 3 {
			0 => out.extend(std::iter::repeat_n((x >> 8) as u8, run)),
			1 => out.extend((0..run).map(|i| (x >> 3) as u8 ^ i as u8)),
			_ => out.extend((0..run).map(|i| (x.rotate_left(i as u32 % 31)) as u8)),
		}
	}
	out.truncate(n);
	out
}

/// Incompressible bytes.
pub fn noise(seed: u32, n: usize) -> Vec<u8> {
	let mut x = seed | 1;
	(0..n)
		.map(|_| {
			x ^= x << 13;
			x ^= x >> 17;
			x ^= x << 5;
			(x >> 11) as u8
		})
		.collect()
}

fn put32(b: &mut [u8], at: usize, v: u32) {
	b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn get32(b: &[u8], at: usize) -> u32 {
	u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Recomputes the directory, header and footer checksums of a one-volume pack after a test has
/// changed bytes in them, so the change reaches the checks behind the checksums.
pub fn fix_checksums(v: &mut [u8]) {
	let dir_off = get32(v, 24) as usize;
	let dir_len = get32(v, 28) as usize;
	if dir_off + dir_len <= v.len() {
		let c = crc32(&v[dir_off..dir_off + dir_len]);
		put32(v, 32, c);
	}
	let hc = crc32(&v[..44]);
	put32(v, 44, hc);
	let n = v.len();
	put32(v, n - 32 + 16, hc);
	let fc = crc32(&v[n - 32..n - 4]);
	put32(v, n - 4, fc);
}

/// A source that records which volumes were opened, in order.
pub struct Logged<'a> {
	pub inner: SliceSource<'a>,
	pub opened: Vec<u16>,
	pub refuse: Option<u16>,
}

impl Source for Logged<'_> {
	fn open_volume(&mut self, volume: u16) -> Result<(), SourceError> {
		self.opened.push(volume);
		if self.refuse == Some(volume) {
			return Err(SourceError::Cancelled);
		}
		self.inner.open_volume(volume)
	}

	fn read_at(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), SourceError> {
		self.inner.read_at(offset, buf)
	}
}
