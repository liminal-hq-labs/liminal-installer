// End to end on Linux: build a themed `.run`, install it headless, update from a signed feed,
// repair, modify, launch and uninstall.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

/// The runner, built beside `liminst` when the test run did not already build it.
fn engine() -> PathBuf {
	let liminst = PathBuf::from(env!("CARGO_BIN_EXE_liminst"));
	let engine = liminst.with_file_name("liminal-engine");
	if !engine.is_file() {
		let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
		let status = Command::new(cargo)
			.args(["build", "-p", "li-runner"])
			.status()
			.expect("cargo runs");
		assert!(status.success(), "cargo build -p li-runner failed");
	}
	assert!(engine.is_file(), "{} is missing", engine.display());
	engine
}

struct Sandbox {
	root: tempfile::TempDir,
}

impl Sandbox {
	fn new() -> Self {
		let root = tempfile::tempdir().unwrap();
		for d in ["home", "xdg", "tmp", "project", "feed"] {
			std::fs::create_dir_all(root.path().join(d)).unwrap();
		}
		Sandbox { root }
	}

	fn path(&self, rel: &str) -> PathBuf {
		self.root.path().join(rel)
	}

	fn env<'a>(&self, c: &'a mut Command) -> &'a mut Command {
		c.env("HOME", self.path("home"))
			.env("XDG_DATA_HOME", self.path("xdg"))
			.env("TMPDIR", self.path("tmp"))
			.env("MARK", self.path("marker"))
			.stdin(std::process::Stdio::null())
	}

	fn liminst(&self, args: &[&str]) -> Output {
		let mut c = Command::new(env!("CARGO_BIN_EXE_liminst"));
		self.env(&mut c)
			.current_dir(self.path("project"))
			.args(args);
		c.output().unwrap()
	}

	fn run(&self, program: &Path, args: &[&str]) -> Output {
		let mut c = Command::new(program);
		self.env(&mut c).args(args);
		c.output().unwrap()
	}
}

fn ok(o: &Output) -> String {
	assert!(
		o.status.success(),
		"exit {:?}\nstdout: {}\nstderr: {}",
		o.status.code(),
		String::from_utf8_lossy(&o.stdout),
		String::from_utf8_lossy(&o.stderr)
	);
	String::from_utf8_lossy(&o.stdout).into_owned()
}

fn write(path: &Path, text: &str) {
	std::fs::create_dir_all(path.parent().unwrap()).unwrap();
	std::fs::write(path, text).unwrap();
}

/// Every file and directory under `root`, relative, with file contents.
fn tree(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
	fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Option<Vec<u8>>>) {
		let Ok(rd) = std::fs::read_dir(dir) else {
			return;
		};
		for e in rd {
			let p = e.unwrap().path();
			let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
			if p.is_dir() {
				out.insert(rel, None);
				walk(root, &p, out);
			} else {
				out.insert(rel, Some(std::fs::read(&p).unwrap()));
			}
		}
	}
	let mut out = BTreeMap::new();
	walk(root, root, &mut out);
	out
}

const DEMO_V1: &str = "#!/bin/sh\ntouch \"$MARK\"\necho v1\n";
const DEMO_V2: &str = "#!/bin/sh\ntouch \"$MARK\"\necho v2\n";

fn write_project(sb: &Sandbox, demo: &str) {
	let p = sb.path("project");
	write(&p.join("installer/payload/linux-x86_64/demo"), demo);
	std::fs::set_permissions(
		p.join("installer/payload/linux-x86_64/demo"),
		std::os::unix::fs::PermissionsExt::from_mode(0o755),
	)
	.unwrap();
	write(&p.join("installer/payload/linux-x86_64/data.txt"), "data\n");
	write(&p.join("installer/docs/README.txt"), "read me\n");
}

fn build(sb: &Sandbox, engine: &Path, version: &str) -> (PathBuf, PathBuf) {
	let out = sb.liminst(&[
		"build",
		"-t",
		"linux-x86_64",
		"--version",
		version,
		"--output",
		"run",
		"--engine",
		engine.to_str().unwrap(),
	]);
	ok(&out);
	let dir = sb.path("project/build/linux-x86_64");
	(
		dir.join(format!("demo-{version}-linux-x86_64.run")),
		dir.join(format!("demo-{version}-linux-x86_64.lip")),
	)
}

