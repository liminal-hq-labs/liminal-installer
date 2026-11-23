// Loads an example `installer/` project into li-model types for the consistency tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use li_model::{ComponentsFile, Flow, InstallerFile, Layout, ThemeFile};

/// Every example project, relative to the repository root.
pub const EXAMPLES: &[&str] = &["planboard", "lt-fizz", "neutral", "tauri-notes"];

/// An example project read from disk.
pub struct Example {
	pub name: String,
	pub dir: PathBuf,
	pub installer: InstallerFile,
	pub components: ComponentsFile,
	pub flows: BTreeMap<String, Flow>,
	pub layouts: BTreeMap<String, Layout>,
	pub themes: BTreeMap<String, ThemeFile>,
}

/// The `examples/` directory of the repository.
pub fn examples_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn read(path: &Path) -> String {
	fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn files_in(dir: &Path, suffix: &str) -> Vec<PathBuf> {
	let mut out: Vec<PathBuf> = fs::read_dir(dir)
		.unwrap_or_else(|e| panic!("listing {}: {e}", dir.display()))
		.map(|e| e.expect("directory entry").path())
		.filter(|p| {
			p.file_name()
				.and_then(|n| n.to_str())
				.is_some_and(|n| n.ends_with(suffix))
		})
		.collect();
	out.sort();
	out
}

impl Example {
	pub fn load(name: &str) -> Example {
		let dir = examples_root().join(name).join("installer");
		let installer = toml::from_str(&read(&dir.join("installer.toml")))
			.unwrap_or_else(|e| panic!("{name}: installer.toml: {e}"));
		let components = toml::from_str(&read(&dir.join("components.toml")))
			.unwrap_or_else(|e| panic!("{name}: components.toml: {e}"));
		let mut flows = BTreeMap::new();
		for path in files_in(&dir.join("flows"), ".flow.json") {
			let flow: Flow = serde_json::from_str(&read(&path))
				.unwrap_or_else(|e| panic!("{}: {e}", path.display()));
			flows.insert(flow.id.clone(), flow);
		}
		let mut layouts = BTreeMap::new();
		for path in files_in(&dir.join("layouts"), ".layout.json") {
			let layout: Layout = serde_json::from_str(&read(&path))
				.unwrap_or_else(|e| panic!("{}: {e}", path.display()));
			layouts.insert(layout.id.clone(), layout);
		}
		let mut themes = BTreeMap::new();
		for entry in fs::read_dir(dir.join("themes")).expect("themes directory") {
			let theme_dir = entry.expect("theme entry").path();
			let file: ThemeFile = toml::from_str(&read(&theme_dir.join("theme.toml")))
				.unwrap_or_else(|e| panic!("{}: theme.toml: {e}", theme_dir.display()));
			themes.insert(file.theme.id.clone(), file);
		}
		Example {
			name: name.to_owned(),
			dir,
			installer,
			components,
			flows,
			layouts,
			themes,
		}
	}

	/// The files a `dist/` rule matches for a target, supporting `*` in the file name.
	pub fn expand_source(&self, from: &str, target_id: &str) -> Vec<PathBuf> {
		let pattern = from.replace("{target}", target_id);
		let path = self.dir.join(&pattern);
		let (Some(parent), Some(file)) = (path.parent(), path.file_name().and_then(|n| n.to_str()))
		else {
			return Vec::new();
		};
		let Ok(entries) = fs::read_dir(parent) else {
			return Vec::new();
		};
		let mut out: Vec<PathBuf> = entries
			.filter_map(Result::ok)
			.map(|e| e.path())
			.filter(|p| {
				p.file_name()
					.and_then(|n| n.to_str())
					.is_some_and(|n| wildcard(file, n))
			})
			.collect();
		out.sort();
		out
	}
}

/// Matches `name` against a pattern whose only metacharacter is `*`.
pub fn wildcard(pattern: &str, name: &str) -> bool {
	let parts: Vec<&str> = pattern.split('*').collect();
	if parts.len() == 1 {
		return pattern == name;
	}
	let (first, last) = (parts[0], parts[parts.len() - 1]);
	if name.len() < first.len() + last.len() || !name.starts_with(first) || !name.ends_with(last) {
		return false;
	}
	let mut rest = &name[first.len()..name.len() - last.len()];
	for mid in &parts[1..parts.len() - 1] {
		match rest.find(mid) {
			Some(i) => rest = &rest[i + mid.len()..],
			None => return false,
		}
	}
	true
}

#[cfg(test)]
mod tests {
	use super::wildcard;

	#[test]
	fn wildcard_matches() {
		assert!(wildcard("FIZZ*", "FIZZ.EXE"));
		assert!(wildcard("*.DRV", "SBPRO.DRV"));
		assert!(wildcard("a*b*c", "aXXbYYc"));
		assert!(!wildcard("*.DRV", "SBPRO.EXE"));
		assert!(wildcard("exact", "exact"));
		assert!(!wildcard("exact", "exact2"));
	}
}
