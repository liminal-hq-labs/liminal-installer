// The rules a medium's label, serial and folders must meet.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::types::{MediaInfo, MediaKind};
use crate::warning::{MediaWarning, MediaWarningCode};

/// The longest volume label FAT allows.
pub const MAX_FAT_LABEL: usize = 11;
/// The longest Joliet volume label, in characters.
pub const MAX_JOLIET_LABEL: usize = 16;
/// The longest ISO 9660 volume id, in d-characters.
pub const MAX_ISO_ID: usize = 32;

/// The punctuation a FAT label and an 8.3 name may hold beside capital letters and digits.
///
/// This is the set `li_package::dos::check_label` accepts. The two are meant to be one rule:
/// until `li-package` has that function on `main` (the floppy writer's change), it is kept here,
/// and the test `matches_the_floppy_writers_label_rule` pins the set.
pub const FAT_LABEL_CHARS: &str = "_-!#$%&'()@^{}~";

/// Why a label was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LabelError {
	#[error("the label is {len} characters; the most is {max}")]
	TooLong { len: usize, max: usize },
	#[error("the label has `{ch}`, which a label may not hold")]
	BadChar { ch: char },
}

/// Why a serial or a folder was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MediaRuleError {
	#[error("a serial is `auto` or eight hexadecimal digits")]
	BadSerial,
	#[error("a folder is a single name with no `/` or `\\`")]
	NotOneFolder,
	#[error("`{0}` is not an 8.3 name")]
	Not83(String),
}

/// A serial: worked out from the version, or fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Serial {
	Auto,
	Fixed(u32),
}

/// Checks a FAT volume label: at most 11 characters, from the capital letters, the digits, the
/// space and [`FAT_LABEL_CHARS`]. An empty label passes.
pub fn check_label(label: &str) -> Result<(), LabelError> {
	check_label_with(label, MAX_FAT_LABEL, true)
}

fn check_label_with(label: &str, max: usize, space: bool) -> Result<(), LabelError> {
	let len = label.chars().count();
	if len > max {
		return Err(LabelError::TooLong { len, max });
	}
	let allowed = |c: char| {
		c.is_ascii_uppercase()
			|| c.is_ascii_digit()
			|| (space && c == ' ')
			|| FAT_LABEL_CHARS.contains(c)
	};
	match label.chars().find(|&c| !allowed(c)) {
		Some(ch) => Err(LabelError::BadChar { ch }),
		None => Ok(()),
	}
}

/// Reads a serial: `auto` in any letter case, or eight hexadecimal digits with an optional dash
/// after the fourth (`4C49-5253`).
pub fn check_serial(text: &str) -> Result<Serial, MediaRuleError> {
	if text.eq_ignore_ascii_case("auto") {
		return Ok(Serial::Auto);
	}
	let digits: String = match text.split_once('-') {
		Some((a, b)) if a.len() == 4 && b.len() == 4 => format!("{a}{b}"),
		_ => text.to_owned(),
	};
	if digits.len() == 8 && digits.bytes().all(|b| b.is_ascii_hexdigit()) {
		return u32::from_str_radix(&digits, 16)
			.map(Serial::Fixed)
			.map_err(|_| MediaRuleError::BadSerial);
	}
	Err(MediaRuleError::BadSerial)
}

/// A fixed serial as it is shown: `XXXX-XXXX`.
pub fn format_serial(serial: u32) -> String {
	format!("{:04X}-{:04X}", serial >> 16, serial & 0xffff)
}

/// Checks the folder of a target: empty for the root, or one 8.3 name (a stem of 1 to 8 and an
/// extension of 0 to 3 characters from the capital letters, the digits and [`FAT_LABEL_CHARS`]).
pub fn check_dir(dir: &str) -> Result<(), MediaRuleError> {
	if dir.is_empty() {
		return Ok(());
	}
	if dir.contains(['/', '\\']) {
		return Err(MediaRuleError::NotOneFolder);
	}
	let (stem, ext) = dir.split_once('.').unwrap_or((dir, ""));
	let part_ok = |s: &str, max: usize| {
		s.len() <= max
			&& s.chars().all(|c| {
				c.is_ascii_uppercase() || c.is_ascii_digit() || FAT_LABEL_CHARS.contains(c)
			})
	};
	if stem.is_empty() || !part_ok(stem, 8) || !part_ok(ext, 3) || ext.contains('.') {
		return Err(MediaRuleError::Not83(dir.to_owned()));
	}
	Ok(())
}

