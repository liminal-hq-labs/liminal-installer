// The `[[media]]` entry and the edit the Studio sends to change one.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

/// What the medium is.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
	/// A 3½-inch 1.44 MB floppy disk (512-byte clusters, 1,457,664 bytes of data).
	#[serde(rename = "floppy-1440")]
	Floppy1440,
	/// A CD-ROM (2048-byte sectors, 650 MB).
	#[serde(rename = "cdrom")]
	Cdrom,
}

/// Whether files that several targets share are stored once, in `COMMON.LPK`.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommonMode {
	Auto,
	None,
}

/// What happens when the payload is larger than the disk.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Overflow {
	/// The build fails with `does_not_fit`.
	Error,
	/// The set continues on a second disk.
	Span,
}

/// One target on a medium and the folder its files go in.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaTarget {
	/// The target, as `os-arch` (`dos-x86`).
	pub target: String,
	/// An 8.3 folder, or empty for the root of the medium.
	pub dir: String,
}

/// One `[[media]]` entry of `installer.toml`.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaInfo {
	pub id: String,
	pub kind: MediaKind,
	/// The volume label.
	pub label: String,
	/// `auto` (worked out from the version) or eight hexadecimal digits.
	pub serial: String,
	pub common: CommonMode,
	/// Components left off this medium.
	pub exclude: Vec<String>,
	pub overflow: Overflow,
	/// Bytes that must stay free; a build that leaves less fails.
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub reserve: u64,
	/// A file used as `README.TXT`; the generated text is used when it is absent.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub readme: Option<String>,
	/// Loose files copied to the root.
	#[serde(default)]
	pub extra: Vec<String>,
	/// The target `AUTORUN.INF` opens (a CD only).
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub autorun: Option<String>,
	pub targets: Vec<MediaTarget>,
}

/// A change to one medium. A field left out is not touched.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaEdit {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub label: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub serial: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub common: Option<CommonMode>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub exclude: Option<Vec<String>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub overflow: Option<Overflow>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional, as = "Option<u32>"))]
	pub reserve: Option<u64>,
	/// A file for `README.TXT`; an empty string goes back to the generated text.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub readme: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub extra: Option<Vec<String>>,
	/// The target `AUTORUN.INF` opens; an empty string clears it.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub autorun: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub targets: Option<Vec<MediaTarget>>,
}

impl MediaEdit {
	/// True when the edit changes nothing.
	pub fn is_empty(&self) -> bool {
		*self == MediaEdit::default()
	}

	/// The entry with this edit applied. Nothing is checked; run [`crate::validate`] on the result.
	pub fn apply(&self, info: &MediaInfo) -> MediaInfo {
		let mut out = info.clone();
		if let Some(v) = &self.label {
			out.label = v.clone();
		}
		if let Some(v) = &self.serial {
			out.serial = v.clone();
		}
		if let Some(v) = self.common {
			out.common = v;
		}
		if let Some(v) = &self.exclude {
			out.exclude = v.clone();
		}
		if let Some(v) = self.overflow {
			out.overflow = v;
		}
		if let Some(v) = self.reserve {
			out.reserve = v;
		}
		if let Some(v) = &self.readme {
			out.readme = (!v.is_empty()).then(|| v.clone());
		}
		if let Some(v) = &self.extra {
			out.extra = v.clone();
		}
		if let Some(v) = &self.autorun {
			out.autorun = (!v.is_empty()).then(|| v.clone());
		}
		if let Some(v) = &self.targets {
			out.targets = v.clone();
		}
		out
	}
}
