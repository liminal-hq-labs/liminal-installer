// Installs the example projects through the web runner's headless mode, into a scratch directory.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use li_engine::{Project, bundle::bundle_project, pack_payload};
use li_model::Receipt;
use li_pack::{SigningKey, generate_keypair};
use li_render_web::{PageKind, RunEvent};
use predicates::prelude::*;
use tempfile::TempDir;

fn example(name: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../../examples")
		.join(name)
		.join("installer")
}

/// A `.lip` of an example for a target (`os-arch:renderer`), as `liminst build` writes it.
fn lip(name: &str, target: &str, version: &str, key: &SigningKey) -> Vec<u8> {
	let project = Project::load_installer_dir(&example(name)).unwrap();
	let (id, renderer) = target.split_once(':').unwrap();
	let t = project
		.installer
		.targets
		.iter()
		.find(|t| t.id == id && t.renderer == renderer)
		.unwrap_or_else(|| panic!("{name} has no target {target}"));
	let plain = pack_payload(&project, &t.id).unwrap();
	let shared = project
		.installer
		.targets
		.iter()
		.filter(|x| x.id == t.id)
		.count()
		> 1;
	bundle_project(
		&project,
		&plain,
		&if shared { t.full_id() } else { t.id.clone() },
		version,
		Some(&key.verifying_key()),
	)
	.unwrap()
}

/// A scratch machine: the install directory, and `HOME` and the data directories under it, so
/// shortcuts and the registry stay out of the real user's files.
struct Scratch {
	dir: TempDir,
}

impl Scratch {
	fn new() -> Self {
		Scratch {
			dir: tempfile::tempdir().unwrap(),
		}
	}

	fn path(&self, rel: &str) -> PathBuf {
		self.dir.path().join(rel)
	}

	fn runner(&self) -> Command {
		let mut cmd = Command::cargo_bin("liminal-runner-headless").unwrap();
		cmd.env("HOME", self.path("home"))
			.env("XDG_DATA_HOME", self.path("home/data"))
			.env("TMPDIR", self.path("tmp"));
		std::fs::create_dir_all(self.path("home/data")).unwrap();
		std::fs::create_dir_all(self.path("tmp")).unwrap();
		cmd
	}

	fn receipt(&self) -> Receipt {
		let bytes = std::fs::read(self.path("app/.liminal/receipt.json")).unwrap();
		serde_json::from_slice(&bytes).unwrap()
	}
}

fn events(path: &Path) -> Vec<RunEvent> {
	std::fs::read_to_string(path)
		.unwrap()
		.lines()
		.map(|l| serde_json::from_str(l).unwrap())
		.collect()
}

fn pages(events: &[RunEvent]) -> Vec<String> {
	events
		.iter()
		.filter_map(|e| match e {
			RunEvent::Page { page } => Some(page.step.clone()),
			_ => None,
		})
		.collect()
}

/// What each example's web target should be drawn as, and the window it opens at.
struct Case {
	name: &'static str,
	target: &'static str,
	look: &'static str,
	window: (u32, u32),
	file: &'static str,
}

const CASES: &[Case] = &[
	Case {
		name: "lt-fizz",
		target: "windows-x86_64:web",
		look: "win11",
		window: (1100, 720),
		file: "FIZZ.EXE",
	},
	Case {
		name: "planboard",
		target: "macos-aarch64:web",
		look: "afterglow",
		window: (1024, 720),
		file: "planboard",
	},
];

#[test]
fn the_examples_install_headless_and_uninstall() {
	let (key, _) = generate_keypair().unwrap();
	for case in CASES {
		let s = Scratch::new();
		let payload = s.path("payload.lip");
		std::fs::write(&payload, lip(case.name, case.target, "1.0.0", &key)).unwrap();
		let log = s.path("install-events.jsonl");
		s.runner()
			.args(["--headless", "--install-dir"])
			.arg(s.path("app"))
			.args(["--answer", "licence.accepted=yes", "--events"])
			.arg(&log)
			.args(["install", "--lip"])
			.arg(&payload)
			.assert()
			.success()
			.stdout(predicate::str::contains("Installed"));

		assert!(s.path("app").join(case.file).is_file(), "{}", case.name);
		assert_eq!(
			s.receipt().target,
			case.target,
			"{}: the right target ran",
			case.name
		);

		let seen = events(&log);
		let RunEvent::Info { info } = &seen[0] else {
			panic!("{}: the first event is not Info", case.name);
		};
		assert_eq!(info.look, case.look, "{}", case.name);
		assert_eq!((info.window.width, info.window.height), case.window);
		assert_eq!(info.target, case.target);
		let steps = pages(&seen);
		assert_eq!(steps.first().map(String::as_str), Some("welcome"));
		assert_eq!(steps.last().map(String::as_str), Some("done"), "{steps:?}");
		assert!(steps.contains(&"copying".to_owned()), "{steps:?}");
		let commit = seen.iter().find_map(|e| match e {
			RunEvent::Page { page } if page.kind == PageKind::Commit => Some(page),
			_ => None,
		});
		assert!(
			commit.is_some_and(|p| p.layout.is_some()),
			"{}: a commit page",
			case.name
		);
		let last = seen.iter().rev().find_map(|e| match e {
			RunEvent::Progress { progress } => Some(progress),
			_ => None,
		});
		assert!(
			last.is_some_and(|p| p.total >= 0.999),
			"{}: progress reaches 1",
			case.name
		);
		assert!(matches!(seen.last(), Some(RunEvent::Done)));

		s.runner()
			.args(["--headless", "--install-dir"])
			.arg(s.path("app"))
			.arg("uninstall")
			.assert()
			.success();
		assert!(
			!s.path("app").exists(),
			"{}: uninstall leaves nothing",
			case.name
		);
	}
}

