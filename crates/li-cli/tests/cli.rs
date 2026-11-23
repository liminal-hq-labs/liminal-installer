// End-to-end tests for the `liminst` binary.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;
use std::process::Command as Std;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn liminst(dir: &Path) -> Command {
	let mut c = Command::cargo_bin("liminst").unwrap();
	c.current_dir(dir);
	c
}

fn project() -> TempDir {
	let dir = tempfile::tempdir().unwrap();
	liminst(dir.path())
		.args([
			"init",
			"--id",
			"hq.test.demo",
			"--name",
			"Demo App",
			"--publisher",
			"Test Co",
		])
		.assert()
		.success();
	let payload = dir.path().join("installer/payload/linux-x86_64");
	std::fs::create_dir_all(&payload).unwrap();
	std::fs::write(payload.join("demo"), "#!/bin/sh\necho demo\n").unwrap();
	dir
}

fn json_of(out: &[u8]) -> Value {
	serde_json::from_slice(out)
		.unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(out)))
}

fn codes(report: &Value) -> Vec<String> {
	report["diagnostics"]
		.as_array()
		.unwrap()
		.iter()
		.map(|d| d["code"].as_str().unwrap().to_owned())
		.collect()
}

#[test]
fn help_lists_every_command() {
	Command::cargo_bin("liminst")
		.unwrap()
		.arg("--help")
		.assert()
		.success()
		.stdout(predicate::str::contains("init"))
		.stdout(predicate::str::contains("check"))
		.stdout(predicate::str::contains("preview"))
		.stdout(predicate::str::contains("build"))
		.stdout(predicate::str::contains("feed"))
		.stdout(predicate::str::contains("theme"))
		.stdout(predicate::str::contains("studio"))
		.stdout(predicate::str::contains("Exit codes"));
}

#[test]
fn init_scaffolds_a_project_that_checks_clean() {
	let dir = project();
	for f in [
		"installer/installer.toml",
		"installer/components.toml",
		"installer/flows/install.flow.json",
		"installer/themes/neutral/theme.toml",
		"installer/themes/neutral/layouts/welcome.layout.json",
	] {
		assert!(dir.path().join(f).is_file(), "{f} missing");
	}
	let out = liminst(dir.path())
		.args(["check", "--json"])
		.assert()
		.success()
		.get_output()
		.stdout
		.clone();
	let report = json_of(&out);
	assert_eq!(report["ok"], true);
	assert_eq!(report["errors"], 0);
	assert_eq!(report["targets"][0], "linux-x86_64");
}

#[test]
fn init_refuses_to_overwrite_without_force() {
	let dir = project();
	liminst(dir.path())
		.arg("init")
		.assert()
		.code(2)
		.stderr(predicate::str::contains("already exists"));
	liminst(dir.path())
		.args(["init", "--force", "--id", "hq.test.demo"])
		.assert()
		.success();
}

#[test]
fn check_without_a_project_exits_2() {
	let dir = tempfile::tempdir().unwrap();
	liminst(dir.path())
		.arg("check")
		.assert()
		.code(2)
		.stderr(predicate::str::contains("liminst init"));
}

#[test]
fn check_reports_a_missing_source_as_a_warning() {
	let dir = tempfile::tempdir().unwrap();
	liminst(dir.path())
		.args(["init", "--id", "hq.test.demo"])
		.assert()
		.success();
	let out = liminst(dir.path())
		.args(["check", "--json"])
		.assert()
		.success()
		.get_output()
		.stdout
		.clone();
	let report = json_of(&out);
	assert_eq!(report["errors"], 0);
	assert!(codes(&report).contains(&"missing-source".to_owned()));
}

