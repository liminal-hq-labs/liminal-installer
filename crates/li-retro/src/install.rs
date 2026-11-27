// Install and uninstall: copying the payload, the plan operations, rollback and `INSTALL.LIR`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The installer engine for DOS. An install makes the install directory, extracts each selected
//! file straight from `SETUP.LPK` to disk (checking its CRC-32 as it goes), performs the plan
//! operations and writes the receipt `INSTALL.LIR` last. It keeps no journal: it remembers what
//! it created in memory, and a cancel or a failure removes exactly that. A file that was already
//! there is overwritten and, if the install is rolled back, left as the new copy. The exception is a
//! file placed in the Windows or the system folder (a destination that starts `%WINDIR%` or
//! `%SYSDIR%`, format 5): one that is not this product's from an earlier install is copied to
//! `BACKUP\WIN` or `BACKUP\SYS` below the install directory first, and put back by a rollback and by
//! the uninstall.
//!
//! # `INSTALL.LIR`
//!
//! A text file in the install directory, one record per line, `\r\n` line ends:
//!
//! ```text
//! LIR 1
//! product=hq.liminal.lt-fizz
//! version=1.0.0
//! target=dos-x86:tui-retro
//! dir=C:\FIZZ
//! components=game,sound,levels
//! D .
//! D SOUND
//! F FIZZ.EXE 1234 1A2B3C4D
//! F SOUND\ADLIB.DRV 4096 0000BEEF
//! I SOUND.INI
//! B FIZZ.BAT
//! ```
//!
//! A file type the install registered is one more record, `A ext|prog_id|exe|previous`: the
//! extension, the type's name, the program that opens it and what owned the extension before. The
//! last field is left out when nothing did. `%`, `|`, CR and LF in a field are written `%25`,
//! `%7C`, `%0D` and `%0A`.
//!
//! A file in the Windows or system folder is an `F` record by full path (`F C:\WINDOWS\X.SCR 12
//! 0000ABCD`), and one it replaced is `O original|backup`: the full path, then the copy below the
//! install directory (`BACKUP\WIN\X.SCR`), escaped as the `A` record is.
//!
//! `D` is a directory the install created (`.` is the install directory itself), `F` a file with
//! its size and CRC-32 in hexadecimal, `I` an INI file the install wrote and `B` a launcher batch
//! file. Uninstall removes `F`, `I` and `B` files, copies each `O` backup back over its original,
//! then removes the `D` directories that are empty, in reverse order, and last the receipt.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::Error;
use crate::model::{FileEntry, Op, Project, rooted};
use crate::pack::{Crc, Pack};
use crate::platform::{GroupAdd, Platform, ShortcutKind};
use crate::ui::Values;

/// The receipt's file name.
pub const RECEIPT: &str = "INSTALL.LIR";

/// What the engine reports while it works.
#[derive(Debug, Clone, Default)]
pub struct Progress {
	/// The file being copied or removed, below the install directory.
	pub file: String,
	pub files_done: u32,
	pub files_total: u32,
	pub bytes_done: u64,
	pub bytes_total: u64,
	/// The part of the current file done, in whole percent.
	pub file_pct: u32,
	/// Free bytes on the drive now, and when the install began.
	pub free: u64,
	pub free_at_start: u64,
}

fn join(dir: &str, rel: &str) -> String {
	if rel == "." || rel.is_empty() {
		dir.to_string()
	} else if rel.contains(':') || rel.starts_with("\\\\") {
		rel.to_string()
	} else if dir.ends_with('\\') {
		format!("{dir}{rel}")
	} else {
		format!("{dir}\\{rel}")
	}
}

/// The install directory without a trailing backslash, except for a drive root.
pub fn normalize_dir(dir: &str) -> String {
	let d = dir.trim().replace('/', "\\").to_ascii_uppercase();
	let d = d.trim_end_matches('\\');
	d.to_string()
}

/// [`normalize_dir`] for a machine that keeps the case of long names (`Shell::long_names`): the same
/// cleaning without the upper-casing.
pub fn normalize_dir_for<P: Platform>(p: &mut P, dir: &str) -> String {
	if p.shell().is_some_and(|s| s.long_names()) {
		let d = dir.trim().replace('/', "\\");
		d.trim_end_matches('\\').to_string()
	} else {
		normalize_dir(dir)
	}
}

/// The files a set of components installs, in the order the receipt lists them.
pub fn selected_files<'p>(project: &'p Project, selected: &[String]) -> Vec<&'p FileEntry> {
	// The compiler writes `files` in destination order and `Project::validate` checks it, so the
	// receipt order needs no sort here.
	let files: Vec<&FileEntry> = project
		.files
		.iter()
		.filter(|f| {
			let c = &project.components[usize::from(f.component)];
			c.required || selected.contains(&c.id)
		})
		.collect();
	files
}

