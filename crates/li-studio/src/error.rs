// The error type shared by the Studio core, the Tauri commands and the MCP tools.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Serialize, Serializer};
use thiserror::Error;

use crate::backend::BackendError;

#[derive(Debug, Error)]
pub enum StudioError {
	#[error("no project is open")]
	NoProject,
	#[error("{0} not found")]
	NotFound(String),
	#[error("invalid {what}: {why}")]
	Invalid { what: String, why: String },
	#[error("nothing to {0}")]
	NothingTo(&'static str),
	#[error("{path}: {message}")]
	Io { path: String, message: String },
	#[error(transparent)]
	Backend(#[from] BackendError),
}

impl StudioError {
	pub fn invalid(what: impl Into<String>, why: impl ToString) -> Self {
		StudioError::Invalid {
			what: what.into(),
			why: why.to_string(),
		}
	}

	pub fn io(path: impl AsRef<std::path::Path>, e: impl ToString) -> Self {
		StudioError::Io {
			path: path.as_ref().display().to_string(),
			message: e.to_string(),
		}
	}
}

impl Serialize for StudioError {
	fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
		s.serialize_str(&self.to_string())
	}
}
