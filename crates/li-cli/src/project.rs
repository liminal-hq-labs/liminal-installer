// Loading an installer project from disk.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use li_model::{ComponentsFile, Flow, InstallerFile, Layout, Target, ThemeFile};

use crate::diag::Diagnostic;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
	#[error(
		"no installer project found at {0} (expected installer/installer.toml; run `liminst init`)"
	)]
	NotFound(PathBuf),
	#[error("cannot read {path}: {message}")]
	Io { path: PathBuf, message: String },
	/// A file the project cannot work without did not parse.
	#[error("{}", .0.message)]
	Invalid(Diagnostic),
}

#[derive(Debug, Clone)]
pub struct LoadedTheme {
	pub dir_name: String,
	pub file: ThemeFile,
	/// Layouts by file stem (`welcome` for `welcome.layout.json`).
	pub layouts: BTreeMap<String, Layout>,
}

/// A loaded `installer/` folder.
#[derive(Debug, Clone)]
pub struct Project {
	/// The folder that holds `installer/`.
	pub root: PathBuf,
	/// The `installer/` folder; every path in the project is relative to it.
	pub dir: PathBuf,
	pub installer: InstallerFile,
	pub components: ComponentsFile,
	/// Flows by file stem (`install` for `install.flow.json`).
	pub flows: BTreeMap<String, Flow>,
	pub themes: BTreeMap<String, LoadedTheme>,
	/// Files that exist but did not parse. They are not loaded; `check` reports them.
	pub problems: Vec<Diagnostic>,
}

fn rel(root: &Path, p: &Path) -> String {
	p.strip_prefix(root)
		.unwrap_or(p)
		.to_string_lossy()
		.replace('\\', "/")
}

fn read(path: &Path) -> Result<String, LoadError> {
	std::fs::read_to_string(path).map_err(|e| LoadError::Io {
		path: path.to_path_buf(),
		message: e.to_string(),
	})
}

fn list(dir: &Path, suffix: &str) -> Vec<PathBuf> {
	let Ok(rd) = std::fs::read_dir(dir) else {
		return Vec::new();
	};
	let mut v: Vec<PathBuf> = rd
		.filter_map(|e| e.ok())
		.map(|e| e.path())
		.filter(|p| {
			p.is_file()
				&& p.file_name()
					.is_some_and(|n| n.to_string_lossy().ends_with(suffix))
		})
		.collect();
	v.sort();
	v
}

impl Project {
	/// Loads the project under `root`. `root` may be the folder that holds `installer/`, or
	/// `installer/` itself.
	pub fn load(root: &Path) -> Result<Project, LoadError> {
		let (root, dir) = if root.join("installer/installer.toml").is_file() {
			(root.to_path_buf(), root.join("installer"))
		} else if root.join("installer.toml").is_file() {
			let parent = root
				.canonicalize()
				.ok()
				.and_then(|p| p.parent().map(Path::to_path_buf))
				.unwrap_or_else(|| root.to_path_buf());
			(parent, root.to_path_buf())
		} else {
			return Err(LoadError::NotFound(root.to_path_buf()));
		};
		let installer_path = dir.join("installer.toml");
		let installer: InstallerFile = toml::from_str(&read(&installer_path)?).map_err(|e| {
			LoadError::Invalid(
				Diagnostic::error("invalid-installer", e.message().to_owned())
					.in_file(rel(&root, &installer_path)),
			)
		})?;
		let components_path = dir.join("components.toml");
		let components: ComponentsFile = if components_path.is_file() {
			toml::from_str(&read(&components_path)?).map_err(|e| {
				LoadError::Invalid(
					Diagnostic::error("invalid-components", e.message().to_owned())
						.in_file(rel(&root, &components_path)),
				)
			})?
		} else {
			ComponentsFile::default()
		};
		let mut problems = Vec::new();
		let mut flows = BTreeMap::new();
		for path in list(&dir.join("flows"), ".flow.json") {
			let stem = file_stem(&path, ".flow.json");
			match serde_json::from_str::<Flow>(&read(&path)?) {
				Ok(f) => {
					flows.insert(stem, f);
				}
				Err(e) => problems.push(
					Diagnostic::error("invalid-flow", e.to_string()).in_file(rel(&root, &path)),
				),
			}
		}
		let mut themes = BTreeMap::new();
		if let Ok(rd) = std::fs::read_dir(dir.join("themes")) {
			let mut dirs: Vec<PathBuf> = rd
				.filter_map(|e| e.ok())
				.map(|e| e.path())
				.filter(|p| p.is_dir())
				.collect();
			dirs.sort();
			for tdir in dirs {
				let name = tdir
					.file_name()
					.map(|n| n.to_string_lossy().into_owned())
					.unwrap_or_default();
				let tfile = tdir.join("theme.toml");
				if !tfile.is_file() {
					problems.push(
						Diagnostic::error("missing-theme-file", "theme folder has no theme.toml")
							.in_file(rel(&root, &tdir)),
					);
					continue;
				}
				let file: ThemeFile = match toml::from_str(&read(&tfile)?) {
					Ok(f) => f,
					Err(e) => {
						problems.push(
							Diagnostic::error("invalid-theme", e.message().to_owned())
								.in_file(rel(&root, &tfile)),
						);
						continue;
					}
				};
				let mut layouts = BTreeMap::new();
				for lp in list(&tdir.join("layouts"), ".layout.json") {
					match serde_json::from_str::<Layout>(&read(&lp)?) {
						Ok(l) => {
							layouts.insert(file_stem(&lp, ".layout.json"), l);
						}
						Err(e) => problems.push(
							Diagnostic::error("invalid-layout", e.to_string())
								.in_file(rel(&root, &lp)),
						),
					}
				}
				themes.insert(
					name.clone(),
					LoadedTheme {
						dir_name: name,
						file,
						layouts,
					},
				);
			}
		}
		Ok(Project {
			root,
			dir,
			installer,
			components,
			flows,
			themes,
			problems,
		})
	}

	pub fn target(&self, id: &str) -> Option<&Target> {
		self.installer.targets.iter().find(|t| t.id == id)
	}

	/// The theme a target uses: its own, else the project default.
	pub fn theme_name_for<'a>(&'a self, target: &'a Target) -> Option<&'a str> {
		target
			.theme
			.as_deref()
			.or(self.installer.defaults.theme.as_deref())
	}

	pub fn theme_for(&self, target: &Target) -> Option<&LoadedTheme> {
		self.theme_name_for(target).and_then(|n| self.themes.get(n))
	}

	/// The layout a step uses on `target`: from its theme, else from the theme's fallback.
	pub fn layout_for(&self, target: &Target, layout_id: &str) -> Option<&Layout> {
		let theme = self.theme_for(target)?;
		theme.layouts.get(layout_id).or_else(|| {
			let fb = theme.file.theme.fallback.as_deref()?;
			self.themes.get(fb)?.layouts.get(layout_id)
		})
	}

	pub fn rel(&self, p: &Path) -> String {
		rel(&self.root, p)
	}
}

fn file_stem(p: &Path, suffix: &str) -> String {
	let n = p
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_default();
	n.strip_suffix(suffix).unwrap_or(&n).to_owned()
}
