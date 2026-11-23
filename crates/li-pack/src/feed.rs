// Feeds with a detached signature, checked against a pinned key.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use li_model::{Artefact, Feed};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::{PackError, SigningKey, VerifyingKey, sha256_hex, sign, verify};

pub const FEED_FILE: &str = "feed.json";
pub const SIG_FILE: &str = "feed.json.sig";

/// Serializes and signs a feed. Returns the exact JSON bytes and the hex signature over them.
pub fn sign_feed(feed: &Feed, key: &SigningKey) -> Result<(Vec<u8>, String), PackError> {
	let mut json = serde_json::to_vec_pretty(feed)?;
	json.push(b'\n');
	let sig = sign(key, &json);
	Ok((json, sig))
}

/// Writes `feed.json` and `feed.json.sig` into `dir`.
pub fn write_feed(dir: &Path, feed: &Feed, key: &SigningKey) -> Result<(), PackError> {
	let (json, sig) = sign_feed(feed, key)?;
	std::fs::write(dir.join(FEED_FILE), json)?;
	std::fs::write(dir.join(SIG_FILE), format!("{sig}\n"))?;
	Ok(())
}

fn parse_time(s: &str) -> Result<OffsetDateTime, PackError> {
	OffsetDateTime::parse(s, &Rfc3339).map_err(|_| PackError::BadTimestamp(s.to_owned()))
}

/// Verifies the signature before reading anything, then refuses a feed whose `version` or
/// `timestamp` is older than `previous`'s. Equal values are accepted.
pub fn verify_feed(
	json: &[u8],
	signature_hex: &str,
	key: &VerifyingKey,
	previous: Option<&Feed>,
) -> Result<Feed, PackError> {
	verify(key, json, signature_hex)?;
	let feed: Feed = serde_json::from_slice(json)?;
	if let Some(prev) = previous {
		if feed.version < prev.version {
			return Err(PackError::FeedRollback(format!(
				"version {} is older than {}",
				feed.version, prev.version
			)));
		}
		if let Some(prev_ts) = &prev.timestamp {
			let Some(ts) = &feed.timestamp else {
				return Err(PackError::FeedRollback(
					"timestamp is missing but the previous feed had one".into(),
				));
			};
			if parse_time(ts)? < parse_time(prev_ts)? {
				return Err(PackError::FeedRollback(format!(
					"timestamp {ts} is older than {prev_ts}"
				)));
			}
		}
	}
	Ok(feed)
}

/// Reads and verifies `feed.json` and `feed.json.sig` from `dir`.
pub fn read_feed(
	dir: &Path,
	key: &VerifyingKey,
	previous: Option<&Feed>,
) -> Result<Feed, PackError> {
	let json = std::fs::read(dir.join(FEED_FILE))?;
	let sig = std::fs::read_to_string(dir.join(SIG_FILE))?;
	verify_feed(&json, &sig, key, previous)
}

/// Checks downloaded bytes against a feed artefact's size and SHA-256.
pub fn verify_artefact(bytes: &[u8], artefact: &Artefact) -> Result<(), PackError> {
	if bytes.len() as u64 != artefact.size {
		return Err(PackError::SizeMismatch {
			what: artefact.url.clone(),
			expected: artefact.size,
			actual: bytes.len() as u64,
		});
	}
	let actual = sha256_hex(bytes);
	if !actual.eq_ignore_ascii_case(&artefact.sha256) {
		return Err(PackError::HashMismatch {
			what: artefact.url.clone(),
			expected: artefact.sha256.clone(),
			actual,
		});
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::generate_keypair;

	fn feed(version: u32, ts: Option<&str>) -> Feed {
		Feed {
			version,
			publisher: "Liminal HQ".into(),
			timestamp: ts.map(Into::into),
			products: vec![],
		}
	}

	#[test]
	fn signed_feed_round_trips_on_disk() {
		let dir = tempfile::tempdir().unwrap();
		let (sk, vk) = generate_keypair().unwrap();
		let f = feed(1, Some("2026-11-20T10:00:00Z"));
		write_feed(dir.path(), &f, &sk).unwrap();
		assert_eq!(read_feed(dir.path(), &vk, None).unwrap(), f);
	}

	#[test]
	fn tampering_and_wrong_keys_are_refused() {
		let (sk, vk) = generate_keypair().unwrap();
		let (_, other) = generate_keypair().unwrap();
		let (json, sig) = sign_feed(&feed(1, None), &sk).unwrap();
		assert!(matches!(
			verify_feed(&json, &sig, &other, None),
			Err(PackError::BadSignature)
		));
		let mut bad = json.clone();
		bad[5] ^= 1;
		assert!(verify_feed(&bad, &sig, &vk, None).is_err());
	}

	#[test]
	fn rollbacks_are_refused() {
		let (sk, vk) = generate_keypair().unwrap();
		let prev = feed(2, Some("2026-11-20T10:00:00Z"));
		let check = |f: Feed| {
			let (json, sig) = sign_feed(&f, &sk).unwrap();
			verify_feed(&json, &sig, &vk, Some(&prev))
		};
		assert!(matches!(
			check(feed(1, Some("2026-11-21T10:00:00Z"))),
			Err(PackError::FeedRollback(_))
		));
		assert!(matches!(
			check(feed(2, Some("2026-11-19T10:00:00Z"))),
			Err(PackError::FeedRollback(_))
		));
		assert!(matches!(
			check(feed(2, None)),
			Err(PackError::FeedRollback(_))
		));
		assert!(check(feed(2, Some("2026-11-20T10:00:00Z"))).is_ok());
		assert!(check(feed(3, Some("2026-11-21T05:00:00+00:00"))).is_ok());
	}

	#[test]
	fn artefacts_are_checked() {
		let a = Artefact {
			target: "t".into(),
			kind: "payload".into(),
			url: "u".into(),
			size: 3,
			sha256: sha256_hex(b"abc"),
		};
		verify_artefact(b"abc", &a).unwrap();
		assert!(verify_artefact(b"abd", &a).is_err());
		assert!(verify_artefact(b"ab", &a).is_err());
	}
}
