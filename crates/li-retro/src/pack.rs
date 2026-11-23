// CRC-32 and the streaming decompressor for the payload.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use miniz_oxide::inflate::stream::{InflateState, inflate};
use miniz_oxide::{DataFormat, MZError, MZFlush, MZStatus};

use crate::Error;

const fn table() -> [u32; 256] {
	let mut t = [0u32; 256];
	let mut i = 0;
	while i < 256 {
		let mut c = i as u32;
		let mut k = 0;
		while k < 8 {
			c = if c & 1 != 0 {
				0xedb8_8320 ^ (c >> 1)
			} else {
				c >> 1
			};
			k += 1;
		}
		t[i] = c;
		i += 1;
	}
	t
}

static TABLE: [u32; 256] = table();

/// A running CRC-32 (the zip and PNG one).
#[derive(Debug, Clone, Copy)]
pub struct Crc(u32);

impl Default for Crc {
	fn default() -> Self {
		Crc(!0)
	}
}

impl Crc {
	pub fn update(&mut self, data: &[u8]) {
		for &b in data {
			self.0 = TABLE[usize::from((self.0 as u8) ^ b)] ^ (self.0 >> 8);
		}
	}

	pub fn finish(self) -> u32 {
		!self.0
	}
}

pub fn crc32(data: &[u8]) -> u32 {
	let mut c = Crc::default();
	c.update(data);
	c.finish()
}

/// Compresses to a raw deflate stream (the host side).
pub fn deflate(data: &[u8]) -> Vec<u8> {
	miniz_oxide::deflate::compress_to_vec(data, 9)
}

/// A raw deflate decoder fed in pieces. Memory use is the decoder's 32 KiB window and one 8 KiB
/// output buffer, whatever the size of the file.
pub struct Inflater {
	state: alloc::boxed::Box<InflateState>,
	out: Vec<u8>,
	done: bool,
}

impl Default for Inflater {
	fn default() -> Self {
		Inflater {
			state: InflateState::new_boxed(DataFormat::Raw),
			out: vec![0u8; 8192],
			done: false,
		}
	}
}

impl Inflater {
	/// Whether the end of the stream has been reached.
	pub fn finished(&self) -> bool {
		self.done
	}

	/// Decodes `input`, handing the output to `sink` in pieces of at most 8 KiB.
	pub fn feed(
		&mut self,
		mut input: &[u8],
		sink: &mut dyn FnMut(&[u8]) -> Result<(), Error>,
	) -> Result<(), Error> {
		let corrupt = || Error::Corrupt("the compressed data is damaged".to_string());
		loop {
			if self.done {
				return Ok(());
			}
			let r = inflate(&mut self.state, input, &mut self.out, MZFlush::None);
			input = &input[r.bytes_consumed..];
			if r.bytes_written > 0 {
				sink(&self.out[..r.bytes_written])?;
			}
			match r.status {
				Ok(MZStatus::StreamEnd) => {
					self.done = true;
					return Ok(());
				}
				Ok(_) | Err(MZError::Buf) => {}
				Err(_) => return Err(corrupt()),
			}
			if r.bytes_written == 0 {
				if input.is_empty() {
					return Ok(());
				}
				if r.bytes_consumed == 0 {
					return Err(corrupt());
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn crc_matches_the_check_value() {
		assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
	}

	fn inflate_all(packed: &[u8], piece: usize) -> Result<Vec<u8>, Error> {
		let mut inf = Inflater::default();
		let mut out = Vec::new();
		for chunk in packed.chunks(piece) {
			inf.feed(chunk, &mut |c| {
				out.extend_from_slice(c);
				Ok(())
			})?;
		}
		if inf.finished() {
			Ok(out)
		} else {
			Err(Error::Corrupt("cut short".into()))
		}
	}

	#[test]
	fn inflates_in_small_pieces() {
		let data: Vec<u8> = (0..50_000u32)
			.map(|i| (i % 251) as u8 ^ (i / 7) as u8)
			.collect();
		let packed = deflate(&data);
		assert_eq!(inflate_all(&packed, 100).unwrap(), data);
		assert_eq!(inflate_all(&packed, 1).unwrap(), data);
		assert_eq!(inflate_all(&packed, 1 << 20).unwrap(), data);
	}

	#[test]
	fn damaged_data_is_an_error() {
		let mut packed = deflate(&[7u8; 4000]);
		let n = packed.len();
		packed.truncate(n / 2);
		assert!(inflate_all(&packed, 64).is_err());
		let mut junk = deflate(b"hello hello hello hello");
		junk[0] = 0xff;
		assert!(inflate_all(&junk, 64).is_err());
	}
}
