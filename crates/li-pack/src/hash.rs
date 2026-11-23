// SHA-256 helpers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

/// The SHA-256 of `data` as lowercase hex.
pub fn sha256_hex(data: &[u8]) -> String {
	hex::encode(Sha256::digest(data))
}

/// The SHA-256 of a file's contents, streamed.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
	let mut file = std::fs::File::open(path)?;
	let mut hasher = Sha256::new();
	let mut buf = vec![0u8; 64 * 1024];
	loop {
		let n = file.read(&mut buf)?;
		if n == 0 {
			break;
		}
		hasher.update(&buf[..n]);
	}
	Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn known_vector() {
		assert_eq!(
			sha256_hex(b"abc"),
			"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
		);
	}

	#[test]
	fn file_matches_bytes() {
		let dir = tempfile::tempdir().unwrap();
		let p = dir.path().join("f");
		std::fs::write(&p, b"hello").unwrap();
		assert_eq!(sha256_file(&p).unwrap(), sha256_hex(b"hello"));
	}
}
