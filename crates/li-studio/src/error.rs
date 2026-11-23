// The error type shared by the Studio core, the Tauri commands and the MCP tools.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize, Serializer};
use thiserror::Error;
use ts_rs::TS;

use crate::backend::BackendError;

/// What went wrong, in terms a caller can branch on. The wire name is the snake_case form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
	/// A document, target, tool or file the request names does not exist.
	NotFound,
	/// An argument or document the request carries is not acceptable.
	InvalidInput,
	/// Reading or writing a file failed.
	Io,
	/// The project is not in a state that allows the request: none is open, or there is nothing to undo.
	Project,
	/// A script could not be tested.
	Script,
	/// The engine behind the Studio failed.
	Backend,
	/// The agent (MCP) server could not do what was asked.
	Agent,
	/// The shell failed in a way no input explains, such as a window that would not open.
	Internal,
}

/// What a failed command or MCP call sends: the kind to branch on and a message to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ErrorPayload {
	pub kind: ErrorKind,
	pub message: String,
}

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
	/// A message shown as it is, with the kind the caller chose.
	#[error("{message}")]
	Plain { kind: ErrorKind, message: String },
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

	/// An error of `kind` whose message is shown exactly as given.
	pub fn new(kind: ErrorKind, message: impl ToString) -> Self {
		StudioError::Plain {
			kind,
			message: message.to_string(),
		}
	}

	pub fn kind(&self) -> ErrorKind {
		match self {
			StudioError::NoProject | StudioError::NothingTo(_) => ErrorKind::Project,
			StudioError::NotFound(_) => ErrorKind::NotFound,
			StudioError::Invalid { .. } => ErrorKind::InvalidInput,
			StudioError::Io { .. } => ErrorKind::Io,
			StudioError::Backend(_) => ErrorKind::Backend,
			StudioError::Plain { kind, .. } => *kind,
		}
	}

	pub fn payload(&self) -> ErrorPayload {
		ErrorPayload {
			kind: self.kind(),
			message: self.to_string(),
		}
	}
}

/// Serialises as `{ "kind": "not_found", "message": "flow `x` not found" }`, which is what a Tauri
/// command rejects with and what the UI parses.
impl Serialize for StudioError {
	fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
		self.payload().serialize(s)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn it_serialises_as_a_kind_and_a_message() {
		let e = StudioError::NotFound("flow `x`".into());
		assert_eq!(
			serde_json::to_value(&e).unwrap(),
			json!({"kind": "not_found", "message": "flow `x` not found"})
		);
	}

	#[test]
	fn every_variant_maps_to_its_kind_and_the_wire_name() {
		let cases = [
			(StudioError::NoProject, ErrorKind::Project, "project"),
			(
				StudioError::NothingTo("undo"),
				ErrorKind::Project,
				"project",
			),
			(
				StudioError::NotFound("a".into()),
				ErrorKind::NotFound,
				"not_found",
			),
			(
				StudioError::invalid("a", "b"),
				ErrorKind::InvalidInput,
				"invalid_input",
			),
			(StudioError::io("/x", "denied"), ErrorKind::Io, "io"),
			(
				BackendError("e".into()).into(),
				ErrorKind::Backend,
				"backend",
			),
			(
				StudioError::new(ErrorKind::Script, "s"),
				ErrorKind::Script,
				"script",
			),
			(
				StudioError::new(ErrorKind::Agent, "s"),
				ErrorKind::Agent,
				"agent",
			),
			(
				StudioError::new(ErrorKind::Internal, "s"),
				ErrorKind::Internal,
				"internal",
			),
		];
		for (error, kind, wire) in cases {
			assert_eq!(error.kind(), kind);
			assert_eq!(serde_json::to_value(&error).unwrap()["kind"], wire);
		}
	}

	#[test]
	fn a_plain_message_is_not_wrapped() {
		let e = StudioError::new(ErrorKind::InvalidInput, "the port is taken");
		assert_eq!(e.to_string(), "the port is taken");
		assert_eq!(e.payload().message, "the port is taken");
	}
}
