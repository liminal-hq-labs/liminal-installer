// Media: the wire types and rules for the disks and discs a project ships on.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A *medium* is one floppy or disc that holds several targets (`SPEC.md` section 13). This crate
//! holds what the CLI, the Studio and the MCP tools share about it: the types they send each
//! other ([`MediaInfo`], [`MediaEdit`], [`MediaPlan`] and what a plan holds), the warnings a
//! medium can have ([`MediaWarning`]) and the rules a label, a serial and a folder must meet
//! ([`validate`]). It has no UI dependency. The planner, the format-keeping edit of
//! `installer.toml` and the build are added here, not in `li-cli` (D83).
//!
//! The types derive `ts_rs::TS` behind the `ts` feature, and their TypeScript is generated into
//! `packages/protocol` by `scripts/generate-protocol-bindings.sh`.

mod plan;
mod types;
mod validate;
mod warning;

pub use plan::{
	ComponentSize, DiskEntry, EntryDest, EntryRole, FileSize, MediaPlan, MediaTargetPlan,
	PackEntryPlan, PackSummary, PackWhich, ReadmePlan, VolumePlan,
};
pub use types::{CommonMode, MediaEdit, MediaInfo, MediaKind, MediaTarget, Overflow};
pub use validate::{
	FAT_LABEL_CHARS, LabelError, MAX_FAT_LABEL, MAX_ISO_ID, MAX_JOLIET_LABEL, MediaRuleError,
	Serial, check_dir, check_label, check_serial, format_serial, iso_volume_id, joliet_label,
	validate,
};
pub use warning::{MediaSeverity, MediaWarning, MediaWarningCode};
