// Small XML and shell helpers shared by the generators.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use sha2::{Digest, Sha256};

pub(crate) fn esc(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
}

/// Single-quotes `s` for a POSIX shell.
pub(crate) fn sh_quote(s: &str) -> String {
	format!("'{}'", s.replace('\'', "'\\''"))
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
	let digest = Sha256::digest(bytes);
	digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// A stable GUID derived from `seed`, so rebuilds keep the same upgrade code.
pub(crate) fn guid(seed: &str) -> String {
	let d = Sha256::digest(seed.as_bytes());
	let mut b = [0u8; 16];
	b.copy_from_slice(&d[..16]);
	b[6] = (b[6] & 0x0f) | 0x50;
	b[8] = (b[8] & 0x3f) | 0x80;
	let h: String = b.iter().map(|x| format!("{x:02X}")).collect();
	format!(
		"{}-{}-{}-{}-{}",
		&h[0..8],
		&h[8..12],
		&h[12..16],
		&h[16..20],
		&h[20..32]
	)
}
