// `liminst theme pack`: a reproducible archive of one theme folder.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;

use crate::project::Project;
use crate::util::sha256_hex;
use crate::{CliError, CliResult};

pub struct Packed {
	pub path: PathBuf,
	pub files: usize,
	pub sha256: String,
	/// Where the detached signature went, when a key was given.
	pub signature: Option<PathBuf>,
}

fn collect(dir: &Path, base: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
	let mut entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
	entries.sort_by_key(|e| e.file_name());
	for e in entries {
		let p = e.path();
		if p.is_dir() {
			collect(&p, base, out)?;
		} else {
			out.push(p.strip_prefix(base).unwrap_or(&p).to_path_buf());
		}
	}
	Ok(())
}

/// Packs `installer/themes/<name>/` into `out` (default `<id>.litheme` beside the project).
/// The theme must have loaded cleanly. The archive is a gzip tar with sorted entries, zero
/// times and root ownership, so the same theme always gives the same bytes. With `sign_key` the
/// archive's bytes are signed with Ed25519 and the hex signature goes to `<archive>.sig`.
pub fn pack(
	project: &Project,
	name: &str,
	out: Option<&Path>,
	sign_key: Option<&Path>,
) -> CliResult<Packed> {
	let found = project.themes.values().find(|t| {
		t.file.theme.id == name
			|| t.dir
				.file_name()
				.is_some_and(|n| n.to_string_lossy() == name)
	});
	let theme = found.ok_or_else(|| {
		let known: Vec<&str> = project.themes.keys().map(String::as_str).collect();
		CliError::new(format!(
			"no theme named `{name}` (the project has: {})",
			if known.is_empty() {
				"none".to_owned()
			} else {
				known.join(", ")
			}
		))
	})?;
	let tdir = theme.dir.clone();
	let mut rels = Vec::new();
	collect(&tdir, &tdir, &mut rels)
		.map_err(|e| CliError::new(format!("cannot read {}: {e}", tdir.display())))?;
	let mut builder = tar::Builder::new(Vec::new());
	for rel in &rels {
		let data = std::fs::read(tdir.join(rel))
			.map_err(|e| CliError::new(format!("cannot read {}: {e}", rel.display())))?;
		let mut h = tar::Header::new_gnu();
		h.set_size(data.len() as u64);
		h.set_mode(0o644);
		h.set_uid(0);
		h.set_gid(0);
		h.set_mtime(0);
		builder
			.append_data(&mut h, rel, data.as_slice())
			.map_err(|e| CliError::new(e.to_string()))?;
	}
	let tar_bytes = builder
		.into_inner()
		.map_err(|e| CliError::new(e.to_string()))?;
	let mut gz = GzEncoder::new(Vec::new(), Compression::new(9));
	std::io::Write::write_all(&mut gz, &tar_bytes).map_err(|e| CliError::new(e.to_string()))?;
	let bytes = gz.finish().map_err(|e| CliError::new(e.to_string()))?;
	let path = out.map(Path::to_path_buf).unwrap_or_else(|| {
		project
			.root
			.join(format!("{}.litheme", theme.file.theme.id))
	});
	crate::util::write_file_bytes(&path, &bytes)?;
	let signature = match sign_key {
		Some(key_path) => {
			let key = li_pack::read_signing_key(key_path)
				.map_err(|e| CliError::new(format!("{}: {e}", key_path.display())))?;
			let mut sig = path.as_os_str().to_owned();
			sig.push(".sig");
			let sig = PathBuf::from(sig);
			crate::util::write_file(&sig, &format!("{}\n", li_pack::sign(&key, &bytes)))?;
			Some(sig)
		}
		None => None,
	};
	Ok(Packed {
		path,
		files: rels.len(),
		sha256: sha256_hex(&bytes),
		signature,
	})
}