/// Whether operation `i` of the project runs: nothing owns it, or its component is installed.
fn runs(project: &Project, selected: &[String], i: usize) -> bool {
	match project.owners.get(i) {
		Some(&o) if o != crate::model::NO_OWNER => {
			let c = &project.components[usize::from(o)];
			c.required || selected.contains(&c.id)
		}
		_ => true,
	}
}

/// The bytes a set of components needs on disk, with a little room for directory entries.
pub fn bytes_needed(project: &Project, selected: &[String]) -> u64 {
	let files = selected_files(project, selected);
	files.iter().map(|f| u64::from(f.size)).sum::<u64>() + files.len() as u64 * 512
}

struct Journal {
	files: Vec<String>,
	dirs: Vec<String>,
	/// A file this install replaced in the Windows or system folder: the backup and the original.
	restores: Vec<(String, String)>,
}

fn rollback<P: Platform>(p: &mut P, j: &Journal) {
	// Back before the backups are removed with the other files this install made.
	for (backup, original) in j.restores.iter().rev() {
		if let Some(s) = p.shell() {
			let _ = s.copy_file(backup, original);
		}
	}
	for f in j.files.iter().rev() {
		let _ = p.remove_file(f);
	}
	for d in j.dirs.iter().rev() {
		let _ = p.remove_dir(d);
	}
}

/// Removes Program Manager items, given as `group|name`, one conversation for each group, and
/// the groups in `made` as well when they are left empty by it.
fn remove_groups<P: Platform>(p: &mut P, items: &[String], made: &[String]) {
	let mut groups: Vec<&str> = Vec::new();
	for g in items {
		if let Some((group, _)) = g.split_once('|')
			&& !groups.contains(&group)
		{
			groups.push(group);
		}
	}
	for group in groups {
		let names: Vec<&str> = items
			.iter()
			.filter_map(|g| g.split_once('|'))
			.filter(|(gr, _)| *gr == group)
			.map(|(_, n)| n)
			.collect();
		let made_group = made.iter().any(|m| m == group);
		if let Some(s) = p.shell() {
			s.remove_group(group, &names, made_group);
		}
	}
}

/// Where a file replaced in the Windows folder is kept, and the one for the system folder.
const BACKUP: &str = "BACKUP";
const WIN: &str = "WIN";
const SYS: &str = "SYS";

/// `BACKUP\WIN`, `BACKUP\SYS` or, for `None`, `BACKUP`.
fn backup_folder(sub: Option<&str>) -> String {
	let mut s = BACKUP.to_string();
	if let Some(sub) = sub {
		s.push('\\');
		s.push_str(sub);
	}
	s
}

pub(crate) fn no_folder() -> Error {
	Error::Io("no Windows folder here".to_string())
}

/// Whether `dest` is in the Windows or the system folder rather than below the install directory.
fn is_rooted(dest: &str) -> bool {
	rooted(dest).is_some()
}

/// The full path a file's destination is installed to. A destination in the Windows or system folder
/// is built by concatenation, never through [`join`], so a folder on a network share works.
fn target_path<P: Platform>(p: &mut P, dir: &str, dest: &str) -> Result<String, Error> {
	let Some((windows, name)) = rooted(dest) else {
		return Ok(join(dir, dest));
	};
	let folder = p.shell().and_then(|s| {
		if windows {
			s.windows_dir()
		} else {
			s.system_dir()
		}
	});
	let mut out = folder.ok_or_else(no_folder)?;
	out.push('\\');
	out.push_str(name);
	Ok(out)
}

/// Gets a file in the Windows or system folder out of the way of the one about to replace it. A
/// file that an earlier install of this product put there is simply replaced, and the backup that
/// install made is carried on; anything else is copied to `BACKUP\WIN` or `BACKUP\SYS` first.
#[allow(clippy::too_many_arguments)]
fn place_rooted<P: Platform>(
	p: &mut P,
	dir: &str,
	dest: &str,
	to: &str,
	before: &Receipt,
	j: &mut Journal,
	made: &mut Made,
) -> Result<(), Error> {
	if before.files.iter().any(|f| f.0.eq_ignore_ascii_case(to)) {
		for o in before
			.overwritten
			.iter()
			.filter(|o| o.0.eq_ignore_ascii_case(to))
		{
			made.overwritten.push(o.clone());
		}
		return Ok(());
	}
	if !p.exists(to) {
		return Ok(());
	}
	let (windows, name) = rooted(dest).unwrap_or((true, dest));
	let folder = backup_folder(Some(if windows { WIN } else { SYS }));
	make_dirs(p, &join(dir, &folder), j)?;
	let mut rel = folder;
	rel.push('\\');
	rel.push_str(name);
	let backup = join(dir, &rel);
	let had = p.exists(&backup);
	match p.shell() {
		Some(s) => s.copy_file(to, &backup)?,
		None => return Err(no_folder()),
	}
	if !had {
		j.files.push(backup.clone());
	}
	j.restores.push((backup, to.to_string()));
	let mut note = String::from("backup: ");
	note.push_str(to);
	note.push_str(" -> ");
	note.push_str(&rel);
	p.notice(&note);
	made.overwritten.push((to.to_string(), rel));
	Ok(())
}

