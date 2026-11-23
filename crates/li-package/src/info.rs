// The product description every generator reads, built from `li-model` types.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use li_model::project::VersionSource;
use li_model::{ComponentsFile, InstallerFile, Scope, Target};

use crate::{PackageError, Result};

/// CPU architectures the native formats name differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
	X86_64,
	Aarch64,
}

impl Arch {
	/// Reads the architecture from a target id such as `linux-x86_64`.
	pub fn from_target(id: &str) -> Arch {
		if id.contains("aarch64") || id.contains("arm64") {
			Arch::Aarch64
		} else {
			Arch::X86_64
		}
	}

	pub fn deb(self) -> &'static str {
		match self {
			Arch::X86_64 => "amd64",
			Arch::Aarch64 => "arm64",
		}
	}

	pub fn rpm(self) -> &'static str {
		match self {
			Arch::X86_64 => "x86_64",
			Arch::Aarch64 => "aarch64",
		}
	}

	pub fn freebsd(self) -> &'static str {
		match self {
			Arch::X86_64 => "amd64",
			Arch::Aarch64 => "aarch64",
		}
	}

	pub fn msix(self) -> &'static str {
		match self {
			Arch::X86_64 => "x64",
			Arch::Aarch64 => "arm64",
		}
	}
}

/// Where a payload file's bytes come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
	File(PathBuf),
	Bytes(Vec<u8>),
}

/// One file to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
	/// Path relative to the install directory, with `/` separators.
	pub dest: String,
	pub content: Content,
	pub executable: bool,
}

impl Entry {
	pub fn bytes(dest: &str, data: &[u8], executable: bool) -> Entry {
		Entry {
			dest: dest.to_owned(),
			content: Content::Bytes(data.to_vec()),
			executable,
		}
	}

	pub fn read(&self) -> Result<Vec<u8>> {
		match &self.content {
			Content::Bytes(b) => Ok(b.clone()),
			Content::File(p) => std::fs::read(p).map_err(|e| PackageError::io(p, e)),
		}
	}
}

/// What the generators need to know about a product and the files to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfo {
	pub id: String,
	pub name: String,
	pub publisher: String,
	pub version: String,
	pub description: String,
	pub licence: Option<String>,
	pub homepage: Option<String>,
	pub arch: Arch,
	pub scope: Scope,
	/// The target id such as `linux-x86_64`.
	pub target: String,
	/// The `target` string written to the receipt, such as `linux-x86_64:native`.
	pub receipt_target: String,
	pub components: Vec<String>,
	pub files: Vec<Entry>,
	/// The main program, relative to the install directory.
	pub executable: Option<String>,
	/// A file inside the payload the `.run` stub starts with `install --from <prefix>` after
	/// extracting; the project's engine binary. When `None` the stub only extracts.
	pub engine: Option<String>,
}

/// Turns the product's version setting into text. `explicit` wins; a `{ from = ... }` source
/// without one is an error, because only the build knows the tag.
pub fn resolve_version(source: &VersionSource, explicit: Option<&str>) -> Result<String> {
	match (explicit, source) {
		(Some(v), _) => Ok(v.trim_start_matches('v').to_owned()),
		(None, VersionSource::Literal(v)) => Ok(v.clone()),
		(None, VersionSource::From { from }) => Err(PackageError::VersionNeeded(from.clone())),
	}
}

impl PackageInfo {
	/// Collects the files the project's components install on `target`. Relative `from` paths
	/// resolve against `root`, `{target}` expands to the target id, and `*` matches within the
	/// last path segment.
	pub fn from_project(
		installer: &InstallerFile,
		components: &ComponentsFile,
		target: &Target,
		version: &str,
		root: &Path,
	) -> Result<PackageInfo> {
		let mut files = Vec::new();
		let mut ids = Vec::new();
		let mut executable = None;
		for c in components
			.components
			.iter()
			.filter(|c| c.applies_to(&target.id))
		{
			ids.push(c.id.clone());
			for rule in &c.files {
				let to = install_relative(&rule.to, &c.id)?;
				let from = rule.from.replace("{target}", &target.id);
				let sources = expand(root, &from)?;
				if sources.is_empty() {
					return Err(PackageError::MissingFile {
						component: c.id.clone(),
						from,
					});
				}
				for src in sources {
					let name = src
						.file_name()
						.map(|n| n.to_string_lossy().into_owned())
						.unwrap_or_default();
					let dest = if to.is_empty() || to.ends_with('/') {
						format!("{to}{name}")
					} else {
						to.clone()
					};
					files.push(Entry {
						dest,
						executable: is_executable(&src),
						content: Content::File(src),
					});
				}
			}
			if executable.is_none()
				&& let Some(s) = c.shortcut.first()
				&& let Ok(rel) = install_relative(&s.target, &c.id)
			{
				executable = Some(rel);
			}
		}
		files.sort_by(|a, b| a.dest.cmp(&b.dest));
		Ok(PackageInfo {
			id: installer.product.id.clone(),
			name: installer.product.name.clone(),
			publisher: installer.product.publisher.clone(),
			version: version.to_owned(),
			description: installer.product.name.clone(),
			licence: None,
			homepage: None,
			arch: Arch::from_target(&target.id),
			scope: installer.defaults.scope,
			target: target.id.clone(),
			receipt_target: format!("{}:native", target.id),
			components: ids,
			files,
			executable,
			engine: None,
		})
	}

