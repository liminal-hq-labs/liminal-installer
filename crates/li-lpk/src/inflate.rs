// A raw DEFLATE (RFC 1951) decoder that pulls its input through a small buffer and writes into
// one caller buffer, which is also its window: every chunk is compressed on its own.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::crc::Crc;
use crate::error::{Error, InflateError, SourceError};
use crate::reader::Source;

/// The size of the input buffer a reader keeps.
pub const INBUF: usize = 256;

/// The stored bytes of one chunk, read through a buffer, with a running checksum.
pub(crate) struct Input<'a> {
	pub src: &'a mut dyn Source,
	pub buf: &'a mut [u8; INBUF],
	pos: usize,
	len: usize,
	at: u32,
	left: u32,
	pub crc: Crc,
}

fn source_error(e: SourceError) -> Error {
	match e {
		SourceError::Eof => Error::Truncated,
		e => Error::Source(e),
	}
}

impl<'a> Input<'a> {
	pub fn new(
		src: &'a mut dyn Source,
		buf: &'a mut [u8; INBUF],
		at: u32,
		left: u32,
		crc: Crc,
	) -> Self {
		Input {
			src,
			buf,
			pos: 0,
			len: 0,
			at,
			left,
			crc,
		}
	}

	/// Whether every stored byte has been handed out.
	pub fn drained(&self) -> bool {
		self.pos == self.len && self.left == 0
	}

	fn refill(&mut self) -> Result<(), Error> {
		let n = (self.left as usize).min(INBUF);
		if n == 0 {
			return Err(Error::Inflate(InflateError::InputEnd));
		}
		self.src
			.read_at(self.at, &mut self.buf[..n])
			.map_err(source_error)?;
		self.crc.update(&self.buf[..n]);
		self.at += n as u32;
		self.left -= n as u32;
		self.pos = 0;
		self.len = n;
		Ok(())
	}

	fn byte(&mut self) -> Result<u8, Error> {
		if self.pos == self.len {
			self.refill()?;
		}
		let b = self.buf[self.pos];
		self.pos += 1;
		Ok(b)
	}

	/// Fills `dst` from the stored bytes.
	pub fn copy_to(&mut self, dst: &mut [u8]) -> Result<(), Error> {
		let mut done = 0;
		while done < dst.len() {
			if self.pos < self.len {
				let n = (self.len - self.pos).min(dst.len() - done);
				dst[done..done + n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
				self.pos += n;
				done += n;
			} else if self.left as usize >= dst.len() - done && dst.len() - done >= INBUF {
				let n = dst.len() - done;
				self.src
					.read_at(self.at, &mut dst[done..])
					.map_err(source_error)?;
				self.crc.update(&dst[done..]);
				self.at += n as u32;
				self.left -= n as u32;
				done += n;
			} else {
				self.refill()?;
			}
		}
		Ok(())
	}
}

struct Bits {
	buf: u32,
	cnt: u32,
}

fn bits(inp: &mut Input, b: &mut Bits, n: u32) -> Result<u32, Error> {
	while b.cnt < n {
		b.buf |= u32::from(inp.byte()?) << b.cnt;
		b.cnt += 8;
	}
	let v = b.buf & ((1u32 << n) - 1);
	b.buf >>= n;
	b.cnt -= n;
	Ok(v)
}

const fn bad(e: InflateError) -> Error {
	Error::Inflate(e)
}

/// A canonical Huffman table: how many codes of each length, and the symbols in code order.
struct Tree {
	counts: [u16; 16],
	syms: [u16; 288],
}

impl Tree {
	const fn new() -> Tree {
		Tree {
			counts: [0; 16],
			syms: [0; 288],
		}
	}

	fn build(&mut self, lens: &[u8]) -> Result<(), Error> {
		self.counts = [0; 16];
		for &l in lens {
			*self
				.counts
				.get_mut(usize::from(l))
				.ok_or(bad(InflateError::BadLengths))? += 1;
		}
		let mut left = 1i32;
		let mut offs = [0u16; 16];
		for l in 1..16 {
			left = (left << 1) - i32::from(self.counts[l]);
			if left < 0 {
				return Err(bad(InflateError::BadLengths));
			}
			if l < 15 {
				offs[l + 1] = offs[l] + self.counts[l];
			}
		}
		for (sym, &l) in lens.iter().enumerate() {
			if l != 0 {
				let o = &mut offs[usize::from(l)];
				self.syms[usize::from(*o)] = sym as u16;
				*o += 1;
			}
		}
		Ok(())
	}