/// The ISO 9660 volume id for a label: capital letters, digits and `_`, with anything else
/// turned into `_`, at most [`MAX_ISO_ID`] characters.
pub fn iso_volume_id(label: &str) -> String {
	label
		.to_ascii_uppercase()
		.chars()
		.map(|c| {
			if c.is_ascii_uppercase() || c.is_ascii_digit() {
				c
			} else {
				'_'
			}
		})
		.take(MAX_ISO_ID)
		.collect()
}

/// The Joliet volume label for a label: the label cut to [`MAX_JOLIET_LABEL`] characters.
pub fn joliet_label(label: &str) -> String {
	label.chars().take(MAX_JOLIET_LABEL).collect()
}

/// Everything wrong with one medium's own entry, as warnings: the label, the serial, the
/// folders, a folder used twice and an `autorun` that names no target of the medium or sits on a
/// floppy. It checks no sizes; the planner adds those.
pub fn validate(info: &MediaInfo) -> Vec<MediaWarning> {
	use MediaWarningCode as Code;
	let mut out = Vec::new();
	let label = match info.kind {
		MediaKind::Floppy1440 => check_label(&info.label),
		MediaKind::Cdrom => check_label_with(&info.label, MAX_JOLIET_LABEL, true),
	};
	if let Err(e) = label {
		out.push(MediaWarning::error(Code::Label, "label", e.to_string()));
	}
	if let Err(e) = check_serial(&info.serial) {
		out.push(MediaWarning::error(Code::Label, "serial", e.to_string()));
	}
	for (i, t) in info.targets.iter().enumerate() {
		if let Err(e) = check_dir(&t.dir) {
			out.push(MediaWarning::error(
				Code::DirNot83,
				t.target.clone(),
				e.to_string(),
			));
		} else if info.targets[..i]
			.iter()
			.any(|o| o.dir.eq_ignore_ascii_case(&t.dir))
		{
			let place = if t.dir.is_empty() { "the root" } else { &t.dir };
			out.push(MediaWarning::error(
				Code::DirNot83,
				t.target.clone(),
				format!("{place} is used by more than one target"),
			));
		}
	}
	if let Some(auto) = &info.autorun {
		if info.kind != MediaKind::Cdrom {
			out.push(MediaWarning::warning(
				Code::AutorunTarget,
				auto.clone(),
				"only a CD has an AUTORUN.INF",
			));
		} else if !info.targets.iter().any(|t| &t.target == auto) {
			out.push(MediaWarning::error(
				Code::AutorunTarget,
				auto.clone(),
				format!("autorun names {auto}, which is not a target of this medium"),
			));
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::{CommonMode, MediaEdit, MediaTarget, Overflow};
	use crate::warning::MediaSeverity;

	fn target(t: &str, dir: &str) -> MediaTarget {
		MediaTarget {
			target: t.into(),
			dir: dir.into(),
		}
	}

	fn floppy() -> MediaInfo {
		MediaInfo {
			id: "floppy".into(),
			kind: MediaKind::Floppy1440,
			label: "QUILL".into(),
			serial: "auto".into(),
			common: CommonMode::Auto,
			exclude: vec!["dicts-more".into()],
			overflow: Overflow::Error,
			reserve: 16 * 1024,
			readme: None,
			extra: vec![],
			autorun: None,
			targets: vec![
				target("dos-x86", ""),
				target("win16-x86", "WIN31"),
				target("windows-x86", "WIN95"),
			],
		}
	}

	#[test]
	fn a_label_of_eleven_capitals_passes_and_twelve_do_not() {
		assert!(check_label("ABCDEFGHIJK").is_ok());
		assert_eq!(
			check_label("ABCDEFGHIJKL"),
			Err(LabelError::TooLong { len: 12, max: 11 })
		);
		assert!(check_label("").is_ok());
		assert!(check_label("QUILL 2").is_ok());
	}

	#[test]
	fn a_label_must_be_upper_case_and_use_only_fat_characters() {
		assert_eq!(check_label("quill"), Err(LabelError::BadChar { ch: 'q' }));
		for ch in [
			".", "*", "?", "/", "\\", ":", ";", "|", "=", ",", "\"", "<", ">", "[", "]", "+",
		] {
			let label = format!("A{ch}B");
			assert!(check_label(&label).is_err(), "{label} should be refused");
		}
		assert!(check_label("A_B-C!#$%&'").is_ok());
		assert!(check_label("()@^{}~ 1").is_ok());
	}

	#[test]
	fn matches_the_floppy_writers_label_rule() {
		// The characters `li_package::dos::check_label` accepts besides A-Z and 0-9.
		assert_eq!(FAT_LABEL_CHARS, "_-!#$%&'()@^{}~");
		assert_eq!(MAX_FAT_LABEL, 11);
	}

	#[test]
	fn a_serial_is_auto_or_eight_hex_digits() {
		assert_eq!(check_serial("auto"), Ok(Serial::Auto));
		assert_eq!(check_serial("AUTO"), Ok(Serial::Auto));
		assert_eq!(check_serial("4C495253"), Ok(Serial::Fixed(0x4c49_5253)));
		assert_eq!(check_serial("4c49-5253"), Ok(Serial::Fixed(0x4c49_5253)));
		for bad in [
			"",
			"4C49525",
			"4C4952530",
			"4C49-525",
			"4C49_5253",
			"ZZZZZZZZ",
			"-4C495253",
		] {
			assert_eq!(check_serial(bad), Err(MediaRuleError::BadSerial), "{bad}");
		}
	}

	#[test]
	fn a_serial_is_shown_as_two_groups_of_four() {
		assert_eq!(format_serial(0x4c49_5253), "4C49-5253");
		assert_eq!(format_serial(0x0000_00ab), "0000-00AB");
		assert_eq!(
			check_serial(&format_serial(0xdead_beef)),
			Ok(Serial::Fixed(0xdead_beef))
		);
	}

	#[test]
	fn a_folder_is_empty_or_one_83_name() {
		for ok in [
			"", "WIN31", "WIN95", "A", "ABCDEFGH", "DOS.X86", "A.B", "_1~",
		] {
			assert_eq!(check_dir(ok), Ok(()), "{ok}");
		}
		assert!(matches!(
			check_dir("ABCDEFGHI"),
			Err(MediaRuleError::Not83(_))
		));
		assert!(matches!(check_dir("win31"), Err(MediaRuleError::Not83(_))));
		assert!(matches!(check_dir("A.BCDE"), Err(MediaRuleError::Not83(_))));
		assert!(matches!(check_dir("A.B.C"), Err(MediaRuleError::Not83(_))));
		assert!(matches!(check_dir(".EXT"), Err(MediaRuleError::Not83(_))));
		assert!(matches!(check_dir("A B"), Err(MediaRuleError::Not83(_))));
		assert_eq!(check_dir("A/B"), Err(MediaRuleError::NotOneFolder));
		assert_eq!(check_dir("A\\B"), Err(MediaRuleError::NotOneFolder));
	}

	#[test]
	fn a_cd_label_is_derived_for_iso_and_joliet() {
		assert_eq!(iso_volume_id("Quill 1.0"), "QUILL_1_0");
		assert_eq!(iso_volume_id(&"A".repeat(40)).len(), MAX_ISO_ID);
		assert_eq!(joliet_label("ABCDEFGHIJKLMNOPQRS"), "ABCDEFGHIJKLMNOP");
	}

	#[test]
	fn a_good_medium_has_no_warnings() {
		assert_eq!(validate(&floppy()), vec![]);
	}

	#[test]
	fn a_bad_label_and_serial_are_label_errors() {
		let mut m = floppy();
		m.label = "quill release".into();
		m.serial = "12".into();
		let w = validate(&m);
		assert_eq!(w.len(), 2);
		assert!(w.iter().all(|w| w.code == MediaWarningCode::Label));
		assert!(w.iter().all(|w| w.severity == MediaSeverity::Error));
		assert_eq!(w[0].subject, "label");
		assert_eq!(w[1].subject, "serial");
	}

	#[test]
	fn a_cd_label_may_be_sixteen_characters() {
		let mut m = floppy();
		m.kind = MediaKind::Cdrom;
		m.label = "QUILL FOR WINDOWS".into();
		assert_eq!(validate(&m).len(), 1);
		m.label = "QUILL FOR WINDOW".into();
		assert_eq!(validate(&m), vec![]);
	}

	#[test]
	fn a_bad_or_repeated_folder_is_a_dir_error_naming_the_target() {
		let mut m = floppy();
		m.targets[1].dir = "WINDOWS31".into();
		m.targets[2].dir = String::new();
		let w = validate(&m);
		assert_eq!(w.len(), 2);
		assert_eq!(
			(w[0].code, w[0].subject.as_str()),
			(MediaWarningCode::DirNot83, "win16-x86")
		);
		assert_eq!(
			(w[1].code, w[1].subject.as_str()),
			(MediaWarningCode::DirNot83, "windows-x86")
		);
		assert!(w[1].message.contains("the root"));
		m.targets[1].dir = "win95".into();
		m.targets[2].dir = "WIN96".into();
		assert_eq!(
			validate(&m).len(),
			1,
			"win95 is not upper case, WIN96 is fine"
		);
		m.targets[2].dir = "WIN95".into();
		m.targets[1].dir = "WIN95".into();
		assert!(
			validate(&m)
				.iter()
				.any(|w| w.message.contains("more than one target"))
		);
	}

	#[test]
	fn autorun_needs_a_cd_and_a_target_of_the_medium() {
		let mut m = floppy();
		m.autorun = Some("windows-x86".into());
		let w = validate(&m);
		assert_eq!(w.len(), 1);
		assert_eq!(w[0].code, MediaWarningCode::AutorunTarget);
		assert_eq!(w[0].severity, MediaSeverity::Warning);

		m.kind = MediaKind::Cdrom;
		assert_eq!(validate(&m), vec![]);
		m.autorun = Some("windows-x86_64".into());
		let w = validate(&m);
		assert_eq!(w.len(), 1);
		assert_eq!(w[0].severity, MediaSeverity::Error);
	}

	#[test]
	fn an_edit_changes_only_what_it_names() {
		let m = floppy();
		let edit = MediaEdit {
			label: Some("QUILL 2".into()),
			reserve: Some(0),
			autorun: Some(String::new()),
			..MediaEdit::default()
		};
		let n = edit.apply(&m);
		assert_eq!(n.label, "QUILL 2");
		assert_eq!(n.reserve, 0);
		assert_eq!(n.targets, m.targets);
		assert_eq!(n.exclude, m.exclude);
		assert!(MediaEdit::default().is_empty());
		assert!(!edit.is_empty());
		assert_eq!(MediaEdit::default().apply(&m), m);
	}

	#[test]
	fn an_empty_readme_or_autorun_clears_it() {
		let mut m = floppy();
		m.readme = Some("media/README.TXT".into());
		m.autorun = Some("windows-x86".into());
		let n = MediaEdit {
			readme: Some(String::new()),
			autorun: Some(String::new()),
			..MediaEdit::default()
		}
		.apply(&m);
		assert_eq!((n.readme, n.autorun), (None, None));
	}

	#[test]
	fn the_wire_form_uses_the_toml_spellings() {
		let json = serde_json::to_value(floppy()).unwrap();
		assert_eq!(json["kind"], "floppy-1440");
		assert_eq!(json["common"], "auto");
		assert_eq!(json["overflow"], "error");
		assert!(json.get("readme").is_none(), "an absent readme is not sent");
		let back: MediaInfo = serde_json::from_value(json).unwrap();
		assert_eq!(back, floppy());
		let code = serde_json::to_value(MediaWarningCode::DefaultOnLaterDisk).unwrap();
		assert_eq!(code, "default_on_later_disk");
		assert_eq!(
			serde_json::to_value(MediaWarningCode::DirNot83).unwrap(),
			"dir_not_8_3"
		);
		assert_eq!(
			serde_json::to_value(MediaWarningCode::Name83).unwrap(),
			"name_83"
		);
	}

	#[test]
	fn every_code_is_listed_once() {
		let mut seen = std::collections::HashSet::new();
		for c in MediaWarningCode::ALL {
			assert!(seen.insert(c));
		}
		assert_eq!(seen.len(), 10);
	}
}
