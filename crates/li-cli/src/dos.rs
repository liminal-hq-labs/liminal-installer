// `liminst build -t dos-x86`: compiles the project, finds or builds the runtime, writes the DOS outputs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The same code builds a `windows-x86` target with the `win32-classic` renderer (`win32/`,
//! `--win32-runtime`, `LIMINST_WIN32_RUNTIME`): `SETUP.EXE` is then the Win32 runtime and the
//! outputs are `win32/` beside the others, a zip and a floppy image.
//!
//! A `tui-retro` target has these outputs:
//!
//! | Output | Is |
//! |---|---|
//! | `files` (always) | `dos/` beside the other outputs: `SETUP.EXE`, `SETUP.LPK` and `README.TXT` as they run |
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

/// Which runtime a target is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
	Dos,
	Win32,
	Win16,
}

impl Flavour {
	fn folder(self) -> &'static str {
		match self {
			Flavour::Dos => "dos",
			Flavour::Win32 => "win32",
			Flavour::Win16 => "win16",
		}
	}
	fn label(self) -> &'static str {
		match self {
			Flavour::Dos => "DOS",
			Flavour::Win32 => "Win32",
			Flavour::Win16 => "Windows 3.1",
		}
	}
}

fn other(m: impl Into<String>) -> BuildError {
	BuildError::Other(m.into())
}

/// Finds the repository's `dos/` folder by walking up from `start`.
fn find_dos_dir(start: &Path, flavour: Flavour) -> Option<PathBuf> {
	start
		.ancestors()
		.map(|d| d.join(flavour.folder()))
		.find(|d| d.join("build.sh").is_file())
}

/// The runtime, `SETUP.EXE`: the named file, or one found or built.
fn runtime(req: &BuildRequest, flavour: Flavour) -> Result<Vec<u8>, BuildError> {
	let (var, named, label) = match flavour {
		Flavour::Dos => ("LIMINST_DOS_RUNTIME", &req.dos_runtime, "DOS"),
		Flavour::Win32 => ("LIMINST_WIN32_RUNTIME", &req.win32_runtime, "Win32"),
		Flavour::Win16 => ("LIMINST_WIN16_RUNTIME", &req.win16_runtime, "Windows 3.1"),
	};
	let from_env = std::env::var_os(var).map(PathBuf::from);
	if let Some(p) = named.clone().or(from_env) {
		return std::fs::read(&p).map_err(|e| {
			other(format!(
				"cannot read the {label} runtime {}: {e}",
				p.display()
			))
		});
	}
	let mut starts = vec![
		std::env::current_dir().unwrap_or_default(),
		req.project.root.clone(),
	];
	if let Ok(exe) = std::env::current_exe() {
		starts.extend(exe.parent().map(Path::to_path_buf));
	}
	for s in &starts {
		if let Some(dos) = find_dos_dir(s, flavour) {
			let setup = dos.join("dist/SETUP.EXE");
			if !setup.is_file() {
				let status = Command::new("bash").arg(dos.join("build.sh")).status();
				match status {
					Ok(s) if s.success() => {}
					_ => {
						return Err(other(format!(
							"the {label} runtime is not built and {} could not build it; build it with `{0}/build.sh`, or pass --{1}-runtime PATH to a prebuilt SETUP.EXE",
							dos.join("build.sh").display(),
							flavour.folder()
						)));
					}
				}
			}
			return std::fs::read(&setup).map_err(|e| other(format!("{}: {e}", setup.display())));
		}
	}
	Err(other(format!(
		"no {label} runtime: pass --{0}-runtime PATH to a prebuilt SETUP.EXE, set {var}, or run from a checkout that has {0}/build.sh",
		flavour.folder()
	)))
}

