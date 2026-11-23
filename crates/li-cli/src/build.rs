// `liminst build`: writes a target's outputs through a `Builder`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};
use std::process::Command;

use li_model::project::VersionSource;
use li_model::{Scope, Target};
use li_package::{
	Content, Entry, PackageError, PackageInfo, arch, deb, freebsd, macos, msix, rpm, run, wix,
};

use crate::project::Project;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
	#[error("{0}")]
	Package(#[from] PackageError),
	#[error("{0}")]
	Other(String),
}

/// Everything a builder needs to write one target.
pub struct BuildRequest<'a> {
	pub project: &'a Project,
	pub target: &'a Target,
	pub version: String,
	pub out_dir: PathBuf,
	/// Output names to build; empty means the target's `outputs`, else the platform defaults.
	pub outputs: Vec<String>,
	/// The engine binary the themed installer hands over to, if any.
	pub engine: Option<PathBuf>,
}

/// One file (or tree) a build wrote, or an output it could not write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltArtefact {
	/// The output name, such as `run` or `deb`.
	pub kind: String,
	pub path: Option<PathBuf>,
	/// What was skipped or left to do, if anything.
	pub note: Option<String>,
}

/// Writes a target's outputs. [`NativeBuilder`] is the implementation that uses `li-package`;
/// a themed-installer builder that links the engine and renderer would implement this too.
pub trait Builder {
	fn build(&self, req: &BuildRequest) -> Result<Vec<BuiltArtefact>, BuildError>;
}

/// Writes the self-extracting `.run` and the native package outputs from `li-package`. Outputs
/// that need another toolchain (floppy images, `setup.exe`, signing) are reported as skipped.
pub struct NativeBuilder;

/// The outputs built for a target that lists none.
pub fn default_outputs(target_id: &str) -> Vec<String> {
	let os = target_id.split('-').next().unwrap_or_default();
	match os {
		"linux" => vec!["run", "deb"],
		"windows" => vec!["msi", "msix"],
		"macos" => vec!["pkg"],
		"freebsd" => vec!["run", "pkg"],
		_ => vec![],
	}
	.into_iter()
	.map(str::to_owned)
	.collect()
}

/// Reads the product version: an explicit value, a literal, or the latest git tag.
pub fn resolve_version(project: &Project, explicit: Option<&str>) -> Result<String, BuildError> {
	if let Some(v) = explicit {
		return Ok(v.trim_start_matches('v').to_owned());
	}
	match &project.installer.product.version {
		VersionSource::Literal(v) => Ok(v.clone()),
		VersionSource::From { from } if from == "git-tag" => {
			let out = Command::new("git")
				.args(["describe", "--tags", "--abbrev=0"])
				.current_dir(&project.root)
				.output();
			match out {
				Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout)
					.trim()
					.trim_start_matches('v')
					.to_owned()),
				_ => Err(BuildError::Other(
					"the version comes from a git tag, but none was found; pass --version".into(),
				)),
			}
		}
		VersionSource::From { from } => Err(BuildError::Other(format!(
			"cannot read the version from `{from}`; pass --version"
		))),
	}
}

impl Builder for NativeBuilder {
	fn build(&self, req: &BuildRequest) -> Result<Vec<BuiltArtefact>, BuildError> {
		let mut info = PackageInfo::from_project(
			&req.project.installer,
			&req.project.components,
			req.target,
			&req.version,
			&req.project.dir,
		)?;
		info.licence = req.project.installer.product.licence.clone();
		if let Some(engine) = &req.engine {
			info.files.push(Entry {
				dest: "engine/liminal-engine".into(),
				content: Content::File(engine.clone()),
				executable: true,
			});
			info.engine = Some("engine/liminal-engine".into());
		}
		let outputs = if !req.outputs.is_empty() {
			req.outputs.clone()
		} else if !req.target.outputs.is_empty() {
			req.target.outputs.clone()
		} else {
			default_outputs(&req.target.id)
		};
		let dir = &req.out_dir;
		std::fs::create_dir_all(dir).map_err(|e| PackageError::io(dir, e))?;
		let mut built = Vec::new();
		for name in &outputs {
			built.push(build_one(name, &info, req, dir)?);
		}
		Ok(built)
	}
}

fn write(path: &Path, text: &str) -> Result<(), BuildError> {
	std::fs::write(path, text).map_err(|e| PackageError::io(path, e).into())
}

