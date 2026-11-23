// Bundling a project into a `.lip`, and unpacking it again at install time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A built `.lip` carries the payload files and, under `_project/`, the project that goes with
//! them, so the runner can walk the real flows with the real layouts and scripts:
//!
//! ```text
//! _project/installer/installer.toml      the manifest
//! _project/installer/components.toml
//! _project/installer/flows/…             flows
//! _project/installer/themes/…            layouts, theme files and their assets
//! _project/installer/scripts/…           Lua hooks
//! _project/feed.pub                      the pinned feed key, as hex
//! _project/version                       the product version this build is
//! _project/target                        the target id the payload was packed for
//! ```
//!
//! An update's `.lip` brings its own project, so a new version can change its flows and
//! components. The pinned feed key is the exception: the runner keeps the one from the install.

use std::path::Path;

use li_pack::{Lip, LipBuilder, VerifyingKey, parse_public_key, public_key_hex};

use crate::error::{EngineError, Result};
use crate::payload::{DirPayload, Payload};
use crate::project::Project;

/// The `.lip` directory that holds the project.
pub const PROJECT_DIR: &str = "_project";
/// Where the pinned feed key sits in a bundled `.lip`.
pub const PINNED_KEY: &str = "_project/feed.pub";
/// Where the product version sits in a bundled `.lip`.
pub const VERSION_FILE: &str = "_project/version";
/// Where the target id (`linux-x86_64`) sits in a bundled `.lip`.
pub const TARGET_FILE: &str = "_project/target";

const PROJECT_FILES: &[&str] = &["installer.toml", "components.toml"];
const PROJECT_TREES: &[&str] = &["flows/", "themes/", "scripts/"];

/// Adds the project's own files (manifest, components, flows, themes and scripts), the build's
/// target and version, and the pinned feed key, if any, to a `.lip` built by `pack_payload`. The product
/// payload under `dist/` and the like is not duplicated.
pub fn bundle_project(
	project: &Project,
	lip_bytes: &[u8],
	target: &str,
	version: &str,
	feed_key: Option<&VerifyingKey>,
) -> Result<Vec<u8>> {
	let lip = Lip::parse(lip_bytes)?;
	let mut builder = LipBuilder::new();
	for e in lip.entries() {
		let data = lip.read(&e.path).unwrap_or_default().to_vec();
		builder.add(&e.path, data, e.executable)?;
	}
	let dir = DirPayload::new(&project.dir)?;
	for path in dir.list() {
		let wanted = PROJECT_FILES.contains(&path.as_str())
			|| PROJECT_TREES.iter().any(|t| path.starts_with(t));
		if wanted {
			let data = dir.read(&path)?;
			builder.add(
				&format!("{PROJECT_DIR}/installer/{path}"),
				data,
				dir.executable(&path),
			)?;
		}
	}
	builder.add(TARGET_FILE, format!("{target}\n").into_bytes(), false)?;
	builder.add(VERSION_FILE, format!("{version}\n").into_bytes(), false)?;
	if let Some(key) = feed_key {
		builder.add(
			PINNED_KEY,
			format!("{}\n", public_key_hex(key)).into_bytes(),
			false,
		)?;
	}
	Ok(builder.build()?)
}

/// Writes the bundled project to `dir/installer/`, ready for `Project::load(dir)`.
pub fn extract_project(lip: &Lip, dir: &Path) -> Result<()> {
	let prefix = format!("{PROJECT_DIR}/installer/");
	let mut found = false;
	for e in lip.entries() {
		let Some(rel) = e.path.strip_prefix(&prefix) else {
			continue;
		};
		found = true;
		let dest = dir.join("installer").join(rel);
		if let Some(parent) = dest.parent() {
			std::fs::create_dir_all(parent).map_err(|err| EngineError::io(parent, err))?;
		}
		std::fs::write(&dest, lip.read(&e.path).unwrap_or_default())
			.map_err(|err| EngineError::io(&dest, err))?;
	}
	if found {
		Ok(())
	} else {
		Err(EngineError::Other(
			"the payload does not carry a project (no `_project/installer/` entries)".into(),
		))
	}
}

/// The pinned feed key a `.lip` carries, if any.
pub fn pinned_key(lip: &Lip) -> Result<Option<VerifyingKey>> {
	match lip.read(PINNED_KEY) {
		Some(bytes) => Ok(Some(parse_public_key(&String::from_utf8_lossy(bytes))?)),
		None => Ok(None),
	}
}

fn text_entry(lip: &Lip, path: &str) -> Option<String> {
	lip.read(path)
		.map(|b| String::from_utf8_lossy(b).trim().to_owned())
		.filter(|v| !v.is_empty())
}

/// The product version a `.lip` was built for, if it carries one.
pub fn bundled_version(lip: &Lip) -> Option<String> {
	text_entry(lip, VERSION_FILE)
}

/// The target id a `.lip` was packed for, if it carries one.
pub fn bundled_target(lip: &Lip) -> Option<String> {
	text_entry(lip, TARGET_FILE)
}