/// Makes `dir` and every parent below the drive root, noting what was made.
fn make_dirs<P: Platform>(p: &mut P, dir: &str, j: &mut Journal) -> Result<(), Error> {
	let mut acc = String::new();
	for (i, part) in dir.split('\\').enumerate() {
		if i == 0 {
			acc.push_str(part);
			if part.ends_with(':') {
				continue;
			}
		} else {
			acc.push('\\');
			acc.push_str(part);
		}
		if p.mkdir(&acc)? {
			j.dirs.push(acc.clone());
		}
	}
	Ok(())
}

/// Reads one file out of the pack and writes it to `to`, checking size and CRC-32. `on_bytes`
/// gets the bytes written so far and returns false to cancel. The pack checks each chunk before
/// it is returned, so a damaged chunk is never written.
fn extract<P: Platform>(
	p: &mut P,
	pack: &mut Pack,
	f: &FileEntry,
	to: &str,
	j: &mut Journal,
	on_bytes: &mut dyn FnMut(&mut P, u64) -> bool,
) -> Result<(), Error> {
	let existed = p.exists(to);
	let handle = p.create(to)?;
	if !existed {
		j.files.push(to.to_string());
	}
	let mut crc = Crc::default();
	let mut total = 0u64;
	let result = (|| {
		let mut x = pack.begin(p, &f.stored)?;
		while let Some(chunk) = pack.next(p, &mut x)? {
			// Written in pieces, so progress and Esc are answered a few times a second on a floppy.
			for piece in chunk.chunks(4096) {
				crc.update(piece);
				total += piece.len() as u64;
				p.write(handle, piece)?;
				if !on_bytes(p, total) {
					return Err(Error::Cancelled);
				}
			}
		}
		Ok(())
	})();
	let closed = p.close(handle);
	result?;
	closed?;
	if total != u64::from(f.size) || crc.value() != f.crc {
		return Err(Error::Corrupt(format!(
			"{} does not match its checksum; the disk may be damaged",
			f.dest
		)));
	}
	Ok(())
}

/// Sets `key=value` in the INI text, adding the section when it is missing.
pub fn ini_set(text: &str, section: &str, key: &str, value: &str) -> String {
	let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
	let header = format!("[{section}]");
	let line = format!("{key}={value}");
	let start = lines
		.iter()
		.position(|l| l.trim().eq_ignore_ascii_case(&header));
	match start {
		None => {
			if lines.last().is_some_and(|l| !l.is_empty()) {
				lines.push(String::new());
			}
			lines.push(header);
			lines.push(line);
		}
		Some(s) => {
			let end = lines[s + 1..]
				.iter()
				.position(|l| l.trim_start().starts_with('['))
				.map_or(lines.len(), |e| s + 1 + e);
			let prefix = format!("{}=", key.to_ascii_lowercase());
			let hit = lines[s + 1..end]
				.iter()
				.position(|l| l.trim_start().to_ascii_lowercase().starts_with(&prefix));
			match hit {
				Some(h) => lines[s + 1 + h] = line,
				None => {
					let mut at = end;
					while at > s + 1 && lines[at - 1].is_empty() {
						at -= 1;
					}
					lines.insert(at, line);
				}
			}
		}
	}
	let mut out = lines.join("\r\n");
	out.push_str("\r\n");
	out
}

/// A shortcut made through the machine's shell: a `.lnk` file on the Start menu in a folder named for
/// the product, or an item in a Program Manager group named for it. Returns false when the shell
/// has no way to make one, and the caller writes the batch launcher instead.
fn shell_shortcut<P: Platform>(
	p: &mut P,
	project: &Project,
	dir: &str,
	name: &str,
	target: &str,
	j: &mut Journal,
	made: &mut Made,
) -> Result<bool, Error> {
	let Some(kind) = p.shell().map(|s| s.shortcut_kind()) else {
		return Ok(false);
	};
	if kind == ShortcutKind::GroupItem {
		return group_item(p, project, name, &join(dir, target), dir, made);
	}
	let Some(programs) = p.shell().and_then(|s| s.start_menu_programs()) else {
		return Ok(true);
	};
	let folder = format!("{programs}\\{}", file_safe(&project.name));
	if p.mkdir(&folder)? {
		j.dirs.push(folder.clone());
	}
	let lnk = format!("{folder}\\{}.lnk", file_safe(name));
	let existed = p.exists(&lnk);
	if let Some(s) = p.shell() {
		s.make_shortcut(&lnk, &join(dir, target), dir, name)?;
	}
	if !existed {
		j.files.push(lnk.clone());
	}
	made.shortcuts.push(lnk);
	Ok(true)
}

