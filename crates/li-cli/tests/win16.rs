// `liminst build -t win16-x86`: the pack, the optional runtime and the report when there is none.
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

fn build(project: &Path, out: &Path, runtime: Option<&Path>) -> std::process::Output {
	let mut c = Command::new(env!("CARGO_BIN_EXE_liminst"));
	c.arg("-C")
		.arg(project)
		.args(["build", "-t", "win16-x86", "--version", "1.0.0"]);
	if let Some(r) = runtime {
		c.arg("--win16-runtime").arg(r);
	}
	// Stay out of the runtime lookup that walks up from the current folder.
	c.env_remove("LIMINST_WIN16_RUNTIME")
		.current_dir(out.parent().unwrap())
		.arg("-o")
		.arg(out)
		.output()
		.unwrap()
}

#[test]
fn builds_the_pack_and_says_when_there_is_no_runtime() {
	let tmp = tempfile::tempdir().unwrap();
	let out = tmp.path().join("out");
	let o = build(&example("lt-fizz"), &out, None);
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	let said = String::from_utf8_lossy(&o.stdout);
	assert!(said.contains("no Windows 3.1 SETUP.EXE"), "{said}");
	let dat = std::fs::read(out.join("win16/SETUP.LPK")).unwrap();
	assert_eq!(&dat[..4], b"LPK\x1a");
	assert!(!out.join("win16/SETUP.EXE").exists());
	let readme = std::fs::read_to_string(out.join("win16/README.TXT")).unwrap();
	assert!(readme.contains("Windows 3.1"), "{readme}");
}

#[test]
fn carries_a_runtime_when_given_one() {
	let tmp = tempfile::tempdir().unwrap();
	let runtime = tmp.path().join("SETUP.EXE");
	std::fs::write(&runtime, vec![0x4du8; 30_000]).unwrap();
	let out = tmp.path().join("out");
	let o = build(&example("lt-fizz"), &out, Some(&runtime));
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	assert_eq!(
		std::fs::read(out.join("win16/SETUP.EXE")).unwrap().len(),
		30_000
	);
	let img = std::fs::read(out.join("lt-fizz-1.0.0-win16-x86-1440.img")).unwrap();
	let names: Vec<String> = li_package::dos::read_floppy(&img)
		.into_iter()
		.map(|(n, _)| n)
		.collect();
	assert_eq!(names, ["SETUP.EXE", "SETUP.LPK", "README.TXT"]);
}

#[test]
fn planboard_has_a_windows_3_1_target_with_8_3_files() {
	let tmp = tempfile::tempdir().unwrap();
	let out = tmp.path().join("out");
	let o = build(&example("planboard"), &out, None);
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	assert!(out.join("win16/SETUP.LPK").is_file());
}
