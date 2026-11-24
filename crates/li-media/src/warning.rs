// What can be wrong with a medium, as a code the UI can link to a tab.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

/// How serious a warning is. Only an error stops a build.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaSeverity {
	Warning,
	Error,
}

/// What a [`MediaWarning`] is about. The Studio picks the tab that fixes it from this code.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaWarningCode {
	/// The payload is larger than the disk; the largest files are named.
	DoesNotFit,
	/// Less than `reserve` bytes would be free.
	BelowReserve,
	/// A component that is ticked by default lands on a later disk.
	DefaultOnLaterDisk,
	/// `COMMON.LPK` uses 4 KiB chunks because Windows 3.x reads it.
	Win16Window,
	/// The label breaks the label rule.
	Label,
	/// A target's folder is not an 8.3 name, or two targets use one folder.
	#[serde(rename = "dir_not_8_3")]
	DirNot83,
	/// `autorun` names a target that is not on the medium, or is set on a floppy.
	AutorunTarget,
	/// No `SETUP.EXE` was found for a target, so its budgeted size is used.
	RuntimeEstimated,
	/// A runtime is older than a feature the medium needs.
	RuntimeLacks,
	/// A destination path does not suit DOS or Windows 3.x.
	#[serde(rename = "name_83")]
	Name83,
}

impl MediaWarningCode {
	/// Every code, in the order the Studio's table lists them.
	pub const ALL: [MediaWarningCode; 10] = [
		MediaWarningCode::DoesNotFit,
		MediaWarningCode::BelowReserve,
		MediaWarningCode::DefaultOnLaterDisk,
		MediaWarningCode::Win16Window,
		MediaWarningCode::Label,
		MediaWarningCode::DirNot83,
		MediaWarningCode::AutorunTarget,
		MediaWarningCode::RuntimeEstimated,
		MediaWarningCode::RuntimeLacks,
		MediaWarningCode::Name83,
	];
}

/// One thing wrong with, or worth knowing about, a medium. `message` is for a person and passes
/// through the Studio unchanged; `code` is for code.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaWarning {
	pub code: MediaWarningCode,
	pub severity: MediaSeverity,
	/// What it is about: a target, a component, a path or the label.
	pub subject: String,
	pub message: String,
}

impl MediaWarning {
	pub fn new(
		code: MediaWarningCode,
		severity: MediaSeverity,
		subject: impl Into<String>,
		message: impl Into<String>,
	) -> Self {
		Self {
			code,
			severity,
			subject: subject.into(),
			message: message.into(),
		}
	}

	pub fn error(
		code: MediaWarningCode,
		subject: impl Into<String>,
		message: impl Into<String>,
	) -> Self {
		Self::new(code, MediaSeverity::Error, subject, message)
	}

	pub fn warning(
		code: MediaWarningCode,
		subject: impl Into<String>,
		message: impl Into<String>,
	) -> Self {
		Self::new(code, MediaSeverity::Warning, subject, message)
	}
}