/// An item in the product's Program Manager group. False when there is no Program Manager to
/// make it, and the caller falls back to a batch launcher.
fn group_item<P: Platform>(
	p: &mut P,
	project: &Project,
	name: &str,
	command: &str,
	dir: &str,
	made: &mut Made,
) -> Result<bool, Error> {
	let added = match p.shell() {
		Some(s) => s.add_group_item(&project.name, name, command, dir)?,
		None => GroupAdd::Unavailable,
	};
	Ok(match added {
		GroupAdd::Unavailable => false,
		GroupAdd::Added { made_group } => {
			made.groups.push(format!("{}|{name}", project.name));
			// Only the call that made the group sees it missing; later items find it there.
			if made_group && !made.made_groups.contains(&project.name) {
				made.made_groups.push(project.name.clone());
			}
			true
		}
	})
}

/// Registers a file type through the shell, when it has any, noting what it replaced.
#[allow(clippy::too_many_arguments)]
fn file_type<P: Platform>(
	p: &mut P,
	dir: &str,
	ext: &str,
	prog_id: &str,
	name: &str,
	target: &str,
	before: &[FileTypeRecord],
	made: &mut Made,
) -> Result<(), Error> {
	if !p.shell().is_some_and(|s| s.file_types()) {
		return Ok(());
	}
	let exe = join(dir, target);
	// A line that was there but empty is no owner: there is nothing to give back.
	let mut previous = match p.shell() {
		Some(s) => s
			.register_file_type(ext, prog_id, name, &exe)?
			.filter(|v| !v.trim().is_empty()),
		None => None,
	};
	// What was there is what the last install put there: keep what that one replaced.
	let ours = |v: &str| {
		let v = v.to_ascii_uppercase();
		let own = |e: &str| v.starts_with(&e.to_ascii_uppercase());
		v == prog_id.to_ascii_uppercase() || own(&exe) || before.iter().any(|b| own(&b.exe))
	};
	if let Some(old) = before.iter().find(|b| b.ext == ext)
		&& previous.as_deref().is_some_and(ours)
	{
		previous = old.previous.clone();
	}
	made.file_types.push(FileTypeRecord {
		ext: ext.to_string(),
		prog_id: prog_id.to_string(),
		exe,
		previous,
	});
	Ok(())
}

/// A name with the characters a file name cannot hold taken out.
fn file_safe(name: &str) -> String {
	let s: String = name
		.chars()
		.map(|c| if "\\/:*?\"<>|".contains(c) { ' ' } else { c })
		.collect();
	s.trim().to_string()
}

fn write_text<P: Platform>(p: &mut P, path: &str, body: &str) -> Result<(), Error> {
	let h = p.create(path)?;
	let r = p.write(h, body.as_bytes());
	let c = p.close(h);
	r?;
	c
}

/// What an install left behind, for the receipt.
struct Made {
	files: Vec<(String, u32, u32)>,
	inis: Vec<String>,
	batches: Vec<String>,
	/// Shortcut files (full paths) and the uninstaller's files, which the shell machine adds.
	shortcuts: Vec<String>,
	extras: Vec<String>,
	keys: Vec<String>,
	/// Program Manager items, as `group|name`.
	groups: Vec<String>,
	/// The Program Manager groups this install made, so the uninstall may delete them.
	made_groups: Vec<String>,
	/// The file types registered, in the order made.
	file_types: Vec<FileTypeRecord>,
	/// A file in the Windows or system folder this install replaced: the full path and the backup
	/// below the install directory (the receipt's `O`).
	overwritten: Vec<(String, String)>,
}

/// A file type an install registered and what it replaced: the receipt's `A` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTypeRecord {
	/// The extension, without a dot, in lower case.
	pub ext: String,
	/// The type's name for the registry.
	pub prog_id: String,
	/// The full path of the program that opens the type.
	pub exe: String,
	/// What owned the extension before this install, if anything.
	pub previous: Option<String>,
}

const ESCAPED: [(char, &str); 4] = [('%', "%25"), ('|', "%7C"), ('\r', "%0D"), ('\n', "%0A")];

fn escape(s: &str) -> String {
	let mut out = String::new();
	for c in s.chars() {
		match ESCAPED.iter().find(|(e, _)| *e == c) {
			Some((_, code)) => out.push_str(code),
			None => out.push(c),
		}
	}
	out
}

fn unescape(s: &str) -> String {
	let mut out = s.to_string();
	// `%25` last, so the text it produces is not read as another escape.
	for (c, code) in ESCAPED.iter().rev() {
		out = out.replace(code, c.encode_utf8(&mut [0u8; 4]));
	}
	out
}

impl FileTypeRecord {
	fn line(&self) -> String {
		let mut l = format!(
			"A {}|{}|{}",
			escape(&self.ext),
			escape(&self.prog_id),
			escape(&self.exe)
		);
		if let Some(prev) = &self.previous {
			l.push('|');
			l.push_str(&escape(prev));
		}
		l
	}

