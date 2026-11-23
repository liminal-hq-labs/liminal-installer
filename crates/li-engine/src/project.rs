// Loading an installer project from disk.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use li_model::{
	Component, ComponentsFile, Flow, InstallerFile, Layout, Target, ThemeFile,
	project::VersionSource, theme::STAGE_LAYOUT,
};

use crate::error::{EngineError, Result};

/// A theme directory: `theme.toml` and `layouts/*.layout.json`.
#[derive(Debug, Clone)]
pub struct ThemeData {
	pub dir: PathBuf,
	pub file: ThemeFile,
	pub layouts: BTreeMap<String, Layout>,
}

/// A loaded project. `dir` is the `installer/` directory; paths in the project are relative to it.
#[derive(Debug, Clone)]
pub struct Project {
	pub root: PathBuf,
	pub dir: PathBuf,
	pub installer: InstallerFile,
	pub components: ComponentsFile,
	/// Flows keyed by flow id (the `id` in the file, not the file name).
	pub flows: BTreeMap<String, Flow>,
	/// File names, kept so checks can point at them.
	pub flow_files: BTreeMap<String, PathBuf>,
	/// Layouts shared by every theme, from `installer/layouts/`. A theme's own layout of the same
	/// id wins.
	pub layouts: BTreeMap<String, Layout>,
	/// Themes keyed by theme id.
	pub themes: BTreeMap<String, ThemeData>,
	/// Lua sources keyed by project-relative path such as `scripts/setup.lua`.
	pub scripts: BTreeMap<String, String>,
}

fn read_text(path: &Path) -> Result<String> {
	std::fs::read_to_string(path).map_err(|e| EngineError::io(path, e))
}

fn parse_err(path: &Path, e: impl std::fmt::Display) -> EngineError {
	EngineError::Parse {
		path: path.to_owned(),
		message: e.to_string(),
	}
}

fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>> {
	let mut out = Vec::new();
	let rd = match std::fs::read_dir(dir) {
		Ok(rd) => rd,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
		Err(e) => return Err(EngineError::io(dir, e)),
	};
	for entry in rd {
		out.push(entry.map_err(|e| EngineError::io(dir, e))?.path());
	}
	out.sort();
	Ok(out)
}

impl Project {
	/// Loads `<dir>/installer/installer.toml`, `components.toml`, `flows/*.flow.json`,
	/// `themes/*/theme.toml` with `layouts/*.layout.json`, and `scripts/*.lua`. Only
	/// `installer.toml` is required.
	pub fn load(dir: &Path) -> Result<Project> {
		let mut project = Self::load_installer_dir(&dir.join("installer"))?;
		project.root = dir.to_owned();
		Ok(project)
	}

	/// Loads a project whose files sit directly in `installer_dir`, whatever that folder is called.
	/// `root` is the folder's parent, or the folder itself when it has none.
	pub fn load_installer_dir(installer_dir: &Path) -> Result<Project> {
		let dir = installer_dir
			.parent()
			.filter(|p| !p.as_os_str().is_empty())
			.unwrap_or(installer_dir);
		let installer_dir = installer_dir.to_path_buf();
		let installer_path = installer_dir.join("installer.toml");
		let installer: InstallerFile = toml::from_str(&read_text(&installer_path)?)
			.map_err(|e| parse_err(&installer_path, e))?;

		let components_path = installer_dir.join("components.toml");
		let components: ComponentsFile = if components_path.exists() {
			toml::from_str(&read_text(&components_path)?)
				.map_err(|e| parse_err(&components_path, e))?
		} else {
			ComponentsFile::default()
		};

		let mut flows = BTreeMap::new();
		let mut flow_files = BTreeMap::new();
		for path in sorted_entries(&installer_dir.join("flows"))? {
			let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
			if !name.is_some_and(|n| n.ends_with(".flow.json")) {
				continue;
			}
			let flow: Flow =
				serde_json::from_str(&read_text(&path)?).map_err(|e| parse_err(&path, e))?;
			flow_files.insert(flow.id.clone(), path);
			flows.insert(flow.id.clone(), flow);
		}

		let mut layouts = BTreeMap::new();
		for lp in sorted_entries(&installer_dir.join("layouts"))? {
			let name = lp.file_name().map(|n| n.to_string_lossy().into_owned());
			if !name.is_some_and(|n| n.ends_with(".layout.json")) {
				continue;
			}
			let layout: Layout =
				serde_json::from_str(&read_text(&lp)?).map_err(|e| parse_err(&lp, e))?;
			layouts.insert(layout.id.clone(), layout);
		}

		let mut themes = BTreeMap::new();
		for theme_dir in sorted_entries(&installer_dir.join("themes"))? {
			let theme_path = theme_dir.join("theme.toml");
			if !theme_path.is_file() {
				continue;
			}
			let file: ThemeFile =
				toml::from_str(&read_text(&theme_path)?).map_err(|e| parse_err(&theme_path, e))?;
			let mut layouts = BTreeMap::new();
			for lp in sorted_entries(&theme_dir.join("layouts"))? {
				let name = lp.file_name().map(|n| n.to_string_lossy().into_owned());
				if !name.is_some_and(|n| n.ends_with(".layout.json")) {
					continue;
				}
				let layout: Layout =
					serde_json::from_str(&read_text(&lp)?).map_err(|e| parse_err(&lp, e))?;
				layouts.insert(layout.id.clone(), layout);
			}
			themes.insert(
				file.theme.id.clone(),
				ThemeData {
					dir: theme_dir,
					file,
					layouts,
				},
			);
		}

		let mut scripts = BTreeMap::new();
		for path in sorted_entries(&installer_dir.join("scripts"))? {
			if path.extension().is_some_and(|e| e == "lua")
				&& let Some(name) = path.file_name()
			{
				scripts.insert(
					format!("scripts/{}", name.to_string_lossy()),
					read_text(&path)?,
				);
			}
		}

		Ok(Project {
			root: dir.to_owned(),
			dir: installer_dir,
			installer,
			components,
			flows,
			flow_files,
			layouts,
			themes,
			scripts,
		})
	}

