// Loading an installer project from disk, through the engine's loader.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use li_engine::EngineError;
pub use li_engine::{Project, ThemeData};

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

/// A path relative to `root`, with `/` separators.
pub fn rel(root: &Path, p: &Path) -> String {
	p.strip_prefix(root)
		.unwrap_or(p)
		.to_string_lossy()
		.replace('\\', "/")
}

/// The folder that holds `installer.toml`, given the project root or `installer/` itself.
pub fn installer_dir(root: &Path) -> Option<PathBuf> {
	if root.join("installer/installer.toml").is_file() {
		Some(root.join("installer"))
	} else if root.join("installer.toml").is_file() {
		Some(root.to_path_buf())
	} else {
		None
	}
}

/// Loads the project under `root`. `root` may be the folder that holds `installer/`, or
/// `installer/` itself.
pub fn load(root: &Path) -> Result<Project, LoadError> {
	let root = if root.join("installer/installer.toml").is_file() {
		root.to_path_buf()
	} else if root.join("installer.toml").is_file() {
		root.canonicalize()
			.ok()
			.and_then(|p| p.parent().map(Path::to_path_buf))
			.unwrap_or_else(|| root.to_path_buf())
	} else {
		return Err(LoadError::NotFound(root.to_path_buf()));
	};
	Project::load(&root).map_err(|e| match e {
		EngineError::Parse { path, message } => {
			let code = if path.ends_with("installer.toml") {
				"invalid-installer"
			} else if path.ends_with("components.toml") {
				"invalid-components"
			} else if path.ends_with("theme.toml") {
				"invalid-theme"
			} else if path.to_string_lossy().ends_with(".layout.json") {
				"invalid-layout"
			} else {
				"invalid-flow"
			};
			LoadError::Invalid(Diagnostic::error(code, message).in_file(rel(&root, &path)))
		}
		EngineError::Io { path, source } => LoadError::Io {
			path,
			message: source.to_string(),
		},
		other => LoadError::Io {
			path: root.clone(),
			message: other.to_string(),
		},
	})
}