	fn parse(rest: &str) -> Option<FileTypeRecord> {
		let mut it = rest.splitn(4, '|');
		Some(FileTypeRecord {
			ext: unescape(it.next()?),
			prog_id: unescape(it.next()?),
			exe: unescape(it.next()?),
			previous: it.next().map(unescape),
		})
	}
}

/// Takes back the file types in `types`, last made first.
fn unregister_file_types<P: Platform>(p: &mut P, types: &[FileTypeRecord]) {
	for t in types.iter().rev() {
		if let Some(s) = p.shell() {
			s.unregister_file_type(&t.ext, &t.prog_id, &t.exe, t.previous.as_deref());
		}
	}
}

/// Installs the selected components (and the required ones) into `dir`.
///
/// `pack` is the open `SETUP.LPK`. `v` supplies the values INI
/// operations are written from. `progress` is called as files are copied and returns false to
/// cancel, which removes everything this call made and returns [`Error::Cancelled`].
pub fn install<P: Platform>(
	p: &mut P,
	project: &Project,
	pack: &mut Pack,
	dir: &str,
	selected: &[String],
	v: &Values,
	progress: &mut dyn FnMut(&mut P, &Progress) -> bool,
) -> Result<(), Error> {
	let dir = normalize_dir_for(p, dir);
	let files = selected_files(project, selected);
	let need = bytes_needed(project, selected);
	let free = p.free_space(&dir).unwrap_or(u64::MAX);
	if free < need {
		return Err(Error::NoSpace { need, have: free });
	}
	let mut j = Journal {
		files: Vec::new(),
		dirs: Vec::new(),
		restores: Vec::new(),
	};
	let mut made = Made {
		files: Vec::new(),
		inis: Vec::new(),
		batches: Vec::new(),
		shortcuts: Vec::new(),
		extras: Vec::new(),
		keys: Vec::new(),
		groups: Vec::new(),
		made_groups: Vec::new(),
		file_types: Vec::new(),
		overwritten: Vec::new(),
	};
	// A reinstall finds its own type where the last install put it; what that install replaced is
	// still the owner to restore. Its files in the Windows folders are ours, not another's to back up.
	let before = Receipt::read(p, &dir).unwrap_or_default();
	let result = do_install(
		p, project, pack, &dir, selected, v, &files, free, &before, &mut j, &mut made, progress,
	);
	if result.is_err() {
		rollback(p, &j);
		// The items go before the group, and the files they start are already gone.
		remove_groups(p, &made.groups, &made.made_groups);
		unregister_file_types(p, &made.file_types);
	}
	result
}

