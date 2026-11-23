// Install and uninstall: copying the payload, the plan operations, rollback and `INSTALL.LIR`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The installer engine for DOS. An install makes the install directory, extracts each selected
//! file straight from `SETUP.LPK` to disk (checking its CRC-32 as it goes), performs the plan
//! operations and writes the receipt `INSTALL.LIR` last. It keeps no journal: it remembers what
//! it created in memory, and a cancel or a failure removes exactly that. A file that was already
//! there is overwritten and, if the install is rolled back, left as the new copy.
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
//! `D` is a directory the install created (`.` is the install directory itself), `F` a file with
//! its size and CRC-32 in hexadecimal, `I` an INI file the install wrote and `B` a launcher batch
//! file. Uninstall removes `F`, `I` and `B` files, then the `D` directories that are empty, in
//! reverse order, and last the receipt.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::Error;
use crate::model::{FileEntry, Op, Project};
use crate::pack::{Crc, Pack};
use crate::platform::Platform;
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
	} else if rel.contains(':') {
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

/// [`normalize_dir`] for a machine that keeps the case of long names (`native_shell`): the same
/// cleaning without the upper-casing.
pub fn normalize_dir_for<P: Platform>(p: &P, dir: &str) -> String {
	if p.native_shell() {
		let d = dir.trim().replace('/', "\\");
		d.trim_end_matches('\\').to_string()
	} else {
		normalize_dir(dir)
	}
}

/// The files a set of components installs, in the order the receipt lists them.
pub fn selected_files<'p>(project: &'p Project, selected: &[String]) -> Vec<&'p FileEntry> {
	let mut files: Vec<&FileEntry> = project
		.files
		.iter()
		.filter(|f| {
			let c = &project.components[usize::from(f.component)];
			c.required || selected.contains(&c.id)
		})
		.collect();
	files.sort_by(|a, b| a.dest.cmp(&b.dest));
	files
}

/// The bytes a set of components needs on disk, with a little room for directory entries.
pub fn bytes_needed(project: &Project, selected: &[String]) -> u64 {
	let files = selected_files(project, selected);
	files.iter().map(|f| u64::from(f.size)).sum::<u64>() + files.len() as u64 * 512
}

struct Journal {
	files: Vec<String>,
	dirs: Vec<String>,
}

fn rollback<P: Platform>(p: &mut P, j: &Journal) {
	for f in j.files.iter().rev() {
		let _ = p.remove_file(f);
	}
	for d in j.dirs.iter().rev() {
		let _ = p.remove_dir(d);
	}
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

/// A shortcut on the Start menu, in a folder named for the product, written through the shell.
fn shell_shortcut<P: Platform>(
	p: &mut P,
	project: &Project,
	dir: &str,
	name: &str,
	target: &str,
	j: &mut Journal,
	made: &mut Made,
) -> Result<(), Error> {
	let Some(programs) = p.start_menu_programs() else {
		return Ok(());
	};
	let folder = format!("{programs}\\{}", file_safe(&project.name));
	if p.mkdir(&folder)? {
		j.dirs.push(folder.clone());
	}
	let lnk = format!("{folder}\\{}.lnk", file_safe(name));
	let existed = p.exists(&lnk);
	p.make_shortcut(&lnk, &join(dir, target), dir, name)?;
	if !existed {
		j.files.push(lnk.clone());
	}
	made.shortcuts.push(lnk);
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
	};
	let mut made = Made {
		files: Vec::new(),
		inis: Vec::new(),
		batches: Vec::new(),
		shortcuts: Vec::new(),
		extras: Vec::new(),
		keys: Vec::new(),
	};
	let result = do_install(
		p, project, pack, &dir, selected, v, &files, free, &mut j, &mut made, progress,
	);
	if result.is_err() {
		rollback(p, &j);
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
		let to = join(dir, &f.dest);
		if let Some((parent, _)) = f.dest.rsplit_once('\\') {
			make_dirs(p, &join(dir, parent), j)?;
		}
		pr.file = f.dest.clone();
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
		made.files.push((f.dest.clone(), f.size, f.crc));
		if !progress(p, &pr) {
			return Err(Error::Cancelled);
		}
	}
	for op in &project.ops {
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
				if p.native_shell() {
					shell_shortcut(p, project, dir, name, target, j, made)?;
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
		}
	}
	if p.native_shell() {
		let bytes = project.encode()?;
		for name in p.install_uninstaller(dir, &bytes)? {
			j.files.push(join(dir, &name));
			made.extras.push(name);
		}
		let uninstall = format!("\"{}\" /U \"{dir}\"", join(dir, "UNINST.EXE"));
		let key = project.id.clone();
		p.register_uninstall(
			&key,
			&[
				("DisplayName", &project.name),
				("DisplayVersion", &project.version),
				("Publisher", &project.publisher),
				("InstallLocation", dir),
				("UninstallString", &uninstall),
			],
		)?;
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
	let receipt = join(dir, RECEIPT);
	write_text(p, &receipt, &lir)?;
	j.files.push(receipt);
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
	/// The uninstaller's own files, below the install folder.
	pub extras: Vec<String>,
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
		p.unregister_uninstall(k);
	}
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