fn done(kind: &str, path: PathBuf, note: Option<&str>) -> BuiltArtefact {
	BuiltArtefact {
		kind: kind.to_owned(),
		path: Some(path),
		note: note.map(str::to_owned),
	}
}

fn build_one(
	name: &str,
	info: &PackageInfo,
	req: &BuildRequest,
	dir: &Path,
) -> Result<BuiltArtefact, BuildError> {
	let slug = info.slug();
	let v = &info.version;
	let os = req.target.id.split('-').next().unwrap_or_default();
	let mut native = info.clone();
	match name {
		"run" | "self-extracting" => {
			native.scope = if info.scope == Scope::Ask {
				Scope::User
			} else {
				info.scope
			};
			native.receipt_target = format!("{}:{}", req.target.id, req.target.renderer);
			let out = dir.join(format!("{slug}-{v}-{}.run", req.target.id));
			run::build(&native, &out)?;
			Ok(done("run", out, None))
		}
		"deb" => {
			native.scope = Scope::Machine;
			let tree = dir.join(format!("{slug}_{v}_{}-tree", info.arch.deb()));
			let out = dir.join(format!("{slug}_{v}_{}.deb", info.arch.deb()));
			if tree.exists() {
				std::fs::remove_dir_all(&tree).map_err(|e| PackageError::io(&tree, e))?;
			}
			match deb::build(&native, &tree, &out) {
				Ok(()) => Ok(done("deb", out, None)),
				Err(PackageError::ToolMissing(t)) => Ok(done(
					"deb",
					tree,
					Some(&format!(
						"`{t}` is not installed; wrote the package tree only. Run `dpkg-deb --root-owner-group --build <tree> <out>.deb`"
					)),
				)),
				Err(e) => Err(e.into()),
			}
		}
		"rpm" => {
			native.scope = Scope::Machine;
			let spec = dir.join(format!("{slug}.spec"));
			let tarball = dir.join(format!("{}.tar.gz", info.source_dir()));
			write(&spec, &rpm::spec(&native))?;
			std::fs::write(&tarball, rpm::source_tarball(&native)?)
				.map_err(|e| PackageError::io(&tarball, e))?;
			Ok(done(
				"rpm",
				spec,
				Some(&format!(
					"wrote the spec and {}; run `rpmbuild -bb` with the tarball in SOURCES",
					tarball.display()
				)),
			))
		}
		"arch" | "pkgbuild" => {
			native.scope = Scope::Machine;
			let (bytes, sum) = arch::source_tarball(&native)?;
			let tarball = dir.join(format!("{}.tar.gz", native.source_dir()));
			std::fs::write(&tarball, bytes).map_err(|e| PackageError::io(&tarball, e))?;
			let pkgbuild = dir.join("PKGBUILD");
			write(&pkgbuild, &arch::pkgbuild(&native, &sum))?;
			Ok(done(
				"arch",
				pkgbuild,
				Some("wrote PKGBUILD and the source tarball; run `makepkg` in this folder"),
			))
		}
		"msi" | "wix" | "wxs" => {
			let out = dir.join(format!("{slug}.wxs"));
			write(&out, &wix::wxs(info)?)?;
			Ok(done(
				"msi",
				out,
				Some("wrote WiX source; compile with `wix build -d SourceDir=<files> <file>.wxs`"),
			))
		}
		"msix" => {
			let out = dir.join("AppxManifest.xml");
			write(&out, &msix::manifest(info))?;
			Ok(done(
				"msix",
				out,
				Some("wrote the manifest; package and sign with `makeappx` and `signtool`"),
			))
		}
		"pkg" if os == "freebsd" => {
			let out = dir.join("+MANIFEST");
			write(&out, &freebsd::manifest(info, 14)?)?;
			Ok(done(
				"pkg",
				out,
				Some("wrote +MANIFEST; run `pkg create -M +MANIFEST`"),
			))
		}
		"pkg" | "app" => {
			let xml = dir.join("distribution.xml");
			write(&xml, &macos::distribution_xml(info))?;
			let plist = dir.join("Info.plist");
			write(&plist, &macos::info_plist(info))?;
			Ok(done(
				"pkg",
				xml,
				Some(
					"wrote distribution.xml and Info.plist; build with `pkgbuild` and `productbuild`",
				),
			))
		}
		other => Ok(BuiltArtefact {
			kind: other.to_owned(),
			path: None,
			note: Some(format!("`{other}` is not available yet; skipped")),
		}),
	}
}