/// The Windows 3.1 runtime, if there is one: the named file, else `retro/win16/out/SETUP.EXE`
/// found by walking up. `SETUP.EXE` is built by `retro/win16/build.sh`; without one `build` says so.
fn win16_runtime(req: &BuildRequest) -> Result<Option<Vec<u8>>, BuildError> {
	let from_env = std::env::var_os("LIMINST_WIN16_RUNTIME").map(PathBuf::from);
	if let Some(p) = req.win16_runtime.clone().or(from_env) {
		return std::fs::read(&p).map(Some).map_err(|e| {
			other(format!(
				"cannot read the Windows 3.1 runtime {}: {e}",
				p.display()
			))
		});
	}
	let mut starts = vec![
		std::env::current_dir().unwrap_or_default(),
		req.project.root.clone(),
	];
	if let Ok(exe) = std::env::current_exe() {
		starts.extend(exe.parent().map(Path::to_path_buf));
	}
	for s in &starts {
		for d in s.ancestors() {
			let p = d.join("retro/win16/out/SETUP.EXE");
			if p.is_file() {
				return std::fs::read(&p)
					.map(Some)
					.map_err(|e| other(format!("{}: {e}", p.display())));
			}
		}
	}
	Ok(None)
}

fn readme(name: &str, version: &str, flavour: Flavour) -> String {
	if flavour == Flavour::Win16 {
		return format!(
			"{name} {version}\r\n\r\nTo install, choose File, Run in Program Manager and run SETUP from this disk or folder.\r\nTo remove it, run SETUP /U.\r\n\r\nSETUP needs Windows 3.1 or 3.11 in 386 Enhanced mode.\r\nKeep SETUP.EXE and SETUP.LPK together.\r\n"
		);
	}
	if flavour == Flavour::Win32 {
		return format!(
			"{name} {version}\r\n\r\nTo install, run SETUP.EXE from this disk or folder.\r\nTo remove it, use Add/Remove Programs, or run SETUP.EXE /U.\r\n\r\nSETUP needs Windows 95 or later (a Pentium or better). Keep SETUP.EXE and SETUP.LPK together.\r\n"
		);
	}
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
	build_for(req, outputs, dir, Flavour::Dos)
}

/// Builds the outputs of a `tui-retro` (DOS) or `win32-classic` (Win32) target.
pub fn build_for(
	req: &BuildRequest,
	outputs: &[String],
	dir: &Path,
	flavour: Flavour,
) -> Result<Vec<BuiltArtefact>, BuildError> {
	let compiled = match flavour {
		Flavour::Dos => li_retro_build::compile(req.project, &req.target.id, &req.version),
		Flavour::Win32 => li_retro_build::compile_win32(req.project, &req.target.id, &req.version),
		Flavour::Win16 => li_retro_build::compile_win16(req.project, &req.target.id, &req.version),
	}
	.map_err(|e| other(e.to_string()))?;
	let dat = compiled.lpk().map_err(|e| other(e.to_string()))?;
	let pack_len = dat.len();
	let exe = if flavour == Flavour::Win16 {
		win16_runtime(req)?
	} else {
		Some(runtime(req, flavour)?)
	};
	let readme = readme(&req.project.installer.product.name, &req.version, flavour).into_bytes();
	let mut files = Vec::new();
	if let Some(exe) = exe {
		files.push(("SETUP.EXE".to_owned(), exe));
	}
	files.push(("SETUP.LPK".to_owned(), dat));
	files.push(("README.TXT".to_owned(), readme));
	let has_exe = files.iter().any(|(n, _)| n == "SETUP.EXE");
	let loose = dir.join(flavour.folder());
	std::fs::create_dir_all(&loose).map_err(|e| PackageError::io(&loose, e))?;
	for (n, d) in &files {
		let p = loose.join(n);
		std::fs::write(&p, d).map_err(|e| PackageError::io(&p, e))?;
	}
	let mut built = vec![BuiltArtefact {
		kind: "files".into(),
		path: Some(loose),
		note: Some(format!(
			"{}SETUP.LPK and README.TXT; {} bytes installed from a {}-byte pack",
			if has_exe { "SETUP.EXE, " } else { "" },
			compiled.install_bytes(),
			pack_len
		)),
	}];
	if flavour == Flavour::Win16 && !has_exe {
		built.push(BuiltArtefact {
			kind: "warning".into(),
			path: None,
			note: Some(
				"no Windows 3.1 SETUP.EXE: build it with retro/win16/build.sh (see docs/retro-win16.md) and pass --win16-runtime PATH to carry it".into(),
			),
		});
	}
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
					"`{other_name}` is not an output of a {} target (use zip or floppy-1440); skipped",
					flavour.label()
				)),
			}),
		}
	}
	Ok(built)
}
