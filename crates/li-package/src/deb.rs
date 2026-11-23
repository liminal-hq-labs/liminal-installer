// Debian packages: the `DEBIAN/control` file, the file tree and `dpkg-deb`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;
use std::process::Command;

use crate::info::PackageInfo;
use crate::{PackageError, Result};

/// The `DEBIAN/control` file. `installed_size_kib` is the unpacked size.
pub fn control(info: &PackageInfo, installed_size_kib: u64) -> String {
	let mut out = String::new();
	out.push_str(&format!("Package: {}\n", info.slug()));
	out.push_str(&format!("Version: {}\n", info.version));
	out.push_str("Section: misc\nPriority: optional\n");
	out.push_str(&format!("Architecture: {}\n", info.arch.deb()));
	out.push_str(&format!(
		"Maintainer: {} <noreply@localhost>\n",
		info.publisher
	));
	out.push_str(&format!("Installed-Size: {installed_size_kib}\n"));
	if let Some(h) = &info.homepage {
		out.push_str(&format!("Homepage: {h}\n"));
	}
	out.push_str(&format!("Description: {}\n", info.name));
	if info.description != info.name {
		out.push_str(&format!(" {}\n", info.description));
	}
	out
}

/// Writes the package tree under `root`: `DEBIAN/control`, the payload under `/opt/<slug>` and
/// a `/usr/bin/<slug>` link to the main program. Returns the unpacked size in KiB.
pub fn write_tree(info: &PackageInfo, root: &Path) -> Result<u64> {
	let install = root.join(info.linux_install_dir().trim_start_matches('/'));
	let mut bytes = 0u64;
	for e in info.payload()? {
		let data = e.read()?;
		bytes += data.len() as u64;
		let path = install.join(&e.dest);
		if let Some(dir) = path.parent() {
			std::fs::create_dir_all(dir).map_err(|err| PackageError::io(dir, err))?;
		}
		std::fs::write(&path, data).map_err(|err| PackageError::io(&path, err))?;
		set_mode(&path, if e.executable { 0o755 } else { 0o644 })?;
	}
	if let Some(exe) = &info.executable {
		let bin = root.join("usr/bin");
		std::fs::create_dir_all(&bin).map_err(|err| PackageError::io(&bin, err))?;
		#[cfg(unix)]
		{
			let link = bin.join(info.slug());
			let target = format!("{}/{exe}", info.linux_install_dir());
			std::os::unix::fs::symlink(target, &link)
				.map_err(|err| PackageError::io(&link, err))?;
		}
	}
	let size = bytes.div_ceil(1024);
	let debian = root.join("DEBIAN");
	std::fs::create_dir_all(&debian).map_err(|err| PackageError::io(&debian, err))?;
	set_mode(&debian, 0o755)?;
	let control_path = debian.join("control");
	std::fs::write(&control_path, control(info, size))
		.map_err(|err| PackageError::io(&control_path, err))?;
	set_mode(&control_path, 0o644)?;
	Ok(size)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
	use std::os::unix::fs::PermissionsExt;
	std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
		.map_err(|e| PackageError::io(path, e))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
	Ok(())
}

/// Writes the tree under `work` and runs `dpkg-deb` to produce `out`.
pub fn build(info: &PackageInfo, work: &Path, out: &Path) -> Result<()> {
	write_tree(info, work)?;
	if let Some(dir) = out.parent() {
		std::fs::create_dir_all(dir).map_err(|e| PackageError::io(dir, e))?;
	}
	let result = Command::new("dpkg-deb")
		.args(["--root-owner-group", "--build"])
		.arg(work)
		.arg(out)
		.output();
	match result {
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
			Err(PackageError::ToolMissing("dpkg-deb".into()))
		}
		Err(e) => Err(PackageError::io("dpkg-deb", e)),
		Ok(o) if !o.status.success() => Err(PackageError::ToolFailed {
			tool: "dpkg-deb".into(),
			detail: String::from_utf8_lossy(&o.stderr).trim().to_owned(),
		}),
		Ok(_) => Ok(()),
	}
}
