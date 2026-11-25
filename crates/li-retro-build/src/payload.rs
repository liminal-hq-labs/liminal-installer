// The files of a target: 8.3 destinations, compression, the payload and the shortcut launchers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_engine::{DirPayload, Payload, Project, expand_sources};
use li_model::{Component, Target};
use li_retro::model::{FileEntry, Op};
use li_retro::pack::{PROJECT_FILE, crc32};

use crate::{CompileError, Flavour, err};

fn dos_name_ok(part: &str) -> bool {
	let ok = |s: &str, max: usize| {
		!s.is_empty()
			&& s.len() <= max
			&& s.bytes()
				.all(|b| b.is_ascii_alphanumeric() || b"_-!#$%&'()@^{}~".contains(&b))
	};
	match part.split_once('.') {
		Some((name, ext)) => ok(name, 8) && ok(ext, 3),
		None => ok(part, 8),
	}
}

/// Turns `{install}/SOUND/ADLIB.DRV` into `SOUND\ADLIB.DRV`: upper case, and every part 8.3.
pub fn dos_rel(to: &str) -> Result<String, CompileError> {
	let Some(rest) = to.strip_prefix("{install}/") else {
		return err(format!(
			"`{to}` is not below `{{install}}`; the DOS runtime installs into one folder"
		));
	};
	let mut parts = Vec::new();
	for part in rest.split('/').filter(|p| !p.is_empty()) {
		let up = part.to_ascii_uppercase();
		if !dos_name_ok(&up) {
			return err(format!(
				"`{part}` in `{to}` is not an 8.3 name; DOS needs at most 8 characters, a dot and 3 more"
			));
		}
		parts.push(up);
	}
	if parts.is_empty() {
		return err(format!("`{to}` names no file"));
	}
	Ok(parts.join("\\"))
}

/// Turns `{install}/docs/Read Me.txt` into `docs\Read Me.txt`: the case kept, parts that Windows
/// accepts (no reserved characters, no empty or dotted-only part).
pub fn win_rel(to: &str) -> Result<String, CompileError> {
	let Some(rest) = to.strip_prefix("{install}/") else {
		return err(format!(
			"`{to}` is not below `{{install}}`; the Windows runtime installs into one folder"
		));
	};
	let mut parts = Vec::new();
	for part in rest.split('/').filter(|p| !p.is_empty()) {
		if part
			.chars()
			.any(|c| "\\:*?\"<>|".contains(c) || c.is_control())
			|| part.trim_matches('.').is_empty()
			|| part.len() > 255
		{
			return err(format!("`{part}` in `{to}` is not a name Windows accepts"));
		}
		parts.push(part.to_owned());
	}
	if parts.is_empty() {
		return err(format!("`{to}` names no file"));
	}
	Ok(parts.join("\\"))
}

/// The files, their contents in the same order, the operations and the component each belongs to.
pub type Built = (Vec<FileEntry>, Vec<Vec<u8>>, Vec<Op>, Vec<u16>);

