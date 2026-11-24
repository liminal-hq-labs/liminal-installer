// The Program Manager commands a Windows 3.1 install sends over DDE, built and quoted as text.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Program Manager takes its commands as one string in a `WM_DDE_EXECUTE`: `[CreateGroup(Quill)]`.
//! Building the strings is separate from sending them, so the host tests (and the sim shell) see
//! exactly what the Windows 3.1 runtime sends.
//!
//! Rules:
//!
//! - An argument that holds `,` `(` `)` `[` or `]` is put in double quotes, so it cannot end the
//!   command early. So is one that starts or ends with a space, which Program Manager would trim.
//! - An argument that holds `"` is refused: Program Manager has no way to write one inside quotes.
//! - `IconPath` is the command itself when that names a program (`.EXE`, `.COM`, `.BAT` or
//!   `.PIF`); for a document or help file it is left empty and Program Manager shows the icon of
//!   the program the file is associated with.

use alloc::format;
use alloc::string::String;

use crate::Error;

/// The longest command string sent, in bytes. Program Manager copes with more, but the
/// runtime's DDE buffer is a fixed size.
pub const MAX_COMMAND: usize = 512;

/// One argument of a command, quoted when it has to be.
pub fn arg(text: &str) -> Result<String, Error> {
	if text.contains('"') {
		return Err(Error::Format(format!(
			"Program Manager cannot take a name with a double quote: {text}"
		)));
	}
	let plain = text
		.chars()
		.all(|c| !matches!(c, ',' | '(' | ')' | '[' | ']'))
		&& !text.starts_with(' ')
		&& !text.ends_with(' ');
	Ok(if plain {
		String::from(text)
	} else {
		format!("\"{text}\"")
	})
}

/// Whether `command` names a program, which has an icon of its own.
pub fn is_program(command: &str) -> bool {
	let upper = command.to_ascii_uppercase();
	[".EXE", ".COM", ".BAT", ".PIF"]
		.iter()
		.any(|e| upper.ends_with(e))
}

fn finish(body: String) -> Result<String, Error> {
	if body.len() > MAX_COMMAND {
		return Err(Error::Format(format!(
			"a Program Manager command of {} bytes is too long (the limit is {MAX_COMMAND})",
			body.len()
		)));
	}
	Ok(body)
}

/// The `.GRP` file name for a group: up to eight letters and digits of its name in upper case.
/// Program Manager writes it in the Windows folder.
pub fn group_file(group: &str) -> String {
	let mut stem: String = group
		.chars()
		.filter(char::is_ascii_alphanumeric)
		.take(8)
		.map(|c| c.to_ascii_uppercase())
		.collect();
	if stem.is_empty() {
		stem.push_str("GROUP");
	}
	format!("{stem}.GRP")
}

/// `[CreateGroup(group,FILE.GRP)]`. When the group exists already, Program Manager makes it the
/// active one and makes nothing.
pub fn create_group(group: &str) -> Result<String, Error> {
	finish(format!(
		"[CreateGroup({},{})]",
		arg(group)?,
		arg(&group_file(group))?
	))
}

/// `[ShowGroup(group,1)]`: opens the group window, restored.
pub fn show_group(group: &str) -> Result<String, Error> {
	finish(format!("[ShowGroup({},1)]", arg(group)?))
}

/// `[ReplaceItem(name)]`: removes the item of that name from the active group, so the `AddItem`
/// that follows does not make a second one. It fails (a NACK) when there is none, which is fine.
pub fn replace_item(name: &str) -> Result<String, Error> {
	finish(format!("[ReplaceItem({})]", arg(name)?))
}

/// `[AddItem(command,name,iconpath,0,-1,-1,workdir)]`, with the icon path left empty for a
/// document. The position `-1,-1` lets Program Manager place the icon.
pub fn add_item(command: &str, name: &str, work_dir: &str) -> Result<String, Error> {
	let icon = if is_program(command) {
		arg(command)?
	} else {
		String::new()
	};
	finish(format!(
		"[AddItem({},{},{icon},0,-1,-1,{})]",
		arg(command)?,
		arg(name)?,
		arg(work_dir)?
	))
}

/// The short form, `[AddItem(command,name,iconpath,0)]`, for a Program Manager that refuses the
/// long one. It leaves the working folder to Program Manager, which uses the program's own.
pub fn add_item_short(command: &str, name: &str) -> Result<String, Error> {
	let icon = if is_program(command) {
		arg(command)?
	} else {
		String::new()
	};
	finish(format!(
		"[AddItem({},{},{icon},0)]",
		arg(command)?,
		arg(name)?
	))
}

