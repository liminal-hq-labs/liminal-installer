// The `.lip` payload container: a manifest of path and SHA-256, then the bytes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{PackError, sha256_hex};

const MAGIC: &[u8; 4] = b"LIP1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LipEntry {
	pub path: String,
	pub size: u64,
	pub sha256: String,
	#[serde(default)]
	pub executable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
	version: u32,
	entries: Vec<LipEntry>,
}

fn check_path(path: &str) -> Result<(), PackError> {
	let bad = path.is_empty()
		|| path.starts_with('/')
		|| path.contains('\\')
		|| path
			.split('/')
			.any(|s| s.is_empty() || s == "." || s == "..");
	if bad {
		Err(PackError::BadPath(path.to_owned()))
	} else {
		Ok(())
	}
}

/// Collects entries and writes them deterministically.
#[derive(Debug, Default)]
pub struct LipBuilder {
	files: BTreeMap<String, (Vec<u8>, bool)>,
}

impl LipBuilder {
	pub fn new() -> Self {
		Self::default()
	}

	/// Adds or replaces an entry.
	pub fn add(&mut self, path: &str, data: Vec<u8>, executable: bool) -> Result<(), PackError> {
		check_path(path)?;
		self.files.insert(path.to_owned(), (data, executable));
		Ok(())
	}

	/// Adds every file under `root`, with paths relative to it. Symbolic links are skipped.
	pub fn add_dir(&mut self, root: &Path) -> Result<(), PackError> {
		fn walk(b: &mut LipBuilder, root: &Path, dir: &Path) -> Result<(), PackError> {
			for entry in std::fs::read_dir(dir)? {
				let entry = entry?;
				let path = entry.path();
				let kind = entry.file_type()?;
				if kind.is_dir() {
					walk(b, root, &path)?;
				} else if kind.is_file() {
					let rel = path
						.strip_prefix(root)
						.map_err(|_| PackError::BadPath(path.display().to_string()))?;
					let rel = rel
						.components()
						.map(|c| c.as_os_str().to_string_lossy().into_owned())
						.collect::<Vec<_>>()
						.join("/");
					#[cfg(unix)]
					let exec = {
						use std::os::unix::fs::PermissionsExt;
						entry.metadata()?.permissions().mode() & 0o111 != 0
					};
					#[cfg(not(unix))]
					let exec = false;
					b.add(&rel, std::fs::read(&path)?, exec)?;
				}
			}
			Ok(())
		}
		walk(self, root, root)
	}

	/// Serializes the container. The same entries always give the same bytes.
	pub fn build(&self) -> Result<Vec<u8>, PackError> {
		let entries = self
			.files
			.iter()
			.map(|(path, (data, executable))| LipEntry {
				path: path.clone(),
				size: data.len() as u64,
				sha256: sha256_hex(data),
				executable: *executable,
			})
			.collect();
		let manifest = serde_json::to_vec(&Manifest {
			version: 1,
			entries,
		})?;
		let mut out = Vec::with_capacity(8 + manifest.len());
		out.extend_from_slice(MAGIC);
		out.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
		out.extend_from_slice(&manifest);
		for (data, _) in self.files.values() {
			out.extend_from_slice(data);
		}
		Ok(out)
	}
}

/// An opened, fully verified `.lip` file.
#[derive(Debug, Clone)]
pub struct Lip {
	entries: Vec<LipEntry>,
	offsets: Vec<usize>,
	data: Vec<u8>,
}