#[allow(clippy::too_many_arguments)]
fn do_install<P: Platform>(
	p: &mut P,
	project: &Project,
	pack: &mut Pack,
	dir: &str,
	selected: &[String],
	v: &Values,
	files: &[&FileEntry],
	free: u64,
	before: &Receipt,
	j: &mut Journal,
	made: &mut Made,
	progress: &mut dyn FnMut(&mut P, &Progress) -> bool,
) -> Result<(), Error> {
	make_dirs(p, dir, j)?;
	let mut pr = Progress {
		files_total: files.len() as u32,
		bytes_total: files.iter().map(|f| u64::from(f.size)).sum(),
		free,
		free_at_start: free,
		..Progress::default()
	};
	if !progress(p, &pr) {
		return Err(Error::Cancelled);
	}
	let mut done_bytes = 0u64;
	for f in files {
		let to = target_path(p, dir, &f.dest)?;
		let outside = is_rooted(&f.dest);
		if outside {
			place_rooted(p, dir, &f.dest, &to, before, j, made)?;
		} else if let Some((parent, _)) = f.dest.rsplit_once('\\') {
			make_dirs(p, &join(dir, parent), j)?;
		}
		pr.file = if outside { to.clone() } else { f.dest.clone() };
		pr.file_pct = 0;
		let mut cancelled = false;
		let size = u64::from(f.size).max(1);
		let base = done_bytes;
		extract(p, pack, f, &to, j, &mut |p, n| {
			pr.file_pct = (n * 100 / size).min(100) as u32;
			pr.bytes_done = base + n;
			let keep = progress(p, &pr);
			cancelled |= !keep;
			keep
		})?;
		if cancelled {
			return Err(Error::Cancelled);
		}
		done_bytes += u64::from(f.size);
		pr.files_done += 1;
		pr.bytes_done = done_bytes;
		pr.file_pct = 100;
		pr.free = p.free_space(dir).unwrap_or(free);
		made.files
			.push((if outside { to } else { f.dest.clone() }, f.size, f.crc));
		if !progress(p, &pr) {
			return Err(Error::Cancelled);
		}
	}
	for (i, op) in project.ops.iter().enumerate() {
		if !runs(project, selected, i) {
			continue;
		}
		match op {
			Op::Mkdir { path } => make_dirs(p, &join(dir, path), j)?,
			Op::WriteIni {
				path,
				section,
				key,
				value,
				when_set,
			} => {
				if !when_set.is_empty() && v.str(when_set).is_none_or(str::is_empty) {
					continue;
				}
				let to = join(dir, path);
				let existed = p.exists(&to);
				let old = if existed {
					String::from_utf8_lossy(&p.read_file(&to)?).into_owned()
				} else {
					String::new()
				};
				let body = ini_set(&old, section, key, &v.interpolate(value));
				write_text(p, &to, &body)?;
				if !existed {
					j.files.push(to);
					made.inis.push(path.clone());
				}
			}
			Op::Shortcut {
				name,
				target,
				batch,
			} => {
				if shell_shortcut(p, project, dir, name, target, j, made)? {
					continue;
				}
				let to = join(dir, batch);
				let existed = p.exists(&to);
				let drive = dir.get(..2).filter(|d| d.ends_with(':')).unwrap_or("");
				let cd = dir.get(2..).filter(|_| !drive.is_empty()).unwrap_or(dir);
				let mut body = String::from("@echo off\r\n");
				if !drive.is_empty() {
					body.push_str(&format!("{drive}\r\n"));
				}
				body.push_str(&format!("cd {cd}\r\n{target} %1 %2 %3 %4\r\n"));
				write_text(p, &to, &body)?;
				if !existed {
					j.files.push(to);
				}
				made.batches.push(batch.clone());
			}
			Op::FileType {
				ext,
				prog_id,
				name,
				target,
			} => file_type(p, dir, ext, prog_id, name, target, &before.file_types, made)?,
		}
	}
	if p.shell().is_some_and(|s| s.keeps_uninstaller()) {
		let streamed = p.shell().is_some_and(|s| s.streams_project());
		let bytes = if streamed {
			Vec::new()
		} else {
			pack.project_bytes(p)?
		};
		let names = match p.shell() {
			Some(s) => s.install_uninstaller(dir, &bytes)?,
			None => Vec::new(),
		};
		for name in names {
			j.files.push(join(dir, &name));
			made.extras.push(name);
		}
		if streamed {
			let dat = join(dir, "UNINST.DAT");
			j.files.push(dat.clone());
			pack.write_project(p, &dat)?;
			made.extras.push("UNINST.DAT".to_string());
		}
		// Where the shell has groups, the uninstaller is an item in the product's group.
		if p.shell().map(|s| s.shortcut_kind()) == Some(ShortcutKind::GroupItem) {
			let item = format!("Uninstall {}", project.name);
			let command = format!("{} /U {dir}", join(dir, "UNINST.EXE"));
			group_item(p, project, &item, &command, dir, made)?;
		}
	}
	if p.shell().is_some_and(|s| s.registers_uninstall()) {
		let uninstall = format!("\"{}\" /U \"{dir}\"", join(dir, "UNINST.EXE"));
		let key = project.id.clone();
		if let Some(s) = p.shell() {
			s.register_uninstall(
				&key,
				&[
					("DisplayName", &project.name),
					("DisplayVersion", &project.version),
					("Publisher", &project.publisher),
					("InstallLocation", dir),
					("UninstallString", &uninstall),
				],
			)?;
		}
		made.keys.push(key);
	}
	let mut lir = String::from("LIR 1\r\n");
	lir.push_str(&format!(
		"product={}\r\nversion={}\r\ntarget={}\r\ndir={}\r\n",
		project.id, project.version, project.target, dir
	));
	let ids: Vec<&str> = project
		.components
		.iter()
		.filter(|c| c.required || selected.contains(&c.id))
		.map(|c| c.id.as_str())
		.collect();
	lir.push_str(&format!("components={}\r\n", ids.join(",")));
	let own: Vec<&String> = j.dirs.iter().collect();
	for d in own {
		let rel = d
			.strip_prefix(dir)
			.map(|r| r.trim_start_matches('\\'))
			.unwrap_or(d);
		lir.push_str(&format!("D {}\r\n", if rel.is_empty() { "." } else { rel }));
	}
	for (dest, size, crc) in &made.files {
		lir.push_str(&format!("F {dest} {size} {crc:08X}\r\n"));
	}
	for (original, backup) in &made.overwritten {
		lir.push_str("O ");
		lir.push_str(&escape(original));
		lir.push('|');
		lir.push_str(&escape(backup));
		lir.push_str("\r\n");
	}
	for i in &made.inis {
		lir.push_str(&format!("I {i}\r\n"));
	}
	for b in &made.batches {
		lir.push_str(&format!("B {b}\r\n"));
	}
	for x in &made.extras {
		lir.push_str(&format!("X {x}\r\n"));
	}
	for sc in &made.shortcuts {
		lir.push_str(&format!("S {sc}\r\n"));
	}
	for k in &made.keys {
		lir.push_str(&format!("R {k}\r\n"));
	}
	for g in &made.groups {
		lir.push_str(&format!("G {g}\r\n"));
	}
	for g in &made.made_groups {
		lir.push_str(&format!("P {g}\r\n"));
	}
	for t in &made.file_types {
		lir.push_str(&t.line());
		lir.push_str("\r\n");
	}
	let receipt = join(dir, RECEIPT);
	// Noted first, so a receipt that fails half way through is rolled back with the rest.
	j.files.push(receipt.clone());
	write_text(p, &receipt, &lir)?;
	Ok(())
}