	/// A target by its `os-arch` id.
	pub fn target(&self, id: &str) -> Option<&Target> {
		self.installer.targets.iter().find(|t| t.id == id)
	}

	pub fn component(&self, id: &str) -> Option<&Component> {
		self.components.components.iter().find(|c| c.id == id)
	}

	/// The theme a target uses: its own `theme`, else the project default.
	pub fn theme_id_for<'s>(&'s self, target: &'s Target) -> Option<&'s str> {
		target
			.theme
			.as_deref()
			.or(self.installer.defaults.theme.as_deref())
	}

	/// Finds a layout for a target: the theme's own, then the themes on its `fallback` chain that
	/// the project has, then the project's shared layouts. A fallback the project does not have,
	/// such as the built-in `plain`, ends the chain.
	pub fn layout_for(&self, target: &Target, layout_id: &str) -> Option<&Layout> {
		let mut next = self.theme_id_for(target);
		let mut seen = Vec::new();
		while let Some(id) = next {
			if seen.contains(&id) {
				break;
			}
			seen.push(id);
			let Some(theme) = self.themes.get(id) else {
				break;
			};
			if let Some(l) = theme.layouts.get(layout_id) {
				return Some(l);
			}
			next = theme.file.theme.fallback.as_deref();
		}
		self.layouts.get(layout_id)
	}

	/// The whole screen for a step: the page's layout, drawn inside the stage of the first theme on the
	/// target's chain that has one (see `Layout::composed`). Without a stage, or when the page draws
	/// its own backdrop, it is the page's layout.
	pub fn screen_for(
		&self,
		target: &Target,
		layout_id: &str,
		step_id: &str,
		step_kind: &str,
	) -> Option<Layout> {
		let page = self.layout_for(target, layout_id)?;
		match self.stage_for(target) {
			Some(stage) => Some(Layout::composed(stage, page, step_id, step_kind)),
			None => Some(page.clone()),
		}
	}

	/// The `stage` layout of the target's theme, or of the nearest theme on its `fallback` chain.
	pub fn stage_for(&self, target: &Target) -> Option<&Layout> {
		let mut next = self.theme_id_for(target);
		let mut seen = Vec::new();
		while let Some(id) = next {
			if seen.contains(&id) {
				break;
			}
			seen.push(id);
			let theme = self.themes.get(id)?;
			if let Some(l) = theme.layouts.get(STAGE_LAYOUT) {
				return Some(l);
			}
			next = theme.file.theme.fallback.as_deref();
		}
		None
	}

	/// The literal version, or `None` when it is read from the build (`{ from = "git-tag" }`).
	pub fn literal_version(&self) -> Option<&str> {
		match &self.installer.product.version {
			VersionSource::Literal(v) => Some(v),
			VersionSource::From { .. } => None,
		}
	}
}
