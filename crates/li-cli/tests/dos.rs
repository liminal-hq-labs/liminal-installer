// `liminst build -t dos-x86`: the files, the zip and the floppy image, and the clear report when a payload does not fit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};
use std::process::Command;

fn fizz() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lt-fizz")
}

fn copy_dir(from: &Path, to: &Path) {
	std::fs::create_dir_all(to).unwrap();
	for e in std::fs::read_dir(from).unwrap().flatten() {
		let dest = to.join(e.file_name());
		if e.path().is_dir() {
			copy_dir(&e.path(), &dest);
		} else {
			std::fs::copy(e.path(), dest).unwrap();
		}
	}
}

fn build(project: &Path, out: &Path, runtime: &Path) -> std::process::Output {
	Command::new(env!("CARGO_BIN_EXE_liminst"))
		.args(["-C"])
		.arg(project)
		.args([
			"build",
			"-t",
			"dos-x86",
			"--version",
			"1.0.0",
			"--dos-runtime",
		])
		.arg(runtime)
		.arg("-o")
		.arg(out)
		.output()
		.unwrap()
}

#[test]
fn builds_the_files_the_zip_and_the_floppy() {
	let tmp = tempfile::tempdir().unwrap();
	let runtime = tmp.path().join("SETUP.EXE");
	std::fs::write(&runtime, vec![0x4du8; 90_000]).unwrap();
	let out = tmp.path().join("out");
	let o = build(&fizz(), &out, &runtime);
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	let said = String::from_utf8_lossy(&o.stdout);
	assert!(
		said.contains("zip:") && said.contains("floppy-1440:"),
		"{said}"
	);
	let dat = std::fs::read(out.join("dos/SETUP.LPK")).unwrap();
	assert_eq!(&dat[..4], b"LPK\x1a");
	assert_eq!(
		std::fs::read(out.join("dos/SETUP.EXE")).unwrap().len(),
		90_000
	);
	let img = std::fs::read(out.join("lt-fizz-1.0.0-dos-x86-1440.img")).unwrap();
	let files = li_package::dos::read_floppy(&img);
	let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
	assert_eq!(names, ["SETUP.EXE", "SETUP.LPK", "README.TXT"]);
	assert_eq!(files[1].1, dat);
	assert!(out.join("lt-fizz-1.0.0-dos-x86.zip").is_file());
}

#[test]
fn a_payload_that_does_not_fit_a_floppy_is_reported_and_the_zip_still_builds() {
	let tmp = tempfile::tempdir().unwrap();
	let project = tmp.path().join("project");
	copy_dir(&fizz(), &project);
	// Noise does not compress, so the payload stays large.
	let mut x = 0x9e37_79b9u32;
	let big: Vec<u8> = (0..1_600_000)
		.map(|_| {
			x ^= x << 13;
			x ^= x >> 17;
			x ^= x << 5;
			(x >> 8) as u8
		})
		.collect();
	std::fs::write(project.join("installer/dist/levels/EP1.LVL"), big).unwrap();
	let runtime = tmp.path().join("SETUP.EXE");
	std::fs::write(&runtime, vec![1u8; 1000]).unwrap();
	let out = tmp.path().join("out");
	let o = build(&project, &out, &runtime);
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	let said = String::from_utf8_lossy(&o.stdout);
	assert!(
		said.contains("floppy-1440: not built") && said.contains("not supported"),
		"{said}"
	);
	assert!(!out.join("lt-fizz-1.0.0-dos-x86-1440.img").exists());
	assert!(out.join("lt-fizz-1.0.0-dos-x86.zip").is_file());
}

#[test]
fn a_missing_runtime_says_what_to_do() {
	let tmp = tempfile::tempdir().unwrap();
	let o = Command::new(env!("CARGO_BIN_EXE_liminst"))
		.current_dir(tmp.path())
		.env_remove("LIMINST_DOS_RUNTIME")
		.arg("-C")
		.arg(fizz())
		.args(["build", "-t", "dos-x86", "--version", "1.0.0", "-o"])
		.arg(tmp.path().join("out"))
		.arg("--dos-runtime")
		.arg(tmp.path().join("nope.exe"))
		.output()
		.unwrap();
	assert!(!o.status.success());
	assert!(String::from_utf8_lossy(&o.stderr).contains("cannot read the DOS runtime"));
}

/// A product repository with its own `dos/build.sh` (no `setup/Cargo.toml`) is not the runtime:
/// the script is never run, and the error names the flag. The binary is copied beside the
/// project so the lookup that walks up from it cannot reach this repository's own `dos/`.
#[cfg(unix)]
#[test]
fn a_product_repositorys_own_build_script_is_never_run() {
	use std::os::unix::fs::PermissionsExt;
	let tmp = tempfile::tempdir().unwrap();
	let root = tmp.path().join("product");
	let project = root.join("project");
	copy_dir(&fizz(), &project);
	let marker = root.join("RAN");
	for folder in ["dos", "win32"] {
		std::fs::create_dir_all(root.join(folder)).unwrap();
		std::fs::write(
			root.join(folder).join("build.sh"),
			format!("#!/bin/sh\ntouch {}\n", marker.display()),
		)
		.unwrap();
	}
	let bin = root.join("bin");
	std::fs::create_dir_all(&bin).unwrap();
	let liminst = bin.join("liminst");
	std::fs::copy(env!("CARGO_BIN_EXE_liminst"), &liminst).unwrap();
	std::fs::set_permissions(&liminst, std::fs::Permissions::from_mode(0o755)).unwrap();
	for (target, flag) in [
		("dos-x86", "--dos-runtime"),
		("windows-x86", "--win32-runtime"),
	] {
		let o = Command::new(&liminst)
			.current_dir(&root)
			.env_remove("LIMINST_DOS_RUNTIME")
			.env_remove("LIMINST_WIN32_RUNTIME")
			.arg("-C")
			.arg(&project)
			.args(["build", "-t", target, "--version", "1.0.0", "-o"])
			.arg(root.join("out"))
			.output()
			.unwrap();
		assert!(!o.status.success());
		let said = String::from_utf8_lossy(&o.stderr);
		assert!(said.contains(flag), "{said}");
		assert!(!marker.exists(), "the product's own build script ran");
	}
}
