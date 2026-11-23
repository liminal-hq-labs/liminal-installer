// The machine abstraction: everything the engine does to the outside world goes through it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use li_script::ScriptFs;

use crate::error::{EngineError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
	File,
	Dir,
}

/// The well-known directories behind the install-time placeholders (except `{install}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirs {
	pub user_data: PathBuf,
	pub system_data: PathBuf,
	pub start_menu: PathBuf,
	pub desktop: PathBuf,
	pub temp: PathBuf,
}

impl Dirs {
	pub fn get(&self, name: &str) -> Option<&Path> {
		match name {
			"user_data" => Some(&self.user_data),
			"system_data" => Some(&self.system_data),
			"start_menu" => Some(&self.start_menu),
			"desktop" => Some(&self.desktop),
			"temp" => Some(&self.temp),
			_ => None,
		}
	}
}

/// Everything the engine needs from a machine. Paths are absolute. Directory creation does not
/// create parents, so the engine journals each directory it makes.
pub trait Machine {
	fn dirs(&self) -> &Dirs;
	/// `linux`, `windows`, `macos` and so on.
	fn os(&self) -> &str;
	fn arch(&self) -> &str;

	fn kind(&self, path: &Path) -> Option<NodeKind>;
	fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
	/// The children of a directory, as full paths, sorted.
	fn list(&self, path: &Path) -> io::Result<Vec<PathBuf>>;
	fn is_executable(&self, path: &Path) -> bool;

	/// Creates or replaces a file. The parent must exist.
	fn write(&mut self, path: &Path, data: &[u8], executable: bool) -> io::Result<()>;
	fn append(&mut self, path: &Path, data: &[u8]) -> io::Result<()>;
	fn create_dir(&mut self, path: &Path) -> io::Result<()>;
	fn remove_file(&mut self, path: &Path) -> io::Result<()>;
	/// Removes an empty directory.
	fn remove_dir(&mut self, path: &Path) -> io::Result<()>;

	/// Bytes free on the volume holding `path`.
	fn free_space(&self, path: &Path) -> u64;

	/// The platform registry stand-in: file types, URL schemes, `PATH` entries, services and
	/// package records on Linux; the real registry on Windows.
	fn registry_get(&self, key: &str) -> Option<String>;
	fn registry_set(&mut self, key: &str, value: Option<&str>) -> io::Result<()>;

	fn launch(&mut self, path: &Path, args: &[String]) -> io::Result<()>;

	/// Runs a program to completion and returns its exit code. Used for installers the engine
	/// wraps (a Tauri NSIS setup, the WebView2 bootstrapper), so a dry run can answer for them.
	fn run(&mut self, program: &Path, args: &[String]) -> io::Result<i32>;

	/// One registry value: `name` under `key` (`HKLM\SOFTWARE\...`); `""` is the default value.
	/// Values the engine wrote itself (`registry.set` operations) read back the same way.
	fn registry_value(&self, key: &str, name: &str) -> Option<String> {
		let stored = self.registry_get(&registry_entry_key(key, name))?;
		Some(decode_registry_value(&stored))
	}

	/// Fetches `url` to `dest` with `curl` (Windows 10 and later ship `curl.exe`).
	fn download(&mut self, url: &str, dest: &Path) -> io::Result<()> {
		let args = [
			"-fsSL".to_owned(),
			"-o".to_owned(),
			dest.to_string_lossy().into_owned(),
			url.to_owned(),
		];
		match self.run(Path::new("curl"), &args)? {
			0 => Ok(()),
			code => Err(io::Error::other(format!("curl exited with {code}"))),
		}
	}

	/// Read-only file access for scripts.
	fn script_fs(&self) -> Arc<dyn ScriptFs>;
}

/// The registry stand-in's key for one value, the same form `registry.set` operations use.
pub fn registry_entry_key(key: &str, name: &str) -> String {
	format!("registry:{key}:{name}")
}