#[test]
fn check_fails_on_a_broken_flow() {
	let dir = project();
	let path = dir.path().join("installer/flows/install.flow.json");
	let mut flow: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
	flow["steps"][1]["next"] = "nowhere".into();
	std::fs::write(&path, flow.to_string()).unwrap();
	let out = liminst(dir.path())
		.args(["check", "--json"])
		.assert()
		.code(1)
		.get_output()
		.stdout
		.clone();
	let report = json_of(&out);
	assert_eq!(report["ok"], false);
	assert!(codes(&report).contains(&"invalid-flow".to_owned()));
	liminst(dir.path())
		.arg("check")
		.assert()
		.code(1)
		.stdout(predicate::str::contains("points at unknown step `nowhere`"))
		.stdout(predicate::str::contains(
			"installer/flows/install.flow.json",
		));
}

#[test]
fn check_fails_on_a_missing_layout() {
	let dir = project();
	std::fs::remove_file(
		dir.path()
			.join("installer/themes/neutral/layouts/done.layout.json"),
	)
	.unwrap();
	let out = liminst(dir.path())
		.args(["check", "--json"])
		.assert()
		.code(1)
		.get_output()
		.stdout
		.clone();
	assert!(codes(&json_of(&out)).contains(&"missing-layout".to_owned()));
}

#[test]
fn check_fails_on_a_control_without_an_accessible_name() {
	let dir = project();
	let path = dir
		.path()
		.join("installer/themes/neutral/layouts/welcome.layout.json");
	let mut layout: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
	layout["root"]["children"][1]["children"][0]["props"] = serde_json::json!({});
	std::fs::write(&path, layout.to_string()).unwrap();
	let out = liminst(dir.path())
		.args(["check", "--json"])
		.assert()
		.code(1)
		.get_output()
		.stdout
		.clone();
	assert!(codes(&json_of(&out)).contains(&"no-accessible-name".to_owned()));
}

#[test]
fn check_fails_on_a_malformed_manifest() {
	let dir = project();
	std::fs::write(dir.path().join("installer/installer.toml"), "[product\n").unwrap();
	let out = liminst(dir.path())
		.args(["check", "--json"])
		.assert()
		.code(1)
		.get_output()
		.stdout
		.clone();
	assert!(codes(&json_of(&out)).contains(&"invalid-installer".to_owned()));
}

#[test]
fn check_rejects_an_unknown_target() {
	let dir = project();
	liminst(dir.path())
		.args(["check", "--target", "dos-x86"])
		.assert()
		.code(2)
		.stderr(predicate::str::contains("no target named `dos-x86`"))
		.stderr(predicate::str::contains("linux-x86_64"));
}

#[test]
fn preview_draws_each_step_and_prints_the_plan() {
	let dir = project();
	liminst(dir.path())
		.args(["preview", "--target", "linux-x86_64"])
		.assert()
		.success()
		.stdout(predicate::str::contains("== Step 1 of 4: welcome"))
		.stdout(predicate::str::contains("Demo App Setup"))
		.stdout(predicate::str::contains("[ Next ]").or(predicate::str::contains("[>Next<]")))
		.stdout(predicate::str::contains("== Plan (5 operations) =="))
		.stdout(predicate::str::contains(
			"copy           payload/linux-x86_64/demo -> {install}/bin/demo",
		))
		.stdout(predicate::str::contains("receipt.write"));
}

#[test]
fn preview_plan_only_and_json() {
	let dir = project();
	liminst(dir.path())
		.args(["preview", "-t", "linux-x86_64", "--only", "plan"])
		.assert()
		.success()
		.stdout(predicate::str::contains("Step 1").not());
	let out = liminst(dir.path())
		.args([
			"preview",
			"-t",
			"linux-x86_64",
			"--json",
			"--style",
			"retro",
		])
		.assert()
		.success()
		.get_output()
		.stdout
		.clone();
	let v = json_of(&out);
	assert_eq!(v["steps"].as_array().unwrap().len(), 4);
	assert_eq!(
		v["plan"]["ops"].as_array().unwrap().last().unwrap()["op"],
		"receipt.write"
	);
}

