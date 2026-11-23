// Errors for the packaging crate.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PackError {
	#[error("i/o error: {0}")]
	Io(#[from] std::io::Error),
	#[error("json error: {0}")]
	Json(#[from] serde_json::Error),
	#[error("bad key: {0}")]
	BadKey(String),
	#[error("signature does not verify")]
	BadSignature,
	#[error("sha256 mismatch for `{what}`: expected {expected}, found {actual}")]
	HashMismatch {
		what: String,
		expected: String,
		actual: String,
	},
	#[error("size mismatch for `{what}`: expected {expected}, found {actual}")]
	SizeMismatch {
		what: String,
		expected: u64,
		actual: u64,
	},
	#[error("bad .lip file: {0}")]
	BadFormat(String),
	#[error("bad path `{0}`")]
	BadPath(String),
	#[error("feed refused: {0}")]
	FeedRollback(String),
	#[error("bad timestamp `{0}`")]
	BadTimestamp(String),
}
