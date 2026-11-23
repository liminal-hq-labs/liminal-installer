// `liminst lpk`: pack a folder, list it, verify it, damage it, unpack it, and span a set.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use assert_cmd::Command;
use predicates::prelude::*;

fn liminst() -> Command {
	Command::cargo_bin("liminst").unwrap()
}

fn tree(root: &std::path::Path) {
	std::fs::create_dir_all(root.join("sound")).unwrap();
	std::fs::create_dir_all(root.join("empty")).unwrap();
	std::fs::write(root.join("readme.txt"), "hello\r\n".repeat(500)).unwrap();
	std::fs::write(root.join("A long file name.dat"), vec![7u8; 3000]).unwrap();
	std::fs::write(
		root.join("sound/adlib.drv"),
		(0..9000u32)
			.map(|i| (i * 31 % 251) as u8)
			.collect::<Vec<_>>(),
	)
	.unwrap();
}

#[test]
fn pack_list_verify_and_unpack() {
	let t = tempfile::tempdir().unwrap();
	let src = t.path().join("src");
	tree(&src);
	let pack = t.path().join("OUT.LPK");
	liminst()
		.args(["lpk", "pack"])
		.arg(&src)
		.arg("-o")
		.arg(&pack)
		.arg("--fixed-time")
		.assert()
		.success()
		.stdout(predicate::str::contains("1 volume"));
	liminst()
		.args(["lpk", "list"])
		.arg(&pack)
		.assert()
		.success()
		.stdout(predicate::str::contains("SOUND/ADLIB.DRV"))
		.stdout(predicate::str::contains("A long file name.dat"))
		.stdout(predicate::str::contains("4 files"));
	liminst()
		.args(["lpk", "verify"])
		.arg(&pack)
		.assert()
		.success()
		.stdout(predicate::str::contains("every checksum matches"));
	let out = t.path().join("out");
	liminst()
		.args(["lpk", "unpack"])
		.arg(&pack)
		.arg("-o")
		.arg(&out)
		.assert()
		.success();
	assert_eq!(
		std::fs::read(out.join("readme.txt")).unwrap(),
		std::fs::read(src.join("readme.txt")).unwrap()
	);
	assert_eq!(
		std::fs::read(out.join("A long file name.dat")).unwrap(),
		vec![7u8; 3000]
	);
	assert!(out.join("empty").is_dir());
	assert!(out.join("sound/adlib.drv").is_file());
	// Reproducible with a fixed time.
	let again = t.path().join("AGAIN.LPK");
	liminst()
		.args(["lpk", "pack"])
		.arg(&src)
		.arg("-o")
		.arg(&again)
		.arg("--fixed-time")
		.assert()
		.success();
	assert_eq!(
		std::fs::read(&pack).unwrap(),
		std::fs::read(&again).unwrap()
	);
}

#[test]
fn a_damaged_pack_fails_verify_with_exit_code_two() {
	let t = tempfile::tempdir().unwrap();
	let src = t.path().join("src");
	tree(&src);
	let pack = t.path().join("OUT.LPK");
	liminst()
		.args(["lpk", "pack"])
		.arg(&src)
		.arg("-o")
		.arg(&pack)
		.assert()
		.success();
	let mut b = std::fs::read(&pack).unwrap();
	let at = b.len() - 100;
	b[at] ^= 0x55;
	std::fs::write(&pack, b).unwrap();
	liminst()
		.args(["lpk", "verify"])
		.arg(&pack)
		.assert()
		.code(2)
		.stderr(predicate::str::contains("checksum").or(predicate::str::contains("damaged")));
}

#[test]
fn a_set_spans_volumes_that_unpack_in_order() {
	let t = tempfile::tempdir().unwrap();
	let src = t.path().join("src");
	std::fs::create_dir_all(&src).unwrap();
	for i in 0..6u32 {
		let mut x = i * 977 + 12345;
		let data: Vec<u8> = (0..30_000)
			.map(|_| {
				x ^= x << 13;
				x ^= x >> 17;
				x ^= x << 5;
				(x >> 9) as u8
			})
			.collect();
		std::fs::write(src.join(format!("F{i}.DAT")), data).unwrap();
	}
	let pack = t.path().join("SET.LPK");
	liminst()
		.args(["lpk", "pack"])
		.arg(&src)
		.arg("-o")
		.arg(&pack)
		.args(["--volume-size", "70000"])
		.assert()
		.success()
		.stdout(predicate::str::contains("SET.L02"));
	assert!(t.path().join("SET.L03").is_file());
	liminst()
		.args(["lpk", "verify"])
		.arg(&pack)
		.assert()
		.success();
	// With the second disk missing the check says so.
	std::fs::remove_file(t.path().join("SET.L02")).unwrap();
	liminst()
		.args(["lpk", "verify"])
		.arg(&pack)
		.assert()
		.code(2)
		.stderr(predicate::str::contains("disk"));
}
