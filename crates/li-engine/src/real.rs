// The real machine: the local file system with Linux user-scope conventions.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use li_script::ScriptFs;

use crate::machine::{Dirs, Machine, NodeKind};

/// The local machine. Files go through `std::fs`. The registry stand-in is a JSON file,
/// `{user_data}/liminal-installer/registry.json`, deleted when it empties.
pub struct RealMachine {
	dirs: Dirs,
}

impl RealMachine {
	pub fn new(dirs: Dirs) -> Self {
		RealMachine { dirs }
	}

	/// User-scope directories from the XDG variables and `HOME`:
	/// `{user_data}` is `$XDG_DATA_HOME` or `~/.local/share`, `{start_menu}` is its `applications`
	/// directory, `{desktop}` is `~/Desktop`, `{system_data}` is `/usr/share`.
	pub fn from_environment() -> Self {
		let home = std::env::var_os("HOME")
			.map(PathBuf::from)
			.unwrap_or_else(|| PathBuf::from("/"));
		let user_data = std::env::var_os("XDG_DATA_HOME")
			.map(PathBuf::from)
			.filter(|p| p.is_absolute())
			.unwrap_or_else(|| home.join(".local/share"));
		RealMachine::new(Dirs {
			start_menu: user_data.join("applications"),
			desktop: home.join("Desktop"),
			system_data: PathBuf::from("/usr/share"),
			temp: std::env::temp_dir(),
			windows: None,
			system: None,
			user_data,
		})
	}

	/// A machine whose directories all live under `root`. Existing directories are created so
	/// that a tree snapshot taken before an install is complete. Meant for tests and dry runs
	/// against a scratch directory.
	pub fn sandboxed(root: &Path) -> io::Result<Self> {
		let dirs = Dirs {
			user_data: root.join("data"),
			system_data: root.join("sysdata"),
			start_menu: root.join("data/applications"),
			desktop: root.join("Desktop"),
			temp: root.join("tmp"),
			windows: Some(root.join("windows")),
			system: Some(root.join("windows/system")),
		};
		for d in [
			&dirs.user_data,
			&dirs.system_data,
			&dirs.desktop,
			&dirs.temp,
		] {
			std::fs::create_dir_all(d)?;
		}
		for d in dirs.windows.iter().chain(&dirs.system) {
			std::fs::create_dir_all(d)?;
		}
		Ok(RealMachine::new(dirs))
	}

	fn registry_path(&self) -> PathBuf {
		self.dirs.user_data.join("liminal-installer/registry.json")
	}

	fn load_registry(&self) -> BTreeMap<String, String> {
		std::fs::read(self.registry_path())
			.ok()
			.and_then(|b| serde_json::from_slice(&b).ok())
			.unwrap_or_default()
	}
}

impl Machine for RealMachine {
	fn dirs(&self) -> &Dirs {
		&self.dirs
	}

	fn os(&self) -> &str {
		std::env::consts::OS
	}

	fn arch(&self) -> &str {
		std::env::consts::ARCH
	}

	fn kind(&self, path: &Path) -> Option<NodeKind> {
		let meta = std::fs::symlink_metadata(path).ok()?;
		if meta.is_dir() {
			Some(NodeKind::Dir)
		} else {
			Some(NodeKind::File)
		}
	}

	fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
		std::fs::read(path)
	}

	fn list(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
		let mut out = Vec::new();
		for entry in std::fs::read_dir(path)? {
			out.push(entry?.path());
		}
		out.sort();
		Ok(out)
	}

	fn is_executable(&self, path: &Path) -> bool {
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
		}
		#[cfg(not(unix))]
		{
			let _ = path;
			false
		}
	}

	fn write(&mut self, path: &Path, data: &[u8], executable: bool) -> io::Result<()> {
		std::fs::write(path, data)?;
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			let mode = if executable { 0o755 } else { 0o644 };
			std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
		}
		#[cfg(not(unix))]
		let _ = executable;
		Ok(())
	}

	fn append(&mut self, path: &Path, data: &[u8]) -> io::Result<()> {
		use std::io::Write;
		let mut f = std::fs::OpenOptions::new()
			.create(true)
			.append(true)
			.open(path)?;
		f.write_all(data)
	}

	fn create_dir(&mut self, path: &Path) -> io::Result<()> {
		std::fs::create_dir(path)
	}

	fn remove_file(&mut self, path: &Path) -> io::Result<()> {
		std::fs::remove_file(path)
	}

	fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
		std::fs::remove_dir(path)
	}

	fn free_space(&self, path: &Path) -> u64 {
		#[cfg(unix)]
		{
			let mut probe = path;
			while !probe.exists() {
				match probe.parent() {
					Some(p) => probe = p,
					None => return u64::MAX,
				}
			}
			match rustix::fs::statvfs(probe) {
				Ok(s) => s.f_bavail.saturating_mul(s.f_frsize),
				Err(_) => u64::MAX,
			}
		}
		#[cfg(not(unix))]
		{
			let _ = path;
			u64::MAX
		}
	}

	fn registry_get(&self, key: &str) -> Option<String> {
		self.load_registry().get(key).cloned()
	}

	fn registry_set(&mut self, key: &str, value: Option<&str>) -> io::Result<()> {
		let mut reg = self.load_registry();
		match value {
			Some(v) => {
				reg.insert(key.to_owned(), v.to_owned());
			}
			None => {
				reg.remove(key);
			}
		}
		let path = self.registry_path();
		if reg.is_empty() {
			match std::fs::remove_file(&path) {
				Ok(()) => {}
				Err(e) if e.kind() == io::ErrorKind::NotFound => {}
				Err(e) => return Err(e),
			}
			// Remove the directory only if it holds nothing else.
			if let Some(dir) = path.parent() {
				let _ = std::fs::remove_dir(dir);
			}
			return Ok(());
		}
		if let Some(dir) = path.parent() {
			std::fs::create_dir_all(dir)?;
		}
		std::fs::write(
			&path,
			serde_json::to_vec_pretty(&reg).map_err(io::Error::other)?,
		)
	}

	fn launch(&mut self, path: &Path, args: &[String]) -> io::Result<()> {
		std::process::Command::new(path)
			.args(args)
			.stdin(std::process::Stdio::null())
			.spawn()
			.map(|_| ())
	}

	fn run(&mut self, program: &Path, args: &[String]) -> io::Result<i32> {
		let status = std::process::Command::new(program)
			.args(args)
			.stdin(std::process::Stdio::null())
			.status()?;
		Ok(status.code().unwrap_or(-1))
	}

	fn script_fs(&self) -> Arc<dyn ScriptFs> {
		Arc::new(StdFs)
	}
}

struct StdFs;

impl ScriptFs for StdFs {
	fn read(&self, path: &str) -> Result<Vec<u8>, String> {
		std::fs::read(path).map_err(|e| format!("{path}: {e}"))
	}

	fn exists(&self, path: &str) -> bool {
		Path::new(path).exists()
	}

	fn list(&self, path: &str) -> Result<Vec<String>, String> {
		let mut names: Vec<String> = std::fs::read_dir(path)
			.map_err(|e| format!("{path}: {e}"))?
			.filter_map(|e| e.ok())
			.map(|e| e.file_name().to_string_lossy().into_owned())
			.collect();
		names.sort();
		Ok(names)
	}
}
