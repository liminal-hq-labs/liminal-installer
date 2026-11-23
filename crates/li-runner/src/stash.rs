// The runner's own files inside an install: `{install}/.liminal/runner/`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use li_model::Feed;
use li_pack::VerifyingKey;

use crate::{RunnerError, RunnerResult};

pub const RUNNER_DIR: &str = ".liminal/runner";
pub const BINARY: &str = "liminal-engine";
pub const PAYLOAD: &str = "payload.lip";
pub const FEED_KEY: &str = "feed.pub";
pub const FEED_SEEN: &str = "feed-seen.json";

fn io(path: &Path, e: std::io::Error) -> RunnerError {
	RunnerError::new(format!("{}: {e}", path.display()))
}

/// Where the runner keeps its files for an install.
pub fn dir(install_dir: &Path) -> PathBuf {
	install_dir.join(RUNNER_DIR)
}

/// The install directory this binary belongs to, when it runs from the stash.
pub fn install_dir_of_current_exe() -> Option<PathBuf> {
	let exe = std::env::current_exe().ok()?;
	let runner = exe.parent()?;
	let liminal = runner.parent()?;
	if runner.file_name()? == "runner" && liminal.file_name()? == ".liminal" {
		Some(liminal.parent()?.to_path_buf())
	} else {
		None
	}
}

pub fn write(install_dir: &Path, name: &str, bytes: &[u8]) -> RunnerResult<()> {
	let d = dir(install_dir);
	std::fs::create_dir_all(&d).map_err(|e| io(&d, e))?;
	let p = d.join(name);
	std::fs::write(&p, bytes).map_err(|e| io(&p, e))
}

pub fn read(install_dir: &Path, name: &str) -> RunnerResult<Vec<u8>> {
	let p = dir(install_dir).join(name);
	std::fs::read(&p).map_err(|e| io(&p, e))
}

pub fn read_optional(install_dir: &Path, name: &str) -> Option<Vec<u8>> {
	std::fs::read(dir(install_dir).join(name)).ok()
}

/// The feed key pinned when the product was installed.
pub fn pinned_key(install_dir: &Path) -> RunnerResult<Option<VerifyingKey>> {
	match read_optional(install_dir, FEED_KEY) {
		Some(b) => Ok(Some(li_pack::parse_public_key(&String::from_utf8_lossy(
			&b,
		))?)),
		None => Ok(None),
	}
}

/// The last feed that verified, for the rollback check.
pub fn last_feed(install_dir: &Path) -> Option<Feed> {
	serde_json::from_slice(&read_optional(install_dir, FEED_SEEN)?).ok()
}

/// The file name a runner keeps in the stash: its own, so the web runner stays `liminal-runner`.
fn exe_name(exe: &Path) -> std::ffi::OsString {
	exe.file_name()
		.map_or_else(|| BINARY.into(), ToOwned::to_owned)
}

/// Copies this binary into the stash and writes the `.lip` and the pinned key beside it.
pub fn save(install_dir: &Path, lip: &[u8], key: Option<&VerifyingKey>) -> RunnerResult<()> {
	let d = dir(install_dir);
	std::fs::create_dir_all(&d).map_err(|e| io(&d, e))?;
	let exe = std::env::current_exe().map_err(|e| RunnerError::new(e.to_string()))?;
	let dest = d.join(exe_name(&exe));
	if std::fs::canonicalize(&exe).ok() != std::fs::canonicalize(&dest).ok() {
		std::fs::copy(&exe, &dest).map_err(|e| io(&dest, e))?;
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
				.map_err(|e| io(&dest, e))?;
		}
	}
	write(install_dir, PAYLOAD, lip)?;
	if let Some(k) = key {
		write(
			install_dir,
			FEED_KEY,
			format!("{}\n", li_pack::public_key_hex(k)).as_bytes(),
		)?;
	}
	Ok(())
}

/// Removes the files the stub extracted (`engine/` and `payload.lip` under `from`), now that
/// the runner has its own copies, and `from` itself when it is empty and not the install.
pub fn clean_bootstrap(from: &Path, install_dir: &Path) {
	let name = std::env::current_exe()
		.ok()
		.map_or_else(|| BINARY.into(), |e| exe_name(&e));
	// Only what the `.run` stub extracted goes: a folder with no `engine/` is a bundle someone keeps.
	if !from.join("engine").join(&name).is_file() {
		return;
	}
	let _ = std::fs::remove_file(from.join("engine").join(name));
	let _ = std::fs::remove_dir(from.join("engine"));
	let _ = std::fs::remove_file(from.join(PAYLOAD));
	if from != install_dir {
		let _ = std::fs::remove_dir(from);
	}
}

/// Removes the stash, then the receipt's directory and the install directory if they are empty.
pub fn remove(install_dir: &Path) {
	let _ = std::fs::remove_dir_all(dir(install_dir));
	let _ = std::fs::remove_dir(install_dir.join(".liminal"));
	let _ = std::fs::remove_dir(install_dir);
}
