// The plan of a medium: what is on each disk, how big it is and what a change would do.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

use crate::types::MediaKind;
use crate::warning::MediaWarning;

/// Which pack a file is stored in.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackWhich {
	/// The target's own `SETUP.LPK`.
	Own,
	/// `COMMON.LPK`, shared by several targets.
	Common,
}

/// What a row of the disk map is.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryRole {
	Folder,
	/// A target's `SETUP.EXE`.
	Setup,
	/// A target's `SETUP.LPK`.
	OwnPack,
	/// `COMMON.LPK` or one of its later volumes.
	CommonPack,
	Readme,
	Autorun,
	Extra,
}

/// Where a pack entry is installed for one target.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryDest {
	pub target: String,
	pub path: String,
}

/// One file inside a pack.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackEntryPlan {
	/// The name it is stored under (`C0000001` in a common pack).
	pub stored: String,
	/// What it is called when installed.
	pub name: String,
	pub dest: Vec<EntryDest>,
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub raw: u64,
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub packed: u64,
	pub targets: Vec<String>,
	pub component: String,
}

/// What a pack file holds, for the details pane.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackSummary {
	pub which: PackWhich,
	/// The size of a chunk's window in bytes (4096 when Windows 3.x reads it, else 32768).
	pub chunk: u32,
	/// The id that ties a target's project to this pack.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub set_id: Option<String>,
}

/// One row of the disk map: a folder or a file, with the files of a pack beneath it.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskEntry {
	/// The name in its folder (`SETUP.LPK`).
	pub name: String,
	pub role: EntryRole,
	/// The size of the file in bytes (for a folder, its directory cluster).
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub raw: u64,
	/// The bytes it takes on the disk: the size rounded up to whole clusters.
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub on_disk: u64,
	/// The targets that use it.
	pub targets: Vec<String>,
	/// The component it belongs to, when it is one component's.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub component: Option<String>,
	/// Set on a pack.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub pack: Option<PackSummary>,
	/// The files of a pack.
	pub entries: Vec<PackEntryPlan>,
	/// The files of a folder.
	pub children: Vec<DiskEntry>,
}

/// One disk of a medium.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumePlan {
	/// 1 for the first disk.
	pub number: u32,
	pub label: String,
	/// The serial as `XXXX-XXXX`, on the first disk.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub serial: Option<String>,
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub capacity: u64,
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub used: u64,
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub free: u64,
	/// Bytes on this disk that belong to a common pack.
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub common: u64,
	pub root: Vec<DiskEntry>,
}

/// A target of the project that could be on the medium, and where it stands.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaTargetPlan {
	pub target: String,
	pub dir: String,
	pub on_medium: bool,
	/// The bytes of its `SETUP.EXE`, found or budgeted.
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub setup_bytes: u64,
	/// Where its `SETUP.EXE` was found.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub runtime: Option<String>,
	/// No `SETUP.EXE` was found, so `setup_bytes` is the budget.
	pub estimated: bool,
}

/// How big one component is and what its switch would change.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentSize {
	pub id: String,
	pub name: String,
	/// A required component cannot be left off.
	pub required: bool,
	/// The installer ticks the component by default, so a person must untick it to leave it out.
	pub default_on: bool,
	/// The component is on this medium (it is not in `exclude`).
	pub on_medium: bool,
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub raw: u64,
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub packed: u64,
	pub pack: PackWhich,
	pub targets: Vec<String>,
	/// The disk it is on; 0 when it is not on the medium.
	pub volume: u32,
	/// The change in free bytes if the switch were flipped: positive when leaving the component
	/// off frees space, negative when putting it back needs space.
	#[cfg_attr(feature = "ts", ts(as = "i32"))]
	pub delta_if_toggled: i64,
	/// The medium would still fit (and keep its reserve) after the switch is flipped.
	pub fits_if_toggled: bool,
}

/// One file of the payload, for the per-file table.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSize {
	pub path: String,
	pub component: String,
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub raw: u64,
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub packed: u64,
	pub pack: PackWhich,
	pub targets: Vec<String>,
	pub volume: u32,
}

/// The README a medium carries.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadmePlan {
	/// The text was generated from the targets; false when `readme` names a file.
	pub generated: bool,
	pub text: String,
}

/// Everything the Media screen draws for one medium.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaPlan {
	pub media_id: String,
	pub kind: MediaKind,
	pub label: String,
	/// The serial as `XXXX-XXXX`, worked out from the version when it is `auto`.
	pub serial: String,
	/// The bytes of one disk.
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub capacity: u64,
	/// The bytes of a cluster (a sector on a CD).
	pub cluster: u32,
	/// Bytes used on every disk together.
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub used: u64,
	/// Bytes free on the last disk.
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub free: u64,
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub reserve: u64,
	/// What the same medium would need if no file were shared, in whole clusters.
	#[cfg_attr(feature = "ts", ts(as = "u32"))]
	pub without_sharing: u64,
	pub volumes: Vec<VolumePlan>,
	pub targets: Vec<MediaTargetPlan>,
	pub components: Vec<ComponentSize>,
	pub files: Vec<FileSize>,
	pub warnings: Vec<MediaWarning>,
	pub readme: ReadmePlan,
	/// The text of `AUTORUN.INF`, on a CD that has one.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(feature = "ts", ts(optional))]
	pub autorun_text: Option<String>,
}