#[test]
fn install_update_repair_modify_launch_and_uninstall() {
	let sb = Sandbox::new();
	let engine = engine();
	let project = sb.path("project");
	ok(&sb.liminst(&[
		"init",
		"--id",
		"hq.test.demo",
		"--name",
		"Demo App",
		"--publisher",
		"Test Co",
	]));
	ok(&sb.liminst(&["keygen", "--out", "keys/feed"]));
	let manifest = project.join("installer/installer.toml");
	let mut text = std::fs::read_to_string(&manifest).unwrap();
	text.push_str("\n[signing]\nfeed_key = \"keys/feed.pub\"\n");
	std::fs::write(&manifest, text).unwrap();
	write(
		&project.join("installer/components.toml"),
		r#"[[component]]
id       = "app"
name     = "Demo App"
required = true
files    = [{ from = "payload/{target}/*", to = "{install}/bin/" }]
shortcut = [{ name = "Demo App", target = "{install}/bin/demo", in = "start_menu" }]

[[component]]
id    = "docs"
name  = "Documentation"
files = [{ from = "docs/README.txt", to = "{install}/share/" }]
"#,
	);
	write_project(&sb, DEMO_V1);
	ok(&sb.liminst(&["check"]));

	// Build v1 and install it headless, with only the required component.
	let (run1, _) = build(&sb, &engine, "1.0.0");
	let before = tree(sb.root.path());
	let prefix = sb.path("prefix");
	let out = sb.run(
		Path::new("sh"),
		&[
			run1.to_str().unwrap(),
			"--prefix",
			prefix.to_str().unwrap(),
			"--",
			"--headless",
			"--answer",
			"components=app",
		],
	);
	ok(&out);
	assert_eq!(
		std::fs::read_to_string(prefix.join("bin/demo")).unwrap(),
		DEMO_V1
	);
	assert!(prefix.join("bin/data.txt").is_file());
	assert!(!prefix.join("share/README.txt").exists());
	let receipt: Value =
		serde_json::from_slice(&std::fs::read(prefix.join(".liminal/receipt.json")).unwrap())
			.unwrap();
	assert_eq!(receipt["installed"], "1.0.0");
	assert_eq!(receipt["product"], "hq.test.demo");
	assert_eq!(receipt["components"], serde_json::json!(["app"]));
	assert_eq!(receipt["target"], "linux-x86_64:tui");
	// The stub's own files are gone; the runner kept copies for later commands.
	assert!(!prefix.join("engine").exists());
	assert!(!prefix.join("payload.lip").exists());
	let runner = prefix.join(".liminal/runner/liminal-engine");
	assert!(runner.is_file());
	assert!(prefix.join(".liminal/runner/payload.lip").is_file());
	assert!(prefix.join(".liminal/runner/feed.pub").is_file());
	assert!(
		tree(&sb.path("xdg"))
			.keys()
			.any(|k| k.ends_with(".desktop")),
		"the shortcut was written"
	);

	// Modify adds the optional component.
	ok(&sb.run(
		&runner,
		&[
			"--headless",
			"modify",
			"--component",
			"app",
			"--component",
			"docs",
		],
	));
	assert_eq!(
		std::fs::read_to_string(prefix.join("share/README.txt")).unwrap(),
		"read me\n"
	);

	// Publish v2 to a signed feed beside the project.
	write_project(&sb, DEMO_V2);
	let (_, lip2) = build(&sb, &engine, "2.0.0");
	std::fs::copy(&lip2, sb.path("feed/demo-2.0.0.lip")).unwrap();
	ok(&sb.liminst(&[
		"feed",
		"add",
		"--feed",
		"../feed/feed.json",
		"-t",
		"linux-x86_64",
		"--file",
		lip2.to_str().unwrap(),
		"--url",
		"demo-2.0.0.lip",
		"--version",
		"2.0.0",
		"--sign-key",
		"keys/feed.key",
	]));
	assert!(sb.path("feed/feed.json.sig").is_file());

	// A feed whose bytes changed after signing is refused, and nothing changes.
	let tampered = sb.path("tampered");
	std::fs::create_dir_all(&tampered).unwrap();
	std::fs::copy(
		sb.path("feed/demo-2.0.0.lip"),
		tampered.join("demo-2.0.0.lip"),
	)
	.unwrap();
	std::fs::copy(
		sb.path("feed/feed.json.sig"),
		tampered.join("feed.json.sig"),
	)
	.unwrap();
	let json = std::fs::read_to_string(sb.path("feed/feed.json")).unwrap();
	std::fs::write(
		tampered.join("feed.json"),
		json.replace("Test Co", "Evil Co"),
	)
	.unwrap();
	let o = sb.run(
		&runner,
		&["--headless", "update", "--feed", tampered.to_str().unwrap()],
	);
	assert_eq!(o.status.code(), Some(1));
	assert!(String::from_utf8_lossy(&o.stderr).contains("refused"));
	assert_eq!(
		std::fs::read_to_string(prefix.join("bin/demo")).unwrap(),
		DEMO_V1
	);

	// A feed signed with another key is refused too.
	ok(&sb.liminst(&["keygen", "--out", "keys/other"]));
	ok(&sb.liminst(&[
		"feed",
		"add",
		"--feed",
		"../tampered/other.json",
		"-t",
		"linux-x86_64",
		"--file",
		lip2.to_str().unwrap(),
		"--url",
		"demo-2.0.0.lip",
		"--version",
		"2.0.0",
		"--sign-key",
		"keys/other.key",
	]));
	std::fs::rename(tampered.join("other.json"), tampered.join("feed.json")).unwrap();
	std::fs::rename(
		tampered.join("other.json.sig"),
		tampered.join("feed.json.sig"),
	)
	.unwrap();
	let o = sb.run(
		&runner,
		&["--headless", "update", "--feed", tampered.to_str().unwrap()],
	);
	assert_eq!(o.status.code(), Some(1));
	assert_eq!(
		std::fs::read_to_string(prefix.join("bin/demo")).unwrap(),
		DEMO_V1
	);

	// The real update, from the feed folder.
	let said = ok(&sb.run(
		&runner,
		&[
			"--headless",
			"update",
			"--feed",
			sb.path("feed").to_str().unwrap(),
		],
	));
	assert!(
		said.contains("Updated hq.test.demo from 1.0.0 to 2.0.0"),
		"{said}"
	);
	assert_eq!(
		std::fs::read_to_string(prefix.join("bin/demo")).unwrap(),
		DEMO_V2
	);
	let receipt: Value =
		serde_json::from_slice(&std::fs::read(prefix.join(".liminal/receipt.json")).unwrap())
			.unwrap();
	assert_eq!(receipt["installed"], "2.0.0");
	assert_eq!(receipt["components"], serde_json::json!(["app", "docs"]));
	let said = ok(&sb.run(
		&runner,
		&[
			"--headless",
			"update",
			"--feed",
			sb.path("feed").to_str().unwrap(),
		],
	));
	assert!(said.contains("up to date"), "{said}");

	// Repair restores a deleted file and a damaged one.
	std::fs::remove_file(prefix.join("bin/data.txt")).unwrap();
	std::fs::write(prefix.join("share/README.txt"), "damaged").unwrap();
	let said = ok(&sb.run(&runner, &["--headless", "repair"]));
	assert!(said.contains("restored 2"), "{said}");
	assert_eq!(
		std::fs::read_to_string(prefix.join("bin/data.txt")).unwrap(),
		"data\n"
	);
	assert_eq!(
		std::fs::read_to_string(prefix.join("share/README.txt")).unwrap(),
		"read me\n"
	);

	// Launch starts the product, which marks that it ran.
	ok(&sb.run(&runner, &["--headless", "launch"]));
	for _ in 0..50 {
		if sb.path("marker").exists() {
			break;
		}
		std::thread::sleep(std::time::Duration::from_millis(100));
	}
	assert!(sb.path("marker").exists(), "the product did not start");

	// Uninstall leaves the machine as it was.
	ok(&sb.run(&runner, &["--headless", "uninstall"]));
	assert!(!prefix.exists(), "{:?}", tree(&prefix));
	let after = tree(sb.root.path());
	let leftovers: Vec<_> = after
		.keys()
		.filter(|k| {
			!before.contains_key(*k)
				&& !k.starts_with("project/")
				&& !k.starts_with("feed")
				&& !k.starts_with("tampered")
				&& *k != "marker"
		})
		.collect();
	assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
}

#[test]
fn the_runner_refuses_a_payload_that_was_changed() {
	let sb = Sandbox::new();
	let engine = engine();
	ok(&sb.liminst(&["init", "--id", "hq.test.demo", "--name", "Demo"]));
	write_project(&sb, DEMO_V1);
	write(
		&sb.path("project/installer/components.toml"),
		r#"[[component]]
id       = "app"
name     = "Demo"
required = true
files    = [{ from = "payload/{target}/demo", to = "{install}/bin/" }]
"#,
	);
	let (run, _) = build(&sb, &engine, "1.0.0");
	let mut bytes = std::fs::read(&run).unwrap();
	let n = bytes.len();
	bytes[n - 20] ^= 0xff;
	std::fs::write(&run, bytes).unwrap();
	let prefix = sb.path("prefix");
	let o = sb.run(
		Path::new("sh"),
		&[
			run.to_str().unwrap(),
			"--prefix",
			prefix.to_str().unwrap(),
			"--",
			"--headless",
		],
	);
	assert!(!o.status.success());
	assert!(String::from_utf8_lossy(&o.stderr).contains("damaged"));
}
