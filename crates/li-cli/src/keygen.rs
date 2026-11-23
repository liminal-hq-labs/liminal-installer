// `liminst keygen`: a new Ed25519 key pair for signing feeds and themes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use crate::{CliError, CliResult};

/// The files `keygen` wrote.
pub struct Keys {
	pub secret: PathBuf,
	pub public: PathBuf,
	pub public_hex: String,
}

fn with_ext(base: &Path, ext: &str) -> PathBuf {
	let mut name = base.as_os_str().to_owned();
	name.push(".");
	name.push(ext);
	name.into()
}

/// Writes `<base>.key` (secret, owner-only on Unix) and `<base>.pub`. Refuses to replace either
/// file unless `force` is set, so a signing key is never lost to a stray rerun.
pub fn generate(base: &Path, force: bool) -> CliResult<Keys> {
	let secret = with_ext(base, "key");
	let public = with_ext(base, "pub");
	for p in [&secret, &public] {
		if p.exists() && !force {
			return Err(CliError::new(format!(
				"{} already exists; pass --force to replace it",
				p.display()
			)));
		}
	}
	if let Some(dir) = base.parent().filter(|d| !d.as_os_str().is_empty()) {
		std::fs::create_dir_all(dir)
			.map_err(|e| CliError::new(format!("cannot create {}: {e}", dir.display())))?;
	}
	let (sk, vk) = li_pack::generate_keypair().map_err(|e| CliError::new(e.to_string()))?;
	li_pack::write_signing_key(&secret, &sk).map_err(|e| CliError::new(e.to_string()))?;
	li_pack::write_public_key(&public, &vk).map_err(|e| CliError::new(e.to_string()))?;
	Ok(Keys {
		secret,
		public,
		public_hex: li_pack::public_key_hex(&vk),
	})
}
