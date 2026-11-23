// Payloads: where file contents come from at install time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use li_pack::{Lip, LipBuilder};

use crate::error::{EngineError, Result};
use crate::project::Project;

/// A source of files, addressed by project-relative path (`dist/linux-x86_64/APP`).
pub trait Payload {
	/// Every file, sorted.
	fn list(&self) -> Vec<String>;
	fn read(&self, path: &str) -> Result<Vec<u8>>;
	fn size(&self, path: &str) -> Option<u64>;
	fn executable(&self, path: &str) -> bool;
}

/// Files read from a directory, normally a project's `installer/` directory.
pub struct DirPayload {
	root: PathBuf,
	files: Vec<String>,
}

impl DirPayload {
	pub fn new(root: &Path) -> Result<Self> {
		fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
			let rd = match std::fs::read_dir(dir) {
				Ok(rd) => rd,
				Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
				Err(e) => return Err(EngineError::io(dir, e)),
			};
			for entry in rd {
				let entry = entry.map_err(|e| EngineError::io(dir, e))?;
				let path = entry.path();
				let kind = entry.file_type().map_err(|e| EngineError::io(&path, e))?;
				if kind.is_dir() {
					walk(root, &path, out)?;
				} else if kind.is_file() {
					let rel = path.strip_prefix(root).expect("under root");
					out.push(
						rel.components()
							.map(|c| c.as_os_str().to_string_lossy().into_owned())
							.collect::<Vec<_>>()
							.join("/"),
					);
				}
			}
			Ok(())
		}
		let mut files = Vec::new();
		walk(root, root, &mut files)?;
		files.sort();
		Ok(DirPayload {
			root: root.to_owned(),
			files,
		})
	}
}

impl Payload for DirPayload {
	fn list(&self) -> Vec<String> {
		self.files.clone()
	}

	fn read(&self, path: &str) -> Result<Vec<u8>> {
		if !self.files.iter().any(|f| f == path) {
			return Err(EngineError::MissingSource(path.to_owned()));
		}
		let full = self.root.join(path);
		std::fs::read(&full).map_err(|e| EngineError::io(full, e))
	}

	fn size(&self, path: &str) -> Option<u64> {
		std::fs::metadata(self.root.join(path))
			.ok()
			.map(|m| m.len())
	}

	fn executable(&self, path: &str) -> bool {
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			std::fs::metadata(self.root.join(path))
				.is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
		}
		#[cfg(not(unix))]
		{
			let _ = path;
			false
		}
	}
}

/// Files from a verified `.lip`.
pub struct LipPayload(pub Lip);

impl LipPayload {
	/// Parses `.lip` bytes, checking every entry's hash.
	pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
		Ok(LipPayload(Lip::parse(bytes)?))
	}
}

impl Payload for LipPayload {
	fn list(&self) -> Vec<String> {
		self.0.entries().iter().map(|e| e.path.clone()).collect()
	}

	fn read(&self, path: &str) -> Result<Vec<u8>> {
		self.0
			.read(path)
			.map(<[u8]>::to_vec)
			.ok_or_else(|| EngineError::MissingSource(path.to_owned()))
	}

	fn size(&self, path: &str) -> Option<u64> {
		self.0.entry(path).map(|e| e.size)
	}

	fn executable(&self, path: &str) -> bool {
		self.0.entry(path).is_some_and(|e| e.executable)
	}
}

/// An in-memory payload, handy in tests and for scripts that generate files.
#[derive(Default)]
pub struct MemPayload {
	files: std::collections::BTreeMap<String, (Vec<u8>, bool)>,
}

impl MemPayload {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn insert(&mut self, path: &str, data: &[u8], executable: bool) {
		self.files
			.insert(path.to_owned(), (data.to_vec(), executable));
	}
}

impl Payload for MemPayload {
	fn list(&self) -> Vec<String> {
		self.files.keys().cloned().collect()
	}

	fn read(&self, path: &str) -> Result<Vec<u8>> {
		self.files
			.get(path)
			.map(|(d, _)| d.clone())
			.ok_or_else(|| EngineError::MissingSource(path.to_owned()))
	}

	fn size(&self, path: &str) -> Option<u64> {
		self.files.get(path).map(|(d, _)| d.len() as u64)
	}

	fn executable(&self, path: &str) -> bool {
		self.files.get(path).is_some_and(|(_, e)| *e)
	}
}

