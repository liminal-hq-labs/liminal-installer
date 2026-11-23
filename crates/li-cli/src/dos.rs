// `liminst build -t dos-x86`: compiles the project, finds or builds the runtime, writes the DOS outputs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A `tui-retro` target has these outputs:
//!
//! | Output | Is |
//! |---|---|
//! | `files` (always) | `dos/` beside the other outputs: `SETUP.EXE`, `SETUP.DAT` and `README.TXT` as they run |
//! | `zip` | those three files in a zip |
//! | `floppy-1440` | those three files on a FAT12 1.44 MB image, or a clear note when they do not fit (disk spanning is not supported) |
//!
//! `SETUP.EXE` is the runtime (`dos/` in this repository). It comes from `--dos-runtime`, the
//! `LIMINST_DOS_RUNTIME` variable, a `dos/dist/SETUP.EXE` found by walking up from the current
//! folder, or a build with `dos/build.sh` when the DOS tools are present.

use std::path::{Path, PathBuf};
use std::process::Command;

use li_package::PackageError;
use li_package::dos::{DosImageError, floppy_1440, zip};

use crate::build::{BuildError, BuildRequest, BuiltArtefact};

fn other(m: impl Into<String>) -> BuildError {
	BuildError::Other(m.into())
}

/// Finds the repository's `dos/` folder by walking up from `start`.
fn find_dos_dir(start: &Path) -> Option<PathBuf> {
	start
		.ancestors()
		.map(|d| d.join("dos"))
		.find(|d| d.join("build.sh").is_file())
}

/// The runtime, `SETUP.EXE`: the named file, or one found or built.
fn runtime(req: &BuildRequest) -> Result<Vec<u8>, BuildError> {
	let from_env = std::env::var_os("LIMINST_DOS_RUNTIME").map(PathBuf::from);
	if let Some(p) = req.dos_runtime.clone().or(from_env) {
		return std::fs::read(&p)
			.map_err(|e| other(format!("cannot read the DOS runtime {}: {e}", p.display())));
	}
	let mut starts = vec![
		std::env::current_dir().unwrap_or_default(),
		req.project.root.clone(),
	];
	if let Ok(exe) = std::env::current_exe() {
		starts.extend(exe.parent().map(Path::to_path_buf));
	}
	for s in &starts {
		if let Some(dos) = find_dos_dir(s) {
			let setup = dos.join("dist/SETUP.EXE");
			if !setup.is_file() {
				let status = Command::new("bash").arg(dos.join("build.sh")).status();
				match status {
					Ok(s) if s.success() => {}
					_ => {
						return Err(other(format!(
							"the DOS runtime is not built and {} could not build it; run `dos/tools/fetch-tools.sh` and `dos/build.sh`, or pass --dos-runtime PATH to a prebuilt SETUP.EXE",
							dos.join("build.sh").display()
						)));
					}
				}
			}
			return std::fs::read(&setup).map_err(|e| other(format!("{}: {e}", setup.display())));
		}
	}
	Err(other(
		"no DOS runtime: pass --dos-runtime PATH to a prebuilt SETUP.EXE, set LIMINST_DOS_RUNTIME, or run from a checkout that has dos/build.sh",
	))
}

fn readme(name: &str, version: &str) -> String {
	format!(
		"{name} {version}\r\n\r\nTo install, run SETUP from this disk or folder.\r\nTo remove it, run SETUP /U.\r\n\r\nSETUP needs a 386 or later, DOS 5 or later and 2 MB of extended memory.\r\nKeep SETUP.EXE and SETUP.DAT together.\r\n"
	)
}

pub fn slug(req: &BuildRequest) -> String {
	let id = &req.project.installer.product.id;
	id.rsplit('.').next().unwrap_or(id).to_owned()
}

/// Builds the outputs of a `tui-retro` target.
pub fn build(
	req: &BuildRequest,
	outputs: &[String],
	dir: &Path,
) -> Result<Vec<BuiltArtefact>, BuildError> {
	let compiled = li_retro_build::compile(req.project, &req.target.id, &req.version)
		.map_err(|e| other(e.to_string()))?;
	let dat = compiled.dat().map_err(|e| other(e.to_string()))?;
	let exe = runtime(req)?;
	let readme = readme(&req.project.installer.product.name, &req.version).into_bytes();
	let files = vec![
		("SETUP.EXE".to_owned(), exe),
		("SETUP.DAT".to_owned(), dat),
		("README.TXT".to_owned(), readme),
	];
	let loose = dir.join("dos");
	std::fs::create_dir_all(&loose).map_err(|e| PackageError::io(&loose, e))?;
	for (n, d) in &files {
		let p = loose.join(n);
		std::fs::write(&p, d).map_err(|e| PackageError::io(&p, e))?;
	}
	let mut built = vec![BuiltArtefact {
		kind: "files".into(),
		path: Some(loose),
		note: Some(format!(
			"SETUP.EXE, SETUP.DAT and README.TXT; {} bytes installed from {} compressed",
			compiled.install_bytes(),
			compiled.payload.len()
		)),
	}];
	for w in &compiled.warnings {
		built.push(BuiltArtefact {
			kind: "warning".into(),
			path: None,
			note: Some(w.clone()),
		});
	}
	let base = format!("{}-{}-{}", slug(req), req.version, req.target.id);
	for name in outputs {
		match name.as_str() {
			"zip" => {
				let out = dir.join(format!("{base}.zip"));
				std::fs::write(&out, zip(&files)).map_err(|e| PackageError::io(&out, e))?;
				built.push(BuiltArtefact {
					kind: "zip".into(),
					path: Some(out),
					note: None,
				});
			}
			"floppy-1440" => {
				let label: String = slug(req)
					.chars()
					.filter(char::is_ascii_alphanumeric)
					.collect();
				match floppy_1440(&label, &files) {
					Ok(img) => {
						let out = dir.join(format!("{base}-1440.img"));
						std::fs::write(&out, img).map_err(|e| PackageError::io(&out, e))?;
						built.push(BuiltArtefact {
							kind: "floppy-1440".into(),
							path: Some(out),
							note: None,
						});
					}
					Err(e @ DosImageError::DoesNotFit { .. }) => built.push(BuiltArtefact {
						kind: "floppy-1440".into(),
						path: None,
						note: Some(format!("not built: {e}")),
					}),
					Err(e) => return Err(other(e.to_string())),
				}
			}
			"files" => {}
			other_name => built.push(BuiltArtefact {
				kind: other_name.to_owned(),
				path: None,
				note: Some(format!(
					"`{other_name}` is not an output of a DOS target (use zip or floppy-1440); skipped"
				)),
			}),
		}
	}
	Ok(built)
}