#[test]
fn preview_rejects_bad_input() {
	let dir = project();
	liminst(dir.path())
		.args(["preview", "-t", "linux-x86_64", "--size", "wide"])
		.assert()
		.code(2)
		.stderr(predicate::str::contains("COLSxROWS"));
	liminst(dir.path())
		.args(["preview", "-t", "linux-x86_64", "--flow", "update"])
		.assert()
		.code(2)
		.stderr(predicate::str::contains("no flow named `update`"));
}

#[test]
fn build_writes_a_run_file_that_installs() {
	let dir = project();
	liminst(dir.path())
		.args([
			"build",
			"-t",
			"linux-x86_64",
			"--version",
			"1.2.3",
			"--output",
			"run",
		])
		.assert()
		.success()
		.stdout(predicate::str::contains("run:"));
	let run = dir
		.path()
		.join("build/linux-x86_64/demo-1.2.3-linux-x86_64.run");
	assert!(run.is_file());
	let prefix = dir.path().join("prefix");
	let o = Std::new("sh")
		.arg(&run)
		.arg("--prefix")
		.arg(&prefix)
		.output()
		.unwrap();
	assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
	assert!(prefix.join("bin/demo").is_file());
	let receipt: Value =
		serde_json::from_slice(&std::fs::read(prefix.join(".liminal/receipt.json")).unwrap())
			.unwrap();
	assert_eq!(receipt["installed"], "1.2.3");
	assert_eq!(receipt["target"], "linux-x86_64:tui");
}

#[test]
fn build_writes_the_default_linux_outputs() {
	let dir = project();
	liminst(dir.path())
		.args(["build", "-t", "linux-x86_64", "--version", "v1.0.0"])
		.assert()
		.success();
	let out = dir.path().join("build/linux-x86_64");
	assert!(out.join("demo-1.0.0-linux-x86_64.run").is_file());
	// The `.deb` is written when dpkg-deb exists, otherwise the package tree is.
	assert!(
		out.join("demo_1.0.0_amd64.deb").is_file()
			|| out.join("demo_1.0.0_amd64-tree/DEBIAN/control").is_file()
	);
}

#[test]
fn build_writes_text_outputs_and_skips_unavailable_ones() {
	let dir = project();
	liminst(dir.path())
		.args([
			"build",
			"-t",
			"linux-x86_64",
			"--version",
			"1.0.0",
			"--output",
			"rpm",
			"--output",
			"arch",
			"--output",
			"floppy-1440",
		])
		.assert()
		.success()
		.stdout(predicate::str::contains(
			"floppy-1440: `floppy-1440` is not available yet",
		));
	let out = dir.path().join("build/linux-x86_64");
	assert!(out.join("demo.spec").is_file());
	assert!(out.join("PKGBUILD").is_file());
	assert!(out.join("demo-1.0.0.tar.gz").is_file());
}

#[test]
fn build_needs_a_version_when_it_comes_from_a_tag() {
	let dir = project();
	liminst(dir.path())
		.args(["build", "-t", "linux-x86_64"])
		.assert()
		.code(2)
		.stderr(predicate::str::contains("pass --version"));
}

#[test]
fn build_refuses_a_project_with_errors() {
	let dir = project();
	std::fs::remove_file(dir.path().join("installer/flows/install.flow.json")).unwrap();
	liminst(dir.path())
		.args(["build", "-t", "linux-x86_64", "--version", "1"])
		.assert()
		.code(1)
		.stderr(predicate::str::contains("Not building"));
}