/// A parsed `INSTALL.LIR`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Receipt {
	pub product: String,
	pub version: String,
	pub target: String,
	pub dir: String,
	pub components: Vec<String>,
	pub dirs: Vec<String>,
	/// Relative path, size and CRC-32.
	pub files: Vec<(String, u32, u32)>,
	/// INI and batch files, which have no checksum.
	pub others: Vec<String>,
	/// The shortcut files (full paths) and the registry keys a Windows install made.
	pub shortcuts: Vec<String>,
	pub keys: Vec<String>,
	/// The Program Manager items a Windows 3.1 install made, as `group|name`.
	pub groups: Vec<String>,
	/// The Program Manager groups the install made (`P`), which the uninstall deletes; a group it
	/// found there is left.
	pub made_groups: Vec<String>,
	/// The uninstaller's own files, below the install folder.
	pub extras: Vec<String>,
	/// The file types the install registered (`A`).
	pub file_types: Vec<FileTypeRecord>,
	/// The files in the Windows or system folder the install replaced (`O`): the full path of the
	/// original and its backup below the install directory.
	pub overwritten: Vec<(String, String)>,
}

impl Receipt {
	pub fn parse(text: &str) -> Result<Receipt, Error> {
		let mut lines = text.lines();
		if lines.next().map(str::trim) != Some("LIR 1") {
			return Err(Error::Format(
				"INSTALL.LIR is not a version 1 receipt".to_string(),
			));
		}
		let mut r = Receipt::default();
		for line in lines {
			let line = line.trim_end();
			if let Some((k, val)) = line.split_once('=') {
				match k {
					"product" => r.product = val.to_string(),
					"version" => r.version = val.to_string(),
					"target" => r.target = val.to_string(),
					"dir" => r.dir = val.to_string(),
					"components" => {
						r.components = val
							.split(',')
							.filter(|s| !s.is_empty())
							.map(str::to_string)
							.collect()
					}
					_ => {}
				}
			} else if let Some(rest) = line.strip_prefix("D ") {
				r.dirs.push(rest.to_string());
			} else if let Some(rest) = line.strip_prefix("F ") {
				let mut it = rest.rsplitn(3, ' ');
				let crc = it.next().and_then(|c| u32::from_str_radix(c, 16).ok());
				let size = it.next().and_then(|c| c.parse().ok());
				match (it.next(), size, crc) {
					(Some(name), Some(size), Some(crc)) => {
						r.files.push((name.to_string(), size, crc))
					}
					_ => return Err(Error::Format(format!("bad receipt line: {line}"))),
				}
			} else if let Some(rest) = line.strip_prefix("I ").or_else(|| line.strip_prefix("B ")) {
				r.others.push(rest.to_string());
			} else if let Some(rest) = line.strip_prefix("S ") {
				r.shortcuts.push(rest.to_string());
			} else if let Some(rest) = line.strip_prefix("X ") {
				r.extras.push(rest.to_string());
			} else if let Some(rest) = line.strip_prefix("R ") {
				r.keys.push(rest.to_string());
			} else if let Some(rest) = line.strip_prefix("G ") {
				r.groups.push(rest.to_string());
			} else if let Some(rest) = line.strip_prefix("P ") {
				r.made_groups.push(rest.to_string());
			} else if let Some(rest) = line.strip_prefix("O ")
				&& let Some((original, backup)) = rest.split_once('|')
			{
				r.overwritten.push((unescape(original), unescape(backup)));
			} else if let Some(rest) = line.strip_prefix("A ")
				&& let Some(t) = FileTypeRecord::parse(rest)
			{
				r.file_types.push(t);
			}
		}
		if r.dir.is_empty() {
			return Err(Error::Format(
				"INSTALL.LIR names no install directory".to_string(),
			));
		}
		Ok(r)
	}

	pub fn read<P: Platform>(p: &mut P, dir: &str) -> Result<Receipt, Error> {
		let path = join(&normalize_dir_for(p, dir), RECEIPT);
		let bytes = p
			.read_file(&path)
			.map_err(|_| Error::Io(format!("{path} was not found; nothing to remove")))?;
		Receipt::parse(&String::from_utf8_lossy(&bytes))
	}
}

