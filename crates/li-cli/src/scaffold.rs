// `liminst init`: the files of a new installer project with a neutral theme.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

pub use li_studio::scaffold::{InitOptions, files};

use crate::{CliError, CliResult};

/// Writes the scaffold into `root/installer/`. Refuses to touch an existing folder unless
/// `force` is set. Returns the paths written.
pub fn init(root: &Path, o: &InitOptions) -> CliResult<Vec<PathBuf>> {
	let manifest = root.join("installer").join("installer.toml");
	if manifest.exists() && !o.force {
		return Err(CliError::new(format!(
			"{} already exists; pass --force to overwrite the scaffold files",
			manifest.display()
		)));
	}
	li_studio::scaffold::init(root, o).map_err(|e| CliError::new(e.to_string()))
}