#[test]
fn feed_add_records_and_replaces_an_artefact() {
	let dir = project();
	let file = dir.path().join("demo.run");
	std::fs::write(&file, b"payload").unwrap();
	let add = |dir: &Path| {
		liminst(dir)
			.args([
				"feed",
				"add",
				"-t",
				"linux-x86_64",
				"--file",
				"demo.run",
				"--url",
				"https://example.org/demo.run",
				"--version",
				"1.0.0",
			])
			.assert()
			.success()
			.stdout(predicate::str::contains("unsigned"));
	};
	add(dir.path());
	add(dir.path());
	let feed = json_of(&std::fs::read(dir.path().join("feed.json")).unwrap());
	assert_eq!(feed["publisher"], "Test Co");
	let product = &feed["products"][0];
	assert_eq!(product["id"], "hq.test.demo");
	assert_eq!(product["channels"]["stable"], "1.0.0");
	let artefacts = product["releases"][0]["artefacts"].as_array().unwrap();
	assert_eq!(artefacts.len(), 1);
	assert_eq!(artefacts[0]["target"], "linux-x86_64:tui");
	assert_eq!(artefacts[0]["size"], 7);
	assert_eq!(
		artefacts[0]["sha256"],
		"239f59ed55e737c77147cf55ad0c1b030b6d7ee748a7426952f9b852d5a935e5"
	);
}

#[test]
fn keygen_writes_a_pair_and_refuses_to_overwrite_it() {
	let dir = project();
	liminst(dir.path())
		.args(["keygen", "--out", "keys/feed"])
		.assert()
		.success()
		.stdout(predicate::str::contains("keys/feed.pub"));
	let secret = dir.path().join("keys/feed.key");
	let public = dir.path().join("keys/feed.pub");
	assert!(li_pack::read_signing_key(&secret).is_ok());
	assert_eq!(
		li_pack::read_signing_key(&secret).unwrap().verifying_key(),
		li_pack::read_public_key(&public).unwrap()
	);
	liminst(dir.path())
		.args(["keygen", "--out", "keys/feed"])
		.assert()
		.code(2)
		.stderr(predicate::str::contains("already exists"));
}

#[test]
fn feed_add_signs_and_a_tampered_feed_fails_verification() {
	let dir = project();
	liminst(dir.path())
		.args(["keygen", "--out", "feed"])
		.assert()
		.success();
	std::fs::write(dir.path().join("demo.run"), b"payload").unwrap();
	liminst(dir.path())
		.args([
			"feed",
			"add",
			"-t",
			"linux-x86_64",
			"--file",
			"demo.run",
			"--url",
			"demo.run",
			"--version",
			"1.0.0",
			"--sign-key",
			"feed.key",
		])
		.assert()
		.success()
		.stdout(predicate::str::contains("Signed:"));
	let json = std::fs::read(dir.path().join("feed.json")).unwrap();
	let sig = std::fs::read_to_string(dir.path().join("feed.json.sig")).unwrap();
	let key = li_pack::read_public_key(&dir.path().join("feed.pub")).unwrap();
	let feed = li_pack::verify_feed(&json, &sig, &key, None).unwrap();
	assert_eq!(feed.products[0].channels["stable"], "1.0.0");
	let mut bad = json.clone();
	bad[10] ^= 1;
	assert!(li_pack::verify_feed(&bad, &sig, &key, None).is_err());

	// An unsigned update drops the signature that no longer matches.
	liminst(dir.path())
		.args([
			"feed",
			"add",
			"-t",
			"linux-x86_64",
			"--file",
			"demo.run",
			"--url",
			"demo.run",
			"--version",
			"1.1.0",
		])
		.assert()
		.success()
		.stdout(predicate::str::contains("unsigned"))
		.stdout(predicate::str::contains("Removed the old signature"));
	assert!(!dir.path().join("feed.json.sig").exists());
}

#[test]
fn feed_add_refuses_an_unreadable_sign_key() {
	let dir = project();
	std::fs::write(dir.path().join("x"), b"x").unwrap();
	liminst(dir.path())
		.args([
			"feed",
			"add",
			"-t",
			"linux-x86_64",
			"--file",
			"x",
			"--url",
			"u",
			"--version",
			"1",
			"--sign-key",
			"nokey",
		])
		.assert()
		.code(2);
}

