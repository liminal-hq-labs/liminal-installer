// `liminst build -t windows-x86`: the files, the zip and the floppy image for the Win32 runtime.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};
use std::process::Command;

fn example(name: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../../examples")
		.join(name)
}

fn build(project: &Path, out: &Path, runtime: &Path) -> std::process::Output {
	Command::new(env!("CARGO_BIN_EXE_liminst"))
		.args(["-C"])
		.arg(project)
		.args([
			"build",
			"-t",
			"windows-x86",
			"--version",
			"1.0.0",
			"--win32-runtime",
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
	std::fs::write(&runtime, vec![0x4du8; 140_000]).unwrap();
	let out = tmp.path().join("out");
	let o = build(&example("lt-fizz"), &out, &runtime);
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	let said = String::from_utf8_lossy(&o.stdout);
	assert!(
		said.contains("zip:") && said.contains("floppy-1440:"),
		"{said}"
	);
	let lpk = std::fs::read(out.join("win32/SETUP.LPK")).unwrap();
	assert_eq!(&lpk[..4], b"LPK\x1a");
	assert_eq!(
		std::fs::read(out.join("win32/SETUP.EXE")).unwrap().len(),
		140_000
	);
	let readme = std::fs::read_to_string(out.join("win32/README.TXT")).unwrap();
	assert!(readme.contains("Add/Remove Programs"), "{readme}");
	let img = std::fs::read(out.join("lt-fizz-1.0.0-windows-x86-1440.img")).unwrap();
	let files = li_package::dos::read_floppy(&img);
	let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
	assert_eq!(names, ["SETUP.EXE", "SETUP.LPK", "README.TXT"]);
	assert_eq!(files[1].1, lpk);
	assert!(out.join("lt-fizz-1.0.0-windows-x86.zip").is_file());
}

#[test]
fn a_long_name_is_kept_in_the_pack() {
	let tmp = tempfile::tempdir().unwrap();
	let runtime = tmp.path().join("SETUP.EXE");
	std::fs::write(&runtime, b"MZ").unwrap();
	let out = tmp.path().join("out");
	let o = build(&example("planboard"), &out, &runtime);
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	let lpk = std::fs::read(out.join("win32/SETUP.LPK")).unwrap();
	let text = String::from_utf8_lossy(&lpk);
	assert!(text.contains("GETTING-STARTED.md") && text.contains("pbcli.exe"));
}

#[test]
fn a_missing_runtime_is_a_clear_error() {
	let tmp = tempfile::tempdir().unwrap();
	let o = build(
		&example("lt-fizz"),
		&tmp.path().join("out"),
		&tmp.path().join("nope.exe"),
	);
	assert!(!o.status.success());
	let err = String::from_utf8_lossy(&o.stderr);
	assert!(err.contains("cannot read the Win32 runtime"), "{err}");
}
