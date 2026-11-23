// `liminst build -t <os-arch>:web`: the staged runner bundle, with a placeholder or a named runner.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../../examples")
		.join(name)
}

fn build(name: &str, target: &str, out: &Path, extra: &[&str]) -> std::process::Output {
	Command::new(env!("CARGO_BIN_EXE_liminst"))
		.arg("-C")
		.arg(example(name))
		.args(["build", "-t", target, "--version", "1.0.0", "-o"])
		.arg(out)
		.args(extra)
		.output()
		.unwrap()
}

fn manifest(dir: &Path) -> Value {
	serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap()
}

#[test]
fn a_web_target_stages_the_bundle_with_a_placeholder_runner() {
	for (name, target, look, window) in [
		("lt-fizz", "windows-x86_64:web", "win11", (1100, 720)),
		("planboard", "macos-aarch64:web", "afterglow", (1024, 720)),
	] {
		let tmp = tempfile::tempdir().unwrap();
		let o = build(name, target, tmp.path(), &[]);
		assert!(
			o.status.success(),
			"{name}: {}",
			String::from_utf8_lossy(&o.stderr)
		);
		let dir = tmp.path().join("web");
		assert!(dir.join("payload.lip").is_file(), "{name}");
		assert!(dir.join("liminal-runner.placeholder").is_file(), "{name}");
		assert!(
			dir.join("project/installer/installer.toml").is_file(),
			"{name}"
		);
		assert!(
			dir.join("project/installer/flows/install.flow.json")
				.is_file(),
			"{name}"
		);
		let m = manifest(&dir);
		assert_eq!(m["target"], target);
		assert_eq!(m["look"], look);
		assert_eq!(m["window"]["width"], window.0);
		assert_eq!(m["window"]["height"], window.1);
		assert_eq!(m["runner"]["placeholder"], true);
		assert_eq!(m["payload"]["file"], "payload.lip");
		let size = std::fs::metadata(dir.join("payload.lip")).unwrap().len();
		assert_eq!(m["payload"]["size"], size);
		let stdout = String::from_utf8_lossy(&o.stdout);
		assert!(stdout.contains("web: "), "{stdout}");
	}
}

#[test]
fn a_named_runner_is_carried_in_the_bundle() {
	let tmp = tempfile::tempdir().unwrap();
	let runner = tmp.path().join("my-runner");
	std::fs::write(&runner, b"#!/bin/sh\nexit 0\n").unwrap();
	let out = tmp.path().join("out");
	let o = build(
		"lt-fizz",
		"windows-x86_64:web",
		&out,
		&["--web-runner", runner.to_str().unwrap()],
	);
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	let dir = out.join("web");
	assert!(
		dir.join("liminal-runner.exe").is_file(),
		"a Windows target gets .exe"
	);
	assert!(!dir.join("liminal-runner.placeholder").exists());
	let m = manifest(&dir);
	assert_eq!(m["runner"]["placeholder"], false);
	assert_eq!(m["runner"]["file"], "liminal-runner.exe");
	assert_eq!(m["runner"]["sha256"].as_str().map(str::len), Some(64));
}

#[test]
fn the_bundled_payload_names_the_web_target() {
	let tmp = tempfile::tempdir().unwrap();
	let o = build("lt-fizz", "windows-x86_64:web", tmp.path(), &[]);
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	let bytes = std::fs::read(tmp.path().join("web/payload.lip")).unwrap();
	let lip = li_pack::Lip::parse(&bytes).unwrap();
	assert_eq!(
		li_engine::bundle::bundled_target(&lip).as_deref(),
		Some("windows-x86_64:web")
	);
}

#[test]
fn an_explicit_output_list_without_web_skips_the_bundle() {
	let tmp = tempfile::tempdir().unwrap();
	let o = build(
		"lt-fizz",
		"windows-x86_64:web",
		tmp.path(),
		&["--output", "msix"],
	);
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	assert!(!tmp.path().join("web").exists());
	assert!(tmp.path().join("AppxManifest.xml").is_file());
}