#[test]
fn theme_pack_is_reproducible() {
	let dir = project();
	let pack = |out: &str| {
		liminst(dir.path())
			.args(["theme", "pack", "--out", out])
			.assert()
			.success()
			.stdout(predicate::str::contains("Packed theme `neutral`: 5 files"));
		std::fs::read(dir.path().join(out)).unwrap()
	};
	assert_eq!(pack("a.litheme"), pack("b.litheme"));
	liminst(dir.path())
		.args(["theme", "pack", "missing"])
		.assert()
		.code(2)
		.stderr(predicate::str::contains("no theme named `missing`"));
}

#[test]
fn theme_pack_signs_the_archive() {
	let dir = project();
	liminst(dir.path())
		.args(["keygen", "--out", "t"])
		.assert()
		.success();
	liminst(dir.path())
		.args(["theme", "pack", "--out", "n.litheme", "--sign-key", "t.key"])
		.assert()
		.success()
		.stdout(predicate::str::contains("Signed:"));
	let bytes = std::fs::read(dir.path().join("n.litheme")).unwrap();
	let sig = std::fs::read_to_string(dir.path().join("n.litheme.sig")).unwrap();
	let key = li_pack::read_public_key(&dir.path().join("t.pub")).unwrap();
	li_pack::verify(&key, &bytes, &sig).unwrap();
	assert!(li_pack::verify(&key, b"other", &sig).is_err());
}

#[test]
fn check_reports_a_bad_condition_from_the_engine() {
	let dir = project();
	let path = dir.path().join("installer/flows/install.flow.json");
	let mut flow: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
	flow["steps"][1]["when"] = "len(components) > 1".into();
	std::fs::write(&path, serde_json::to_string(&flow).unwrap()).unwrap();
	let out = liminst(dir.path())
		.args(["check", "--json"])
		.assert()
		.code(1)
		.get_output()
		.stdout
		.clone();
	assert!(codes(&json_of(&out)).contains(&"bad-condition".to_owned()));
}

#[test]
fn studio_needs_mcp() {
	let dir = tempfile::tempdir().unwrap();
	liminst(dir.path())
		.args(["studio"])
		.assert()
		.code(2)
		.stderr(predicate::str::contains("pass --mcp"));
}

#[test]
fn studio_mcp_without_a_project_fails_plainly() {
	let dir = tempfile::tempdir().unwrap();
	liminst(dir.path())
		.args(["studio", "--mcp"])
		.assert()
		.code(2)
		.stderr(predicate::str::contains("cannot open the project"));
}

#[test]
fn studio_mcp_prints_the_address_and_token_then_serves() {
	use std::io::{BufRead, BufReader, Read, Write};
	use std::net::TcpStream;
	use std::process::{Command, Stdio};

	let dir = tempfile::tempdir().unwrap();
	liminst(dir.path()).arg("init").assert().success();
	let mut child = Command::new(assert_cmd::cargo::cargo_bin("liminst"))
		.args(["-C"])
		.arg(dir.path())
		.args(["studio", "--mcp"])
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()
		.unwrap();
	let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
	let url_line = lines.next().unwrap().unwrap();
	let token_line = lines.next().unwrap().unwrap();
	let url = url_line.strip_prefix("Studio MCP server: ").unwrap();
	let token = token_line.strip_prefix("Token: ").unwrap();
	assert_eq!(token.len(), 48);
	let addr = url
		.strip_prefix("http://")
		.unwrap()
		.strip_suffix("/mcp")
		.unwrap();
	let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
	let mut sock = TcpStream::connect(addr).unwrap();
	write!(
		sock,
		"POST /mcp HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
		body.len()
	)
	.unwrap();
	let mut reply = String::new();
	sock.read_to_string(&mut reply).unwrap();
	child.kill().unwrap();
	child.wait().unwrap();
	assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
	assert!(reply.contains("get_flow"), "{reply}");
}