/// `[DeleteItem(name)]`, from the active group.
pub fn delete_item(name: &str) -> Result<String, Error> {
	finish(format!("[DeleteItem({})]", arg(name)?))
}

/// `[DeleteGroup(group)]`.
pub fn delete_group(group: &str) -> Result<String, Error> {
	finish(format!("[DeleteGroup({})]", arg(group)?))
}

/// Whether a `WM_DDE_DATA` list of groups (one name per line) names `group`, ignoring case.
/// Program Manager's own comparison ignores case.
pub fn group_listed(list: &str, group: &str) -> bool {
	list.lines()
		.any(|l| l.trim().eq_ignore_ascii_case(group.trim()))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn plain_arguments_stay_bare_and_special_ones_are_quoted() {
		assert_eq!(arg("Quill").unwrap(), "Quill");
		assert_eq!(arg("C:\\QUILL\\QUILL.EXE").unwrap(), "C:\\QUILL\\QUILL.EXE");
		assert_eq!(arg("Read Me").unwrap(), "Read Me");
		assert_eq!(arg("Fizz, Part 1").unwrap(), "\"Fizz, Part 1\"");
		assert_eq!(arg("A (b)").unwrap(), "\"A (b)\"");
		assert_eq!(arg("[x]").unwrap(), "\"[x]\"");
		assert_eq!(arg(" pad").unwrap(), "\" pad\"");
		assert!(arg("say \"hi\"").is_err());
	}

	#[test]
	fn the_icon_path_is_the_command_for_programs_only() {
		assert!(is_program("C:\\Q\\Q.exe"));
		assert!(is_program("C:\\Q\\RUN.BAT"));
		assert!(is_program("C:\\Q\\GAME.PIF"));
		assert!(!is_program("C:\\Q\\QUILL.HLP"));
		assert!(!is_program("C:\\Q\\README.TXT"));
	}

	#[test]
	fn commands_have_a_fixed_shape() {
		assert_eq!(
			create_group("Quill").unwrap(),
			"[CreateGroup(Quill,QUILL.GRP)]"
		);
		assert_eq!(show_group("Quill").unwrap(), "[ShowGroup(Quill,1)]");
		assert_eq!(replace_item("Quill").unwrap(), "[ReplaceItem(Quill)]");
		assert_eq!(
			add_item("C:\\QUILL\\QUILL.EXE", "Quill", "C:\\QUILL").unwrap(),
			"[AddItem(C:\\QUILL\\QUILL.EXE,Quill,C:\\QUILL\\QUILL.EXE,0,-1,-1,C:\\QUILL)]"
		);
		assert_eq!(
			add_item("C:\\QUILL\\QUILL.HLP", "Quill Help", "C:\\QUILL").unwrap(),
			"[AddItem(C:\\QUILL\\QUILL.HLP,Quill Help,,0,-1,-1,C:\\QUILL)]"
		);
		assert_eq!(
			add_item_short("C:\\QUILL\\QUILL.EXE", "Quill").unwrap(),
			"[AddItem(C:\\QUILL\\QUILL.EXE,Quill,C:\\QUILL\\QUILL.EXE,0)]"
		);
		assert_eq!(delete_item("Quill").unwrap(), "[DeleteItem(Quill)]");
		assert_eq!(delete_group("Quill").unwrap(), "[DeleteGroup(Quill)]");
	}

	#[test]
	fn a_group_file_is_eight_upper_case_characters_at_most() {
		assert_eq!(group_file("Quill"), "QUILL.GRP");
		assert_eq!(
			group_file("Lieutenant Fizz \u{2014} Episode One"),
			"LIEUTENA.GRP"
		);
		assert_eq!(group_file("\u{2014}"), "GROUP.GRP");
	}

	#[test]
	fn a_long_command_is_refused() {
		let long = "X".repeat(MAX_COMMAND);
		assert!(add_item(&long, "n", "C:\\").is_err());
	}

	#[test]
	fn the_group_list_is_compared_without_case() {
		let list = "Main\r\nQuill\r\nGames\r\n";
		assert!(group_listed(list, "QUILL"));
		assert!(!group_listed(list, "Quil"));
		assert!(!group_listed("", "Quill"));
	}
}
