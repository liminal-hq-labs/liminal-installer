// Errors for the engine.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
	#[error("{path}: {source}")]
	Io {
		path: PathBuf,
		source: std::io::Error,
	},
	#[error("{path}: {message}")]
	Parse { path: PathBuf, message: String },
	#[error("unknown placeholder `{0}`")]
	Placeholder(String),
	#[error("unknown target `{0}`")]
	UnknownTarget(String),
	#[error("unknown component `{0}`")]
	UnknownComponent(String),
	#[error("component `{0}` does not apply to target `{1}`")]
	NotApplicable(String, String),
	#[error("required component `{0}` was not selected")]
	RequiredComponent(String),
	#[error("payload has no file `{0}`")]
	MissingSource(String),
	#[error("sha256 mismatch for `{what}`: expected {expected}, found {actual}")]
	HashMismatch {
		what: String,
		expected: String,
		actual: String,
	},
	#[error("not enough free space: need {need} bytes, have {have}")]
	InsufficientSpace { need: u64, have: u64 },
	#[error("no receipt at {0}; the product is not installed there")]
	NotInstalled(PathBuf),
	#[error("already up to date at version {0}")]
	UpToDate(String),
	#[error("feed has no usable release: {0}")]
	Feed(String),
	#[error("script error: {0}")]
	Script(#[from] li_script::ScriptError),
	#[error("package error: {0}")]
	Pack(#[from] li_pack::PackError),
	#[error("condition error: {0}")]
	Condition(#[from] crate::cond::CondError),
	#[error("flow error: {0}")]
	Flow(String),
	#[error("cancelled; every change was rolled back")]
	Cancelled,
	#[error("failed and rolled back every change: {0}")]
	RolledBack(Box<EngineError>),
	#[error("failed, and rolling back also failed: {cause}; {}", .rollback.join("; "))]
	RollbackFailed {
		cause: Box<EngineError>,
		rollback: Vec<String>,
	},
	#[error("{0}")]
	Other(String),
}

impl EngineError {
	pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
		EngineError::Io {
			path: path.into(),
			source,
		}
	}
}

pub type Result<T> = std::result::Result<T, EngineError>;
