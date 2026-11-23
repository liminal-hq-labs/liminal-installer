// The runner's command line.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use assert_cmd::Command;
use predicates::prelude::*;

fn engine() -> Command {
	Command::cargo_bin("liminal-engine").unwrap()
}

#[test]
fn help_lists_the_lifecycle_commands() {
	let out = engine().arg("--help").output().unwrap();
	let text = String::from_utf8_lossy(&out.stdout);
	for word in [
		"install",
		"update",
		"modify",
		"repair",
		"uninstall",
		"launch",
		"--headless",
		"--answer",
		"Exit codes",
	] {
		assert!(text.contains(word), "help lacks `{word}`");
	}
}

#[test]
fn install_without_a_payload_fails_cleanly() {
	let dir = tempfile::tempdir().unwrap();
	engine()
		.args(["--headless", "install", "--from"])
		.arg(dir.path())
		.assert()
		.code(1)
		.stderr(predicate::str::contains("payload.lip"));
}

#[test]
fn commands_on_something_not_installed_fail_cleanly() {
	let dir = tempfile::tempdir().unwrap();
	for cmd in ["repair", "uninstall", "launch"] {
		engine()
			.args(["--headless", "--install-dir"])
			.arg(dir.path())
			.arg(cmd)
			.assert()
			.code(1)
			.stderr(predicate::str::contains("not installed"));
	}
}

#[test]
fn a_bad_answer_is_a_failure() {
	let dir = tempfile::tempdir().unwrap();
	engine()
		.args(["--headless", "--answer", "oops", "install", "--from"])
		.arg(dir.path())
		.assert()
		.failure();
}