	fn decode(&self, inp: &mut Input, b: &mut Bits) -> Result<u16, Error> {
		let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
		for len in 1..16 {
			code |= bits(inp, b, 1)? as i32;
			let count = i32::from(self.counts[len]);
			if code - count < first {
				return self
					.syms
					.get((index + code - first) as usize)
					.copied()
					.ok_or(bad(InflateError::BadCode));
			}
			index += count;
			first = (first + count) << 1;
			code <<= 1;
		}
		Err(bad(InflateError::BadCode))
	}
}

const LBASE: [u16; 29] = [
	3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
	163, 195, 227, 258,
];
const LEXT: [u8; 29] = [
	0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DBASE: [u16; 30] = [
	1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
	2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [
	0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
	13,
];
const ORDER: [u8; 19] = [
	16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

fn codes(
	inp: &mut Input,
	b: &mut Bits,
	lit: &Tree,
	dist: &Tree,
	out: &mut [u8],
	n: &mut usize,
) -> Result<(), Error> {
	loop {
		let sym = lit.decode(inp, b)?;
		if sym < 256 {
			*out.get_mut(*n).ok_or(bad(InflateError::OutputOverflow))? = sym as u8;
			*n += 1;
		} else if sym == 256 {
			return Ok(());
		} else {
			let s = usize::from(sym - 257);
			let (&base, &ext) = LBASE
				.get(s)
				.zip(LEXT.get(s))
				.ok_or(bad(InflateError::BadCode))?;
			let len = usize::from(base) + bits(inp, b, u32::from(ext))? as usize;
			let d = usize::from(dist.decode(inp, b)?);
			let (&base, &ext) = DBASE
				.get(d)
				.zip(DEXT.get(d))
				.ok_or(bad(InflateError::BadCode))?;
			let dist = usize::from(base) + bits(inp, b, u32::from(ext))? as usize;
			if dist > *n {
				return Err(bad(InflateError::BadDistance));
			}
			if *n + len > out.len() {
				return Err(bad(InflateError::OutputOverflow));
			}
			for _ in 0..len {
				out[*n] = out[*n - dist];
				*n += 1;
			}
		}
	}
}

fn dynamic(inp: &mut Input, b: &mut Bits, lit: &mut Tree, dist: &mut Tree) -> Result<(), Error> {
	let nlit = bits(inp, b, 5)? as usize + 257;
	let ndist = bits(inp, b, 5)? as usize + 1;
	let ncode = bits(inp, b, 4)? as usize + 4;
	if nlit > 286 || ndist > 30 {
		return Err(bad(InflateError::BadLengths));
	}
	let mut cl = [0u8; 19];
	for &o in ORDER.iter().take(ncode) {
		cl[usize::from(o)] = bits(inp, b, 3)? as u8;
	}
	lit.build(&cl)?;
	let mut lens = [0u8; 316];
	let total = nlit + ndist;
	let mut i = 0;
	while i < total {
		let sym = lit.decode(inp, b)?;
		let (val, rep) = match sym {
			0..=15 => (sym as u8, 1),
			16 => {
				if i == 0 {
					return Err(bad(InflateError::BadLengths));
				}
				(lens[i - 1], 3 + bits(inp, b, 2)? as usize)
			}
			17 => (0, 3 + bits(inp, b, 3)? as usize),
			_ => (0, 11 + bits(inp, b, 7)? as usize),
		};
		if i + rep > total {
			return Err(bad(InflateError::BadLengths));
		}
		lens[i..i + rep].fill(val);
		i += rep;
	}
	if lens[256] == 0 {
		return Err(bad(InflateError::BadLengths));
	}
	lit.build(&lens[..nlit])?;
	dist.build(&lens[nlit..total])
}

/// Decodes one raw deflate stream into `out` and returns how many bytes it wrote.
pub(crate) fn inflate(inp: &mut Input, out: &mut [u8]) -> Result<usize, Error> {
	let mut b = Bits { buf: 0, cnt: 0 };
	let mut n = 0usize;
	let mut lit = Tree::new();
	let mut dist = Tree::new();
	loop {
		let last = bits(inp, &mut b, 1)?;
		match bits(inp, &mut b, 2)? {
			0 => {
				b = Bits { buf: 0, cnt: 0 };
				let mut h = [0u8; 4];
				inp.copy_to(&mut h)?;
				let len = usize::from(u16::from_le_bytes([h[0], h[1]]));
				if h[2] != !h[0] || h[3] != !h[1] {
					return Err(bad(InflateError::BadStoredLength));
				}
				let dst = out
					.get_mut(n..n + len)
					.ok_or(bad(InflateError::OutputOverflow))?;
				inp.copy_to(dst)?;
				n += len;
			}
			1 => {
				let mut lens = [8u8; 288];
				lens[144..256].fill(9);
				lens[256..280].fill(7);
				lit.build(&lens)?;
				dist.build(&[5u8; 30])?;
				codes(inp, &mut b, &lit, &dist, out, &mut n)?;
			}
			2 => {
				dynamic(inp, &mut b, &mut lit, &mut dist)?;
				codes(inp, &mut b, &lit, &dist, out, &mut n)?;
			}
			_ => return Err(bad(InflateError::BadBlockType)),
		}
		if last == 1 {
			return Ok(n);
		}
	}
}

#[cfg(all(test, feature = "writer"))]
mod tests {
	use super::*;
	use crate::reader::SliceSource;
	use alloc::vec::Vec;

	fn run(stream: &[u8], out_len: usize) -> Result<Vec<u8>, Error> {
		let vols = [stream];
		let mut src = SliceSource::new(&vols);
		src.open_volume(1).unwrap();
		let mut buf = [0u8; INBUF];
		let mut inp = Input::new(&mut src, &mut buf, 0, stream.len() as u32, Crc::default());
		let mut out = alloc::vec![0u8; out_len];
		let n = inflate(&mut inp, &mut out)?;
		out.truncate(n);
		Ok(out)
	}

	fn sample(seed: u32, n: usize) -> Vec<u8> {
		let mut x = seed | 1;
		(0..n)
			.map(|i| {
				x ^= x << 13;
				x ^= x >> 17;
				x ^= x << 5;
				if i % 5 < 3 {
					(i / 7) as u8
				} else {
					(x >> 9) as u8 & 0x1f
				}
			})
			.collect()
	}

	#[test]
	fn decodes_what_deflate_makes_at_every_level() {
		for level in 0..=10u8 {
			for (seed, n) in [(1, 0), (2, 1), (3, 300), (4, 5000), (5, 32768)] {
				let data = sample(seed, n);
				let packed = miniz_oxide::deflate::compress_to_vec(&data, level);
				assert_eq!(run(&packed, n).unwrap(), data, "level {level} size {n}");
			}
		}
	}

	#[test]
	fn decodes_a_stored_block_and_a_fixed_block() {
		// A stored block of "abc" (final), then a fixed-Huffman block holding "a" (final).
		let stored = [0x01, 3, 0, 0xfc, 0xff, b'a', b'b', b'c'];
		assert_eq!(run(&stored, 3).unwrap(), b"abc");
		let fixed = [0x4b, 0x04, 0x00];
		assert_eq!(run(&fixed, 1).unwrap(), b"a");
	}

	#[test]
	fn refuses_a_stream_that_overruns_or_underruns_the_output() {
		let data = sample(9, 4000);
		let packed = miniz_oxide::deflate::compress_to_vec(&data, 6);
		assert_eq!(
			run(&packed, 3999).unwrap_err(),
			Error::Inflate(InflateError::OutputOverflow)
		);
		// A longer window accepts it but the reader then compares the length; here the count shows.
		assert_eq!(run(&packed, 4001).unwrap().len(), 4000);
	}

	#[test]
	fn refuses_bad_streams_with_a_typed_error() {
		assert_eq!(
			run(&[0x07], 10).unwrap_err(),
			Error::Inflate(InflateError::BadBlockType)
		);
		assert_eq!(
			run(&[0x01, 3, 0, 0, 0], 10).unwrap_err(),
			Error::Inflate(InflateError::BadStoredLength)
		);
		assert_eq!(
			run(&[], 10).unwrap_err(),
			Error::Inflate(InflateError::InputEnd)
		);
		// A back-reference before the start: fixed block, length 3 distance 1 with nothing yet.
		assert_eq!(
			run(&[0x03, 0x02, 0x00], 10).unwrap_err(),
			Error::Inflate(InflateError::BadDistance)
		);
	}

	#[test]
	fn every_prefix_and_every_bit_flip_of_a_real_stream_is_handled() {
		let data = sample(11, 3000);
		let packed = miniz_oxide::deflate::compress_to_vec(&data, 9);
		for n in 0..packed.len() {
			assert!(run(&packed[..n], 3000).is_err(), "prefix {n}");
		}
		for i in 0..packed.len() {
			for bit in 0..8 {
				let mut d = packed.clone();
				d[i] ^= 1 << bit;
				// Either an error, or a different but bounded output; never a panic.
				let _ = run(&d, 3000);
			}
		}
	}

	#[test]
	fn random_streams_never_panic_and_agree_with_a_reference_when_they_decode() {
		let mut x = 0x1234_5678u32;
		let mut agree = 0;
		for _ in 0..8_000 {
			x ^= x << 13;
			x ^= x >> 17;
			x ^= x << 5;
			let n = (x % 64) as usize + 1;
			let s: Vec<u8> = (0..n)
				.map(|_| {
					x ^= x << 13;
					x ^= x >> 17;
					x ^= x << 5;
					(x >> 7) as u8
				})
				.collect();
			if let Ok(mine) = run(&s, 70_000)
				&& let Ok(theirs) = miniz_oxide::inflate::decompress_to_vec(&s)
			{
				assert_eq!(mine, theirs);
				agree += 1;
			}
		}
		assert!(agree > 0);
	}
}