/// Removes what the receipt in `dir` lists: files, then the directories the install made when
/// they are empty, then the receipt. A file that is already gone is not an error. Returns how many
/// files were removed. `progress` returning false stops early (the receipt stays, so a second
/// run finishes the job).
pub fn uninstall<P: Platform>(
	p: &mut P,
	dir: &str,
	progress: &mut dyn FnMut(&mut P, &Progress) -> bool,
) -> Result<u32, Error> {
	let dir = normalize_dir_for(p, dir);
	let r = Receipt::read(p, &dir)?;
	let total = (r.files.len() + r.others.len() + r.shortcuts.len()) as u32;
	let mut pr = Progress {
		files_total: total,
		bytes_total: u64::from(total),
		..Progress::default()
	};
	let mut removed = 0;
	let names = r
		.files
		.iter()
		.map(|f| f.0.as_str())
		.chain(r.others.iter().map(String::as_str));
	let names: Vec<&str> = names.collect();
	for name in names.iter().rev() {
		pr.file = name.to_string();
		let path = join(&dir, name);
		if p.exists(&path) {
			p.remove_file(&path)?;
			removed += 1;
		}
		pr.files_done += 1;
		pr.bytes_done = u64::from(pr.files_done);
		pr.file_pct = 100;
		pr.free = p.free_space(&dir).unwrap_or(0);
		if !progress(p, &pr) {
			return Err(Error::Cancelled);
		}
	}
	// A file the install replaced goes back, last first. The copy is kept when it cannot be put back,
	// so a second run can finish.
	for (original, backup) in r.overwritten.iter().rev() {
		let backup = join(&dir, backup);
		if p.exists(&backup) {
			match p.shell() {
				Some(s) => s.copy_file(&backup, original)?,
				None => return Err(no_folder()),
			}
			p.remove_file(&backup)?;
		}
	}
	if !r.overwritten.is_empty() {
		for sub in [Some(WIN), Some(SYS), None] {
			let _ = p.remove_dir(&join(&dir, &backup_folder(sub)));
		}
	}
	// What only a Windows install makes. A shortcut or the uninstaller that cannot be removed is
	// not worth stopping for.
	for sc in r.shortcuts.iter().rev() {
		if p.exists(sc) {
			let _ = p.remove_file(sc);
		}
		pr.files_done += 1;
		pr.bytes_done = u64::from(pr.files_done);
		pr.file = sc.rsplit('\\').next().unwrap_or(sc).to_string();
		if !progress(p, &pr) {
			return Err(Error::Cancelled);
		}
	}
	for k in &r.keys {
		if let Some(s) = p.shell() {
			s.unregister_uninstall(k);
		}
	}
	remove_groups(p, &r.groups, &r.made_groups);
	unregister_file_types(p, &r.file_types);
	for x in &r.extras {
		let path = join(&dir, x);
		if p.exists(&path) {
			let _ = p.remove_file(&path);
		}
	}
	p.remove_file(&join(&dir, RECEIPT))?;
	// The directories the install made, innermost first; the install directory is listed as `.`.
	for d in r.dirs.iter().rev() {
		let _ = p.remove_dir(&join(&dir, d));
	}
	// The install folder may have been there before the install, so the receipt does not list it.
	// Removing a folder fails while it holds anything, so the person's own files are never touched.
	let _ = p.remove_dir(&dir);
	Ok(removed)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn ini_set_adds_and_replaces() {
		let a = ini_set("", "sound", "card", "adlib");
		assert_eq!(a, "[sound]\r\ncard=adlib\r\n");
		let b = ini_set(&a, "sound", "port", "220");
		assert_eq!(b, "[sound]\r\ncard=adlib\r\nport=220\r\n");
		let c = ini_set(&b, "sound", "CARD", "sbpro");
		assert_eq!(c, "[sound]\r\nCARD=sbpro\r\nport=220\r\n");
		let d = ini_set(&c, "video", "mode", "13");
		assert!(d.ends_with("\r\n\r\n[video]\r\nmode=13\r\n"), "{d:?}");
	}

	#[test]
	fn receipts_round_trip() {
		let text = "LIR 1\r\nproduct=a.b\r\nversion=1.0\r\ntarget=dos-x86:tui-retro\r\ndir=C:\\FIZZ\r\ncomponents=game,sound\r\nD .\r\nD SOUND\r\nF FIZZ.EXE 12 0000ABCD\r\nF SOUND\\A.DRV 5 00000001\r\nI SOUND.INI\r\nB FIZZ.BAT\r\n";
		let r = Receipt::parse(text).unwrap();
		assert_eq!(r.dir, "C:\\FIZZ");
		assert_eq!(r.components, ["game", "sound"]);
		assert_eq!(r.dirs, [".", "SOUND"]);
		assert_eq!(r.files[1], ("SOUND\\A.DRV".to_string(), 5, 1));
		assert_eq!(r.others, ["SOUND.INI", "FIZZ.BAT"]);
		assert!(Receipt::parse("nope").is_err());
	}
}