	/// A short package name: the last dotted segment of the product id, lowercased and limited
	/// to characters every package format accepts. `hq.liminal.lt-fizz` gives `lt-fizz`.
	pub fn slug(&self) -> String {
		let last = self.id.rsplit('.').next().unwrap_or(&self.id);
		let s: String = last
			.to_lowercase()
			.chars()
			.map(|c| {
				if c.is_ascii_alphanumeric() || c == '-' {
					c
				} else {
					'-'
				}
			})
			.collect();
		let s = s.trim_matches('-').to_owned();
		if s.is_empty() {
			"product".to_owned()
		} else {
			s
		}
	}

	/// Where Linux and FreeBSD native packages install the product.
	pub fn linux_install_dir(&self) -> String {
		format!("/opt/{}", self.slug())
	}

	/// The version with characters that RPM and Arch reject replaced by `_`.
	pub fn package_version(&self) -> String {
		self.version.replace(['-', '+'], "_")
	}

	/// The top directory of source tarballs: `lt-fizz-1.0.0`.
	pub fn source_dir(&self) -> String {
		format!("{}-{}", self.slug(), self.package_version())
	}

	/// All files to install, including the generated receipt, sorted by destination.
	pub fn payload(&self) -> Result<Vec<Entry>> {
		let mut all = self.files.clone();
		all.push(crate::payload::receipt_entry(self)?);
		all.sort_by(|a, b| a.dest.cmp(&b.dest));
		Ok(all)
	}
}

fn is_executable(p: &Path) -> bool {
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
	}
	#[cfg(not(unix))]
	{
		let _ = p;
		false
	}
}

/// Turns `{install}/SOUND/` into `SOUND/`; any other placeholder is an error.
fn install_relative(path: &str, component: &str) -> Result<String> {
	let rest = path.strip_prefix("{install}").unwrap_or(path);
	if let Some(start) = rest.find('{') {
		let end = rest[start..]
			.find('}')
			.map_or(rest.len(), |e| start + e + 1);
		return Err(PackageError::Placeholder {
			component: component.to_owned(),
			placeholder: rest[start..end].to_owned(),
			path: path.to_owned(),
		});
	}
	Ok(rest.trim_start_matches('/').to_owned())
}

fn expand(root: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
	let full = root.join(pattern);
	let name = full
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_default();
	if !name.contains('*') {
		return Ok(if full.is_file() { vec![full] } else { vec![] });
	}
	let dir = full.parent().unwrap_or(root);
	let Ok(read) = std::fs::read_dir(dir) else {
		return Ok(vec![]);
	};
	let mut out: Vec<PathBuf> = read
		.filter_map(|e| e.ok())
		.map(|e| e.path())
		.filter(|p| {
			p.is_file()
				&& p.file_name()
					.is_some_and(|n| glob_match(&name, &n.to_string_lossy()))
		})
		.collect();
	out.sort();
	Ok(out)
}

/// Matches `*` against any run of characters; everything else is literal.
fn glob_match(pattern: &str, text: &str) -> bool {
	let parts: Vec<&str> = pattern.split('*').collect();
	let mut rest = text;
	for (i, part) in parts.iter().enumerate() {
		if i == 0 {
			let Some(r) = rest.strip_prefix(part) else {
				return false;
			};
			rest = r;
		} else if i == parts.len() - 1 {
			return rest.ends_with(part);
		} else {
			let Some(pos) = rest.find(part) else {
				return false;
			};
			rest = &rest[pos + part.len()..];
		}
	}
	rest.is_empty()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn globs() {
		assert!(glob_match("*.DRV", "SB.DRV"));
		assert!(!glob_match("*.DRV", "SB.EXE"));
		assert!(glob_match("a*c*e", "abcde"));
		assert!(glob_match("FIZZ.EXE", "FIZZ.EXE"));
	}

	#[test]
	fn install_paths() {
		assert_eq!(install_relative("{install}/SOUND/", "c").unwrap(), "SOUND/");
		assert_eq!(install_relative("{install}/", "c").unwrap(), "");
		assert!(install_relative("{start_menu}/x", "c").is_err());
	}
}