/// A stored value is either the raw text a test or a platform put there, or the JSON of the
/// `registry.set` operation the engine wrote.
fn decode_registry_value(stored: &str) -> String {
	match serde_json::from_str::<li_model::Operation>(stored) {
		Ok(li_model::Operation::RegistrySet { value, .. }) => value,
		_ => stored.to_owned(),
	}
}

/// Creates `path` and any missing parents, without journaling. For scratch space only.
pub fn create_dir_all(m: &mut dyn Machine, path: &Path) -> io::Result<()> {
	if m.kind(path) == Some(NodeKind::Dir) {
		return Ok(());
	}
	if let Some(parent) = path.parent()
		&& parent != path
	{
		create_dir_all(m, parent)?;
	}
	m.create_dir(path)
}

/// Resolves and unresolves the install-time placeholders.
#[derive(Debug, Clone)]
pub struct Resolver {
	pub install: PathBuf,
	pub dirs: Dirs,
}

const NAMES: &[&str] = &[
	"install",
	"start_menu",
	"desktop",
	"user_data",
	"system_data",
	"temp",
];

impl Resolver {
	pub fn new(install: &Path, dirs: &Dirs) -> Self {
		Resolver {
			install: install.to_owned(),
			dirs: dirs.clone(),
		}
	}

	fn dir(&self, name: &str) -> Option<&Path> {
		if name == "install" {
			Some(&self.install)
		} else {
			self.dirs.get(name)
		}
	}

	/// Replaces every `{name}` placeholder.
	pub fn resolve(&self, text: &str) -> Result<PathBuf> {
		let mut out = String::new();
		let mut rest = text;
		while let Some(open) = rest.find('{') {
			out.push_str(&rest[..open]);
			let close = rest[open..]
				.find('}')
				.ok_or_else(|| EngineError::Placeholder(rest[open..].to_owned()))?;
			let name = &rest[open + 1..open + close];
			let dir = self
				.dir(name)
				.ok_or_else(|| EngineError::Placeholder(format!("{{{name}}}")))?;
			out.push_str(&dir.to_string_lossy());
			rest = &rest[open + close + 1..];
		}
		out.push_str(rest);
		Ok(PathBuf::from(out))
	}

	/// The inverse: the placeholder form of an absolute path, using the longest matching prefix.
	pub fn unresolve(&self, path: &Path) -> String {
		let best = NAMES
			.iter()
			.filter_map(|n| self.dir(n).map(|d| (*n, d)))
			.filter(|(_, d)| path.starts_with(d))
			.max_by_key(|(_, d)| d.components().count());
		match best {
			Some((name, dir)) => {
				let rest = path.strip_prefix(dir).expect("prefix checked");
				let mut out = format!("{{{name}}}");
				for part in rest.components() {
					out.push('/');
					out.push_str(&part.as_os_str().to_string_lossy());
				}
				out
			}
			None => path.to_string_lossy().into_owned(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn resolver() -> Resolver {
		let dirs = Dirs {
			user_data: "/h/.local/share".into(),
			system_data: "/usr/share".into(),
			start_menu: "/h/.local/share/applications".into(),
			desktop: "/h/Desktop".into(),
			temp: "/tmp".into(),
		};
		Resolver::new(Path::new("/opt/app"), &dirs)
	}

	#[test]
	fn resolves_and_unresolves() {
		let r = resolver();
		let p = r.resolve("{install}/SOUND/A.DRV").unwrap();
		assert_eq!(p, Path::new("/opt/app/SOUND/A.DRV"));
		assert_eq!(r.unresolve(&p), "{install}/SOUND/A.DRV");
		assert_eq!(
			r.unresolve(Path::new("/h/.local/share/applications/x.desktop")),
			"{start_menu}/x.desktop"
		);
		assert_eq!(r.unresolve(Path::new("/opt/app")), "{install}");
		assert_eq!(r.unresolve(Path::new("/opt")), "/opt");
		assert!(r.resolve("{nope}/x").is_err());
		assert!(r.resolve("{install").is_err());
	}
}