fn glob_match(pat: &[char], name: &[char]) -> bool {
	match (pat.first(), name.first()) {
		(None, None) => true,
		(Some('*'), _) => {
			glob_match(&pat[1..], name) || (!name.is_empty() && glob_match(pat, &name[1..]))
		}
		(Some('?'), Some(_)) => glob_match(&pat[1..], &name[1..]),
		(Some(p), Some(n)) if p == n => glob_match(&pat[1..], &name[1..]),
		_ => false,
	}
}

/// Expands a component's `from`: an exact file, a `*`/`?` pattern in the last segment, or a
/// trailing `/` for a whole directory tree. Returns `(source, relative)` pairs, where
/// `relative` is what is appended to a directory destination.
pub fn expand_sources(payload: &dyn Payload, pattern: &str) -> Vec<(String, String)> {
	let files = payload.list();
	let mut out: Vec<(String, String)> = if let Some(prefix) = pattern.strip_suffix('/') {
		let prefix = format!("{prefix}/");
		files
			.into_iter()
			.filter_map(|f| f.strip_prefix(&prefix).map(|r| (f.clone(), r.to_owned())))
			.collect()
	} else if pattern.contains(['*', '?']) {
		let (dir, name_pat) = match pattern.rsplit_once('/') {
			Some((d, n)) => (Some(d), n),
			None => (None, pattern),
		};
		let name_pat: Vec<char> = name_pat.chars().collect();
		files
			.into_iter()
			.filter_map(|f| {
				let (fdir, fname) = match f.rsplit_once('/') {
					Some((d, n)) => (Some(d), n),
					None => (None, f.as_str()),
				};
				let chars: Vec<char> = fname.chars().collect();
				(fdir == dir && glob_match(&name_pat, &chars))
					.then(|| (f.clone(), fname.to_owned()))
			})
			.collect()
	} else {
		files
			.into_iter()
			.filter(|f| f == pattern)
			.map(|f| {
				let base = f.rsplit('/').next().unwrap_or(&f).to_owned();
				(f, base)
			})
			.collect()
	};
	out.sort();
	out
}

/// Builds the `.lip` for one target: every file its applicable components name, read from the
/// project's `installer/` directory.
pub fn pack_payload(project: &Project, target: &str) -> Result<Vec<u8>> {
	let dir = DirPayload::new(&project.dir)?;
	let mut builder = LipBuilder::new();
	let tauri =
		crate::tauri::plan::WindowsTauri::of(project, target, project.installer.defaults.scope);
	for component in &project.components.components {
		if !component.applies_to(target) {
			continue;
		}
		// A wrapped Tauri setup carries the app's files itself.
		if tauri.as_ref().map(|t| t.skips(component)) == Some(crate::tauri::plan::Skip::All) {
			continue;
		}
		for rule in &component.files {
			let pattern = rule.from.replace("{target}", target);
			let found = expand_sources(&dir, &pattern);
			if found.is_empty() {
				return Err(EngineError::MissingSource(pattern));
			}
			for (src, _) in found {
				let data = dir.read(&src)?;
				builder.add(&src, data, dir.executable(&src))?;
			}
		}
	}
	if let Some(t) = &tauri {
		for src in crate::tauri::plan::payload_files(t, &dir)? {
			let data = dir.read(&src)?;
			builder.add(&src, data, dir.executable(&src))?;
		}
	}
	Ok(builder.build()?)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn mem() -> MemPayload {
		let mut p = MemPayload::new();
		for f in [
			"dist/a/APP",
			"dist/a/x.DRV",
			"dist/a/y.DRV",
			"dist/a/sub/z",
			"dist/b/APP",
		] {
			p.insert(f, f.as_bytes(), false);
		}
		p
	}

	#[test]
	fn expands_exact_glob_and_tree() {
		let p = mem();
		assert_eq!(
			expand_sources(&p, "dist/a/APP"),
			vec![("dist/a/APP".into(), "APP".into())]
		);
		let g: Vec<_> = expand_sources(&p, "dist/a/*.DRV")
			.into_iter()
			.map(|x| x.1)
			.collect();
		assert_eq!(g, vec!["x.DRV", "y.DRV"]);
		let t: Vec<_> = expand_sources(&p, "dist/a/")
			.into_iter()
			.map(|x| x.1)
			.collect();
		assert_eq!(t, vec!["APP", "sub/z", "x.DRV", "y.DRV"]);
		assert!(expand_sources(&p, "dist/c/*").is_empty());
	}
}