impl Lip {
	/// Parses `bytes` and checks every entry's size and SHA-256.
	pub fn parse(bytes: &[u8]) -> Result<Lip, PackError> {
		if bytes.len() < 8 || &bytes[..4] != MAGIC {
			return Err(PackError::BadFormat("missing LIP1 magic".into()));
		}
		let len = u32::from_le_bytes(bytes[4..8].try_into().expect("four bytes")) as usize;
		let body = 8usize
			.checked_add(len)
			.filter(|end| *end <= bytes.len())
			.ok_or_else(|| PackError::BadFormat("manifest runs past the end".into()))?;
		let manifest: Manifest = serde_json::from_slice(&bytes[8..body])?;
		if manifest.version != 1 {
			return Err(PackError::BadFormat(format!(
				"unsupported manifest version {}",
				manifest.version
			)));
		}
		let data = bytes[body..].to_vec();
		let mut offsets = Vec::new();
		let mut at = 0usize;
		for entry in &manifest.entries {
			check_path(&entry.path)?;
			let size = entry.size as usize;
			let end = at
				.checked_add(size)
				.filter(|end| *end <= data.len())
				.ok_or_else(|| {
					PackError::BadFormat(format!("`{}` runs past the end", entry.path))
				})?;
			let actual = sha256_hex(&data[at..end]);
			if actual != entry.sha256 {
				return Err(PackError::HashMismatch {
					what: entry.path.clone(),
					expected: entry.sha256.clone(),
					actual,
				});
			}
			offsets.push(at);
			at = end;
		}
		if at != data.len() {
			return Err(PackError::BadFormat(
				"trailing bytes after the last entry".into(),
			));
		}
		Ok(Lip {
			entries: manifest.entries,
			offsets,
			data,
		})
	}

	pub fn entries(&self) -> &[LipEntry] {
		&self.entries
	}

	pub fn entry(&self, path: &str) -> Option<&LipEntry> {
		self.entries.iter().find(|e| e.path == path)
	}

	pub fn read(&self, path: &str) -> Option<&[u8]> {
		let i = self.entries.iter().position(|e| e.path == path)?;
		let at = self.offsets[i];
		Some(&self.data[at..at + self.entries[i].size as usize])
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sample() -> Vec<u8> {
		let mut b = LipBuilder::new();
		b.add("b/two.txt", b"two".to_vec(), false).unwrap();
		b.add("a/one.bin", b"one".to_vec(), true).unwrap();
		b.build().unwrap()
	}

	#[test]
	fn round_trips_and_is_deterministic() {
		let bytes = sample();
		let mut other = LipBuilder::new();
		other.add("a/one.bin", b"one".to_vec(), true).unwrap();
		other.add("b/two.txt", b"two".to_vec(), false).unwrap();
		assert_eq!(bytes, other.build().unwrap());
		let lip = Lip::parse(&bytes).unwrap();
		assert_eq!(lip.entries()[0].path, "a/one.bin");
		assert!(lip.entry("a/one.bin").unwrap().executable);
		assert_eq!(lip.read("b/two.txt").unwrap(), b"two");
		assert!(lip.read("nope").is_none());
	}

	#[test]
	fn corrupt_data_is_refused() {
		let mut bytes = sample();
		let last = bytes.len() - 1;
		bytes[last] ^= 1;
		assert!(matches!(
			Lip::parse(&bytes),
			Err(PackError::HashMismatch { .. })
		));
		assert!(matches!(
			Lip::parse(b"NOPE0000"),
			Err(PackError::BadFormat(_))
		));
	}

	#[test]
	fn bad_paths_are_refused() {
		let mut b = LipBuilder::new();
		for p in ["", "/abs", "a/../b", "a//b", "a\\b"] {
			assert!(b.add(p, vec![], false).is_err(), "{p}");
		}
	}

	#[test]
	fn add_dir_uses_relative_paths() {
		let dir = tempfile::tempdir().unwrap();
		std::fs::create_dir(dir.path().join("sub")).unwrap();
		std::fs::write(dir.path().join("sub/x"), b"x").unwrap();
		let mut b = LipBuilder::new();
		b.add_dir(dir.path()).unwrap();
		let lip = Lip::parse(&b.build().unwrap()).unwrap();
		assert_eq!(lip.read("sub/x").unwrap(), b"x");
	}
}
