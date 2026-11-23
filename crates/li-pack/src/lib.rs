// Hashing, signing, payload containers and signed feeds.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Packaging primitives for Liminal Installer.
//!
//! - [`hash`]: SHA-256 as lowercase hex.
//! - [`keys`]: Ed25519 key generation, key files, signing and verification.
//! - [`lip`]: the `.lip` payload container.
//! - [`feed`]: feeds with a detached `feed.json.sig` signature.
//!
//! # The `.lip` format
//!
//! A `.lip` file is a flat, deterministic container: the same entries always give the same bytes.
//!
//! ```text
//! "LIP1"                 4 bytes, magic and format version
//! manifest_len           4 bytes, little-endian u32
//! manifest               manifest_len bytes of JSON
//! data                   every entry's bytes, back to back, in manifest order
//! ```
//!
//! The manifest is `{"version":1,"entries":[{"path":"…","size":N,"sha256":"…","executable":false}]}`
//! with entries sorted by path. Paths use `/`, are relative, and contain no `..` or empty segments.
//! Readers check every entry's size and SHA-256 when they open a file and refuse it on any mismatch.

pub mod error;
pub mod feed;
pub mod hash;
pub mod keys;
pub mod lip;

pub use error::PackError;
pub use feed::{
	FEED_FILE, SIG_FILE, read_feed, sign_feed, verify_artefact, verify_feed, write_feed,
};
pub use hash::{sha256_file, sha256_hex};
pub use keys::{
	SigningKey, VerifyingKey, generate_keypair, read_public_key, read_signing_key, sign, verify,
	write_public_key, write_signing_key,
};
pub use lip::{Lip, LipBuilder, LipEntry};
