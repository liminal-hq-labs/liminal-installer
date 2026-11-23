// The open project: its files read into li-model types and written back on every change.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use li_model::{ComponentsFile, Flow, InstallerFile, Layout, ThemeFile};
use serde::{Deserialize, Serialize};

use crate::error::StudioError;

/// Names one editable document.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "type", content = "id", rename_all = "lowercase")]
pub enum DocKey {
	Flow(String),
	Layout(String),
	/// A path relative to `installer/`, such as `scripts/sound-setup.lua`.
	Script(String),
	/// The Tauri import and its settings: `installer.toml`, `components.toml` and `tauri.json`
	/// change together, so they are one document in the undo history.
	Tauri,
}

/// The text of the files a Tauri import or settings change writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TauriFiles {
	pub installer_toml: String,
	pub components_toml: String,
	/// `None` when the project has not been imported.
	pub snapshot_json: Option<String>,
}

/// The contents of one editable document.
#[derive(Debug, Clone, PartialEq)]
pub enum Doc {
	Flow(Flow),
	Layout(Layout),
	Script(String),
	Tauri(TauriFiles),
}

/// An `installer/` directory loaded into memory.
#[derive(Debug, Clone)]
pub struct Project {
	/// The `installer/` directory.
	pub root: PathBuf,
	pub installer: InstallerFile,
	pub components: ComponentsFile,
	pub flows: BTreeMap<String, Flow>,
	pub layouts: BTreeMap<String, Layout>,
	pub themes: BTreeMap<String, ThemeFile>,
	pub scripts: BTreeMap<String, String>,
}

/// Serializes as JSON indented with tabs, as the project's `.editorconfig` asks.
pub fn to_tab_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
	let mut buf = Vec::new();
	let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
	value.serialize(&mut ser)?;
	buf.push(b'\n');
	Ok(String::from_utf8(buf).expect("serde_json writes UTF-8"))
}

