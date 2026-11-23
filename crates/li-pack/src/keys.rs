// Ed25519 keys, key files, signing and verification.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use ed25519_dalek::{Signature, Signer};
pub use ed25519_dalek::{SigningKey, VerifyingKey};

use crate::PackError;

/// Generates a fresh key pair from the operating system's random source.
pub fn generate_keypair() -> Result<(SigningKey, VerifyingKey), PackError> {
	let mut seed = [0u8; 32];
	getrandom::fill(&mut seed).map_err(|e| PackError::BadKey(e.to_string()))?;
	let signing = SigningKey::from_bytes(&seed);
	let verifying = signing.verifying_key();
	Ok((signing, verifying))
}

fn read_hex32(path: &Path) -> Result<[u8; 32], PackError> {
	let text = std::fs::read_to_string(path)?;
	let bytes = hex::decode(text.trim()).map_err(|e| PackError::BadKey(e.to_string()))?;
	bytes
		.try_into()
		.map_err(|_| PackError::BadKey("expected 32 bytes of hex".into()))
}

/// Writes the secret key as one line of hex. On Unix the file is readable only by its owner.
pub fn write_signing_key(path: &Path, key: &SigningKey) -> Result<(), PackError> {
	std::fs::write(path, format!("{}\n", hex::encode(key.to_bytes())))?;
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
	}
	Ok(())
}

pub fn read_signing_key(path: &Path) -> Result<SigningKey, PackError> {
	Ok(SigningKey::from_bytes(&read_hex32(path)?))
}

/// Writes the public key as one line of hex.
pub fn write_public_key(path: &Path, key: &VerifyingKey) -> Result<(), PackError> {
	std::fs::write(path, format!("{}\n", hex::encode(key.to_bytes())))?;
	Ok(())
}

pub fn read_public_key(path: &Path) -> Result<VerifyingKey, PackError> {
	VerifyingKey::from_bytes(&read_hex32(path)?).map_err(|e| PackError::BadKey(e.to_string()))
}

/// The public key as lowercase hex, the form key files and bundles use.
pub fn public_key_hex(key: &VerifyingKey) -> String {
	hex::encode(key.to_bytes())
}

/// Reads a public key from hex text such as a key file's contents.
pub fn parse_public_key(text: &str) -> Result<VerifyingKey, PackError> {
	let bytes = hex::decode(text.trim()).map_err(|e| PackError::BadKey(e.to_string()))?;
	let bytes: [u8; 32] = bytes
		.try_into()
		.map_err(|_| PackError::BadKey("expected 32 bytes of hex".into()))?;
	VerifyingKey::from_bytes(&bytes).map_err(|e| PackError::BadKey(e.to_string()))
}

/// Signs `data`; the signature is returned as hex.
pub fn sign(key: &SigningKey, data: &[u8]) -> String {
	hex::encode(key.sign(data).to_bytes())
}

/// Verifies a hex signature over `data`.
pub fn verify(key: &VerifyingKey, data: &[u8], signature_hex: &str) -> Result<(), PackError> {
	let bytes = hex::decode(signature_hex.trim()).map_err(|_| PackError::BadSignature)?;
	let bytes: [u8; 64] = bytes.try_into().map_err(|_| PackError::BadSignature)?;
	key.verify_strict(data, &Signature::from_bytes(&bytes))
		.map_err(|_| PackError::BadSignature)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sign_and_verify() {
		let (sk, vk) = generate_keypair().unwrap();
		let sig = sign(&sk, b"data");
		verify(&vk, b"data", &sig).unwrap();
		assert!(verify(&vk, b"other", &sig).is_err());
		assert!(verify(&vk, b"data", "zz").is_err());
	}

	#[test]
	fn public_keys_round_trip_as_hex() {
		let (_, vk) = generate_keypair().unwrap();
		assert_eq!(parse_public_key(&public_key_hex(&vk)).unwrap(), vk);
		assert!(parse_public_key("abcd").is_err());
	}

	#[test]
	fn key_files_round_trip() {
		let dir = tempfile::tempdir().unwrap();
		let (sk, vk) = generate_keypair().unwrap();
		write_signing_key(&dir.path().join("k"), &sk).unwrap();
		write_public_key(&dir.path().join("k.pub"), &vk).unwrap();
		assert_eq!(read_signing_key(&dir.path().join("k")).unwrap(), sk);
		assert_eq!(read_public_key(&dir.path().join("k.pub")).unwrap(), vk);
	}
}
