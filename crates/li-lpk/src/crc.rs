// CRC-32 (the zip and PNG polynomial) with a 16-entry table, so the reader stays small.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const TABLE: [u32; 16] = [
	0x0000_0000,
	0x1db7_1064,
	0x3b6e_20c8,
	0x26d9_30ac,
	0x76dc_4190,
	0x6b6b_51f4,
	0x4db2_6158,
	0x5005_713c,
	0xedb8_8320,
	0xf00f_9344,
	0xd6d6_a3e8,
	0xcb61_b38c,
	0x9b64_c2b0,
	0x86d3_d2d4,
	0xa00a_e278,
	0xbdbd_f21c,
];

/// A running CRC-32.
#[derive(Debug, Clone, Copy)]
pub struct Crc(u32);

impl Default for Crc {
	fn default() -> Self {
		Crc(!0)
	}
}

impl Crc {
	/// Adds `data`.
	pub fn update(&mut self, data: &[u8]) {
		let mut c = self.0;
		for &b in data {
			c ^= u32::from(b);
			c = TABLE[(c & 15) as usize] ^ (c >> 4);
			c = TABLE[(c & 15) as usize] ^ (c >> 4);
		}
		self.0 = c;
	}

	/// The checksum of everything added so far.
	pub fn value(&self) -> u32 {
		!self.0
	}
}

/// The CRC-32 of `data`.
pub fn crc32(data: &[u8]) -> u32 {
	let mut c = Crc::default();
	c.update(data);
	c.value()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn matches_the_check_value() {
		assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
		assert_eq!(crc32(b""), 0);
	}

	#[test]
	fn pieces_equal_the_whole() {
		let mut c = Crc::default();
		c.update(b"1234");
		c.update(b"56789");
		assert_eq!(c.value(), 0xcbf4_3926);
	}
}