fn safe_segment(id: &str) -> bool {
	!id.is_empty()
		&& id != "."
		&& !id.contains("..")
		&& id
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A document id: one safe name, or for a layout `theme/name`.
fn safe_id(id: &str) -> bool {
	let mut parts = id.split('/');
	let (a, b) = (parts.next(), parts.next());
	parts.next().is_none() && a.is_some_and(safe_segment) && b.is_none_or(safe_segment)
}

fn safe_script(path: &str) -> bool {
	path.starts_with("scripts/")
		&& path.ends_with(".lua")
		&& !path.contains("..")
		&& !path.contains('\\')
		&& !path.starts_with('/')
}

impl DocKey {
	pub fn validate(&self) -> Result<(), StudioError> {
		let ok = match self {
			DocKey::Flow(id) => safe_id(id) && !id.contains('/'),
			DocKey::Layout(id) => safe_id(id),
			DocKey::Script(path) => safe_script(path),
			DocKey::Tauri => true,
		};
		if ok {
			Ok(())
		} else {
			Err(StudioError::invalid("document name", format!("{self:?}")))
		}
	}

	fn relative_path(&self) -> String {
		match self {
			DocKey::Flow(id) => format!("flows/{id}.flow.json"),
			DocKey::Layout(id) => match id.split_once('/') {
				Some((theme, name)) => format!("themes/{theme}/layouts/{name}.layout.json"),
				None => format!("layouts/{id}.layout.json"),
			},
			DocKey::Script(path) => path.clone(),
			DocKey::Tauri => "installer.toml".to_owned(),
		}
	}
}

fn read(path: &Path) -> Result<String, StudioError> {
	fs::read_to_string(path).map_err(|e| StudioError::io(path, e))
}

fn dir_files(dir: &Path, suffix: &str) -> Vec<PathBuf> {
	let Ok(rd) = fs::read_dir(dir) else {
		return Vec::new();
	};
	let mut out: Vec<PathBuf> = rd
		.filter_map(Result::ok)
		.map(|e| e.path())
		.filter(|p| {
			p.file_name()
				.and_then(|n| n.to_str())
				.is_some_and(|n| n.ends_with(suffix))
		})
		.collect();
	out.sort();
	out
}

impl Project {
	/// Opens a project from its `installer/` directory, or from the directory that holds it.
	pub fn open(path: &Path) -> Result<Project, StudioError> {
		let root = if path.join("installer.toml").is_file() {
			path.to_path_buf()
		} else if path.join("installer/installer.toml").is_file() {
			path.join("installer")
		} else {
			return Err(StudioError::NotFound(format!(
				"installer.toml under {}",
				path.display()
			)));
		};
		let installer = toml::from_str(&read(&root.join("installer.toml"))?)
			.map_err(|e| StudioError::invalid("installer.toml", e))?;
		let components_path = root.join("components.toml");
		let components = if components_path.is_file() {
			toml::from_str(&read(&components_path)?)
				.map_err(|e| StudioError::invalid("components.toml", e))?
		} else {
			ComponentsFile::default()
		};
		let mut flows = BTreeMap::new();
		for p in dir_files(&root.join("flows"), ".flow.json") {
			let flow: Flow = serde_json::from_str(&read(&p)?)
				.map_err(|e| StudioError::invalid(p.display().to_string(), e))?;
			flows.insert(flow.id.clone(), flow);
		}
		let mut layouts = BTreeMap::new();
		for p in dir_files(&root.join("layouts"), ".layout.json") {
			let layout: Layout = serde_json::from_str(&read(&p)?)
				.map_err(|e| StudioError::invalid(p.display().to_string(), e))?;
			layouts.insert(layout.id.clone(), layout);
		}
		let mut themes = BTreeMap::new();
		if let Ok(rd) = fs::read_dir(root.join("themes")) {
			for entry in rd.filter_map(Result::ok) {
				let file = entry.path().join("theme.toml");
				if file.is_file() {
					let theme: ThemeFile = toml::from_str(&read(&file)?)
						.map_err(|e| StudioError::invalid(file.display().to_string(), e))?;
					let theme_id = theme.theme.id.clone();
					themes.insert(theme_id.clone(), theme);
					for p in dir_files(&entry.path().join("layouts"), ".layout.json") {
						let mut layout: Layout = serde_json::from_str(&read(&p)?)
							.map_err(|e| StudioError::invalid(p.display().to_string(), e))?;
						layout.id = format!("{theme_id}/{}", layout.id);
						layouts.insert(layout.id.clone(), layout);
					}
				}
			}
		}
		let mut scripts = BTreeMap::new();
		for p in dir_files(&root.join("scripts"), ".lua") {
			let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
			scripts.insert(format!("scripts/{name}"), read(&p)?);
		}
		Ok(Project {
			root,
			installer,
			components,
			flows,
			layouts,
			themes,
			scripts,
		})
	}

	/// The layout a flow step names: the shared one, else the first theme's.
	pub fn find_layout(&self, id: &str) -> Option<&Layout> {
		self.layouts.get(id).or_else(|| {
			self.layouts
				.iter()
				.find(|(k, _)| k.split_once('/').is_some_and(|(_, n)| n == id))
				.map(|(_, l)| l)
		})
	}

	pub fn get(&self, key: &DocKey) -> Option<Doc> {
		match key {
			DocKey::Flow(id) => self.flows.get(id).cloned().map(Doc::Flow),
			DocKey::Layout(id) => self.layouts.get(id).cloned().map(Doc::Layout),
			DocKey::Script(path) => self.scripts.get(path).cloned().map(Doc::Script),
			DocKey::Tauri => Some(Doc::Tauri(TauriFiles {
				installer_toml: fs::read_to_string(self.root.join("installer.toml")).ok()?,
				components_toml: fs::read_to_string(self.root.join("components.toml"))
					.unwrap_or_default(),
				snapshot_json: fs::read_to_string(self.root.join(li_model::TauriSnapshot::PATH))
					.ok(),
			})),
		}
	}

	/// Replaces, creates or (with `None`) removes a document, in memory and on disk.
	pub fn put(&mut self, key: &DocKey, doc: Option<Doc>) -> Result<(), StudioError> {
		key.validate()?;
		if *key == DocKey::Tauri {
			return self.put_tauri(doc);
		}
		let path = self.root.join(key.relative_path());
		match doc {
			Some(doc) => {
				let text = match (&doc, key) {
					(Doc::Flow(f), DocKey::Flow(id)) if &f.id == id => to_tab_json(f),
					(Doc::Layout(l), DocKey::Layout(id)) if &l.id == id => {
						// A theme's layout is stored under its own short id.
						let mut file = l.clone();
						if let Some((_, name)) = id.split_once('/') {
							file.id = name.to_owned();
						}
						to_tab_json(&file)
					}
					(Doc::Script(s), DocKey::Script(_)) => Ok(s.clone()),
					_ => {
						return Err(StudioError::invalid(
							"document",
							"the document does not match its name",
						));
					}
				}
				.map_err(|e| StudioError::invalid("document", e))?;
				if let Some(parent) = path.parent() {
					fs::create_dir_all(parent).map_err(|e| StudioError::io(parent, e))?;
				}
				fs::write(&path, text).map_err(|e| StudioError::io(&path, e))?;
				match doc {
					Doc::Flow(f) => self.flows.insert(f.id.clone(), f).map(drop),
					Doc::Layout(l) => self.layouts.insert(l.id.clone(), l).map(drop),
					Doc::Script(s) => {
						let DocKey::Script(p) = key else {
							unreachable!()
						};
						self.scripts.insert(p.clone(), s).map(drop)
					}
					Doc::Tauri(_) => None,
				};
			}
			None => {
				if path.exists() {
					fs::remove_file(&path).map_err(|e| StudioError::io(&path, e))?;
				}
				match key {
					DocKey::Flow(id) => self.flows.remove(id).map(drop),
					DocKey::Layout(id) => self.layouts.remove(id).map(drop),
					DocKey::Script(p) => self.scripts.remove(p).map(drop),
					DocKey::Tauri => None,
				};
			}
		}
		Ok(())
	}

	fn put_tauri(&mut self, doc: Option<Doc>) -> Result<(), StudioError> {
		let Some(Doc::Tauri(files)) = doc else {
			return Err(StudioError::invalid(
				"document",
				"the Tauri import cannot be removed",
			));
		};
		// Parse first, so a refused change leaves every file as it was.
		let installer: InstallerFile = toml::from_str(&files.installer_toml)
			.map_err(|e| StudioError::invalid("installer.toml", e))?;
		let components: ComponentsFile = if files.components_toml.trim().is_empty() {
			ComponentsFile::default()
		} else {
			toml::from_str(&files.components_toml)
				.map_err(|e| StudioError::invalid("components.toml", e))?
		};
		let write = |name: &str, text: &str| {
			let path = self.root.join(name);
			fs::write(&path, text).map_err(|e| StudioError::io(&path, e))
		};
		write("installer.toml", &files.installer_toml)?;
		if !files.components_toml.is_empty() || self.root.join("components.toml").exists() {
			write("components.toml", &files.components_toml)?;
		}
		let snapshot = self.root.join(li_model::TauriSnapshot::PATH);
		match &files.snapshot_json {
			Some(text) => write(li_model::TauriSnapshot::PATH, text)?,
			None => {
				if snapshot.exists() {
					fs::remove_file(&snapshot).map_err(|e| StudioError::io(&snapshot, e))?;
				}
			}
		}
		self.installer = installer;
		self.components = components;
		Ok(())
	}

	/// The files a `from` rule matches for a target; `*` is the only wildcard, in the file name.
	pub fn expand_source(&self, from: &str, target_id: &str) -> Vec<PathBuf> {
		let pattern = from.replace("{target}", target_id);
		let path = self.root.join(&pattern);
		let (Some(parent), Some(file)) = (path.parent(), path.file_name().and_then(|n| n.to_str()))
		else {
			return Vec::new();
		};
		let Ok(rd) = fs::read_dir(parent) else {
			return Vec::new();
		};
		let mut out: Vec<PathBuf> = rd
			.filter_map(Result::ok)
			.map(|e| e.path())
			.filter(|p| {
				p.is_file()
					&& p.file_name()
						.and_then(|n| n.to_str())
						.is_some_and(|n| wildcard(file, n))
			})
			.collect();
		out.sort();
		out
	}

	/// The theme a target uses: its own, else the project default.
	pub fn theme_for(&self, target: &li_model::Target) -> Option<String> {
		target
			.theme
			.clone()
			.or_else(|| self.installer.defaults.theme.clone())
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
	use super::*;

	#[test]
	fn rejects_unsafe_names() {
		assert!(DocKey::Flow("install".into()).validate().is_ok());
		assert!(DocKey::Flow("../x".into()).validate().is_err());
		assert!(DocKey::Layout("a/b/c".into()).validate().is_err());
		assert!(DocKey::Layout("a/../b".into()).validate().is_err());
		assert!(DocKey::Layout("theme/welcome".into()).validate().is_ok());
		assert!(DocKey::Flow("a/b".into()).validate().is_err());
		assert!(DocKey::Script("scripts/a.lua".into()).validate().is_ok());
		assert!(
			DocKey::Script("scripts/../a.lua".into())
				.validate()
				.is_err()
		);
		assert!(DocKey::Script("/etc/passwd".into()).validate().is_err());
		assert!(DocKey::Script("scripts/a.sh".into()).validate().is_err());
	}

	#[test]
	fn doc_keys_serialize_as_tagged_objects() {
		let v = serde_json::to_value(DocKey::Flow("install".into())).unwrap();
		assert_eq!(v, serde_json::json!({"type": "flow", "id": "install"}));
	}

	#[test]
	fn tab_json_indents_with_tabs() {
		let s = to_tab_json(&serde_json::json!({"a": [1]})).unwrap();
		assert!(s.contains("\n\t\"a\""));
	}
}