/// Builds the file list, the files' contents and the launcher operations. The pack compresses
/// them, a chunk at a time, when it is written.
pub fn build(
	project: &Project,
	target: &Target,
	components: &[&Component],
	flavour: Flavour,
	warnings: &mut Vec<String>,
) -> Result<Built, CompileError> {
	let rel_of = |to: &str| match flavour {
		Flavour::Dos | Flavour::Win16 => dos_rel(to),
		Flavour::Win32 => win_rel(to),
	};
	let source = DirPayload::new(&project.dir).map_err(|e| CompileError::Message(e.to_string()))?;
	let mut files: Vec<(FileEntry, Vec<u8>)> = Vec::new();
	let mut ops: Vec<Op> = Vec::new();
	let mut owners: Vec<u16> = Vec::new();
	let mut batches: Vec<String> = Vec::new();
	for (ci, c) in components.iter().enumerate() {
		for rule in &c.files {
			let pattern = rule.from.replace("{target}", &target.id);
			let found = expand_sources(&source, &pattern);
			if found.is_empty() {
				return err(format!("component `{}`: no file matches `{pattern}`", c.id));
			}
			for (src, rel) in found {
				let to = if rule.to.ends_with('/') {
					format!("{}{}", rule.to, rel)
				} else if pattern.contains(['*', '?']) || pattern.ends_with('/') {
					format!("{}/{}", rule.to, rel)
				} else {
					rule.to.clone()
				};
				let dest = rel_of(&to)
					.map_err(|e| CompileError::Message(format!("component `{}`: {e}", c.id)))?;
				if dest.eq_ignore_ascii_case(PROJECT_FILE) {
					return err(format!(
						"component `{}`: `{dest}` is reserved for the compiled project",
						c.id
					));
				}
				if files
					.iter()
					.any(|(f, _)| f.dest.eq_ignore_ascii_case(&dest))
				{
					return err(format!("component `{}`: `{dest}` is installed twice", c.id));
				}
				let data = source
					.read(&src)
					.map_err(|e| CompileError::Message(e.to_string()))?;
				let size = u32::try_from(data.len())
					.map_err(|_| CompileError::Message(format!("`{dest}` is larger than 4 GiB")))?;
				files.push((
					FileEntry {
						component: ci as u16,
						stored: String::new(),
						dest,
						size,
						crc: crc32(&data),
					},
					data,
				));
			}
		}
		for s in &c.shortcut {
			let rel = rel_of(&s.target).map_err(|e| {
				CompileError::Message(format!("component `{}` shortcut `{}`: {e}", c.id, s.name))
			})?;
			if flavour == Flavour::Win32 {
				ops.push(Op::Shortcut {
					name: s.name.clone(),
					target: rel,
					batch: String::new(),
				});
				continue;
			}
			let stem = rel
				.rsplit('\\')
				.next()
				.unwrap_or(&rel)
				.split('.')
				.next()
				.unwrap_or("RUN")
				.to_owned();
			let mut batch = format!("{stem}.BAT");
			let mut n = 1;
			while batches.contains(&batch) || rel.eq_ignore_ascii_case(&batch) {
				batch = format!("{}{n}.BAT", &stem[..stem.len().min(7)]);
				n += 1;
			}
			batches.push(batch.clone());
			ops.push(Op::Shortcut {
				name: s.name.clone(),
				target: rel,
				batch,
			});
		}
		for ft in &c.file_types {
			if flavour == Flavour::Dos {
				warnings.push(format!(
					"component `{}`: the file type `{}` has no {} equivalent and is skipped",
					c.id,
					ft.extension,
					flavour.name()
				));
				continue;
			}
			match file_type_op(project, c, ft, flavour, &rel_of, &files) {
				Ok(op) => ops.push(op),
				Err(why) => warnings.push(format!(
					"component `{}`: the file type `{}` is skipped: {why}",
					c.id, ft.extension
				)),
			}
		}
		if !c.url_schemes.is_empty() || !c.path.is_empty() {
			warnings.push(format!(
				"component `{}`: URL schemes and PATH entries have no {} equivalent and are skipped",
				c.id,
				flavour.name()
			));
		}
		// Every operation this component added belongs to it.
		owners.resize(ops.len(), ci as u16);
	}
	files.sort_by(|a, b| a.0.dest.cmp(&b.0.dest));
	// The names the pack stores them under: the destination itself on DOS, `F0000001` and on for a
	// Windows name, which the pack keeps as the long name.
	for (i, (f, _)) in files.iter_mut().enumerate() {
		f.stored = match flavour {
			Flavour::Dos | Flavour::Win16 => f.dest.replace('\\', "/"),
			Flavour::Win32 => format!("F{:07X}", i + 1),
		};
	}
	let (entries, data) = files.into_iter().unzip();
	Ok((entries, data, ops, owners))
}

/// The registered name of a file type: the product id and the extension, so two products never
/// share one.
fn prog_id(product: &str, ext: &str) -> String {
	format!("{product}.{ext}")
}

/// The operation that registers a component's file type, or why the runtime cannot. The program
/// that opens it is `open_with`, else the component's first shortcut.
fn file_type_op(
	project: &Project,
	c: &Component,
	ft: &li_model::project::FileTypeRule,
	flavour: Flavour,
	rel_of: &dyn Fn(&str) -> Result<String, CompileError>,
	files: &[(FileEntry, Vec<u8>)],
) -> Result<Op, String> {
	let ext = ft.extension.trim_start_matches('.').to_ascii_lowercase();
	// Windows 3.x associates three-character extensions only.
	let longest = if flavour == Flavour::Win16 { 3 } else { 64 };
	if ext.is_empty()
		|| ext.len() > longest
		|| !ext
			.chars()
			.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
	{
		return Err(if flavour == Flavour::Win16 {
			"Windows 3.1 takes one to three letters or digits".to_string()
		} else {
			"it is not a plain extension".to_string()
		});
	}
	let open = ft
		.open_with
		.as_deref()
		.or_else(|| c.shortcut.first().map(|s| s.target.as_str()))
		.ok_or_else(|| "no `open_with` and no shortcut names the program".to_string())?;
	let mut target = rel_of(open).map_err(|e| e.to_string())?;
	// A component shared with other systems names its program without `.exe`; Windows needs it.
	let bare = !target.rsplit('\\').next().unwrap_or("").contains('.');
	if flavour == Flavour::Win32 && bare {
		let exe = format!("{target}.exe");
		if let Some((f, _)) = files
			.iter()
			.find(|(f, _)| f.dest.eq_ignore_ascii_case(&exe))
		{
			target = f.dest.clone();
		}
	}
	Ok(Op::FileType {
		prog_id: prog_id(&project.installer.product.id, &ext),
		ext,
		name: ft.name.clone(),
		target,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn makes_8_3_destinations() {
		assert_eq!(
			dos_rel("{install}/sound/adlib.drv").unwrap(),
			"SOUND\\ADLIB.DRV"
		);
		assert_eq!(dos_rel("{install}/FIZZ.EXE").unwrap(), "FIZZ.EXE");
		assert!(dos_rel("{install}/LONGNAME9.EXE").is_err());
		assert!(dos_rel("{install}/A.TOOL").is_err());
		assert!(dos_rel("{user_data}/A.EXE").is_err());
		assert!(dos_rel("{install}/").is_err());
	}
}