#[test]
fn a_page_with_next_switched_off_stops_the_headless_run() {
	let (key, _) = generate_keypair().unwrap();
	let s = Scratch::new();
	let payload = s.path("payload.lip");
	std::fs::write(
		&payload,
		lip("planboard", "macos-aarch64:web", "1.0.0", &key),
	)
	.unwrap();
	s.runner()
		.args(["--headless", "--install-dir"])
		.arg(s.path("app"))
		.args(["install", "--lip"])
		.arg(&payload)
		.assert()
		.code(1)
		.stderr(predicate::str::contains("licence").and(predicate::str::contains("Next")));
	assert!(!s.path("app").exists(), "nothing was installed");
}

#[test]
fn repair_modify_and_update_run_headless() {
	let (key, _) = generate_keypair().unwrap();
	let s = Scratch::new();
	let payload = s.path("payload.lip");
	std::fs::write(
		&payload,
		lip("lt-fizz", "windows-x86_64:web", "1.0.0", &key),
	)
	.unwrap();
	let at = |cmd: &mut Command| {
		cmd.args(["--headless", "--install-dir"]).arg(s.path("app"));
	};
	let mut install = s.runner();
	at(&mut install);
	install
		.args([
			"--answer",
			"licence.accepted=yes",
			"--answer",
			"components=game",
			"install",
			"--lip",
		])
		.arg(&payload)
		.assert()
		.success();
	assert!(!s.path("app/LEVELS").exists(), "only the game was chosen");

	// Repair puts back a file that went missing.
	std::fs::remove_file(s.path("app/FIZZ.EXE")).unwrap();
	let log = s.path("repair-events.jsonl");
	let mut repair = s.runner();
	at(&mut repair);
	repair
		.arg("--events")
		.arg(&log)
		.arg("repair")
		.assert()
		.success();
	assert!(s.path("app/FIZZ.EXE").is_file(), "repair restored the file");
	let seen = events(&log);
	assert!(matches!(&seen[0], RunEvent::Info { info } if info.look == "win11"));
	assert!(seen.iter().any(|e| matches!(e, RunEvent::Progress { .. })));

	// Modify adds a component.
	let mut modify = s.runner();
	at(&mut modify);
	modify
		.args(["modify", "--component", "game", "--component", "levels"])
		.assert()
		.success();
	assert!(s.path("app/LEVELS/EP1.LVL").is_file());

	// Update to a signed release.
	let feed = s.path("feed");
	std::fs::create_dir_all(&feed).unwrap();
	let v2 = lip("lt-fizz", "windows-x86_64:web", "1.1.0", &key);
	std::fs::write(feed.join("fizz-1.1.0.lip"), &v2).unwrap();
	let project = Project::load_installer_dir(&example("lt-fizz")).unwrap();
	let p = &project.installer.product;
	let release = li_model::Release {
		version: "1.1.0".into(),
		artefacts: vec![li_model::Artefact {
			target: "windows-x86_64:web".into(),
			kind: "payload".into(),
			url: "fizz-1.1.0.lip".into(),
			size: v2.len() as u64,
			sha256: li_pack::sha256_hex(&v2),
		}],
	};
	let product = li_model::FeedProduct {
		id: p.id.clone(),
		name: p.name.clone(),
		channels: [("stable".to_owned(), "1.1.0".to_owned())].into(),
		releases: vec![release],
		themes: vec![],
	};
	let feed_file = li_model::Feed {
		version: 1,
		publisher: p.publisher.clone(),
		timestamp: Some("2026-11-23T12:00:00Z".into()),
		products: vec![product],
	};
	li_pack::write_feed(&feed, &feed_file, &key).unwrap();
	let mut update = s.runner();
	at(&mut update);
	update
		.arg("update")
		.arg("--feed")
		.arg(&feed)
		.assert()
		.success();
	assert_eq!(s.receipt().installed, "1.1.0");
}
