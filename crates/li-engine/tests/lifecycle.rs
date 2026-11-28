// Lifecycle tests: install, update, modify, repair and uninstall against real and simulated machines.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::*;
use li_engine::{
	EngineError, FaultMachine, LaunchOutcome, LipPayload, Machine, Phase, Progress, RunOptions,
	SimMachine, find_update, install, launch, modify, pack_payload, preview, read_receipt, repair,
	uninstall, update,
};
use li_model::{Artefact, Feed, FeedProduct, Operation, Release};
use li_pack::{LipBuilder, generate_keypair, sha256_hex, sign_feed, verify_feed};

#[test]
fn install_modify_repair_uninstall_round_trip() {
	let project = project();
	let payload = payload(&project);
	let root = tempfile::tempdir().unwrap();
	let mut m = machine(root.path());
	let install_dir = root.path().join("app");
	let before = tree(root.path());

	let receipt = install(
		&mut m,
		&project,
		&payload,
		&request(&install_dir),
		&mut RunOptions::default(),
	)
	.unwrap();
	assert_eq!(receipt.installed, "1.0.0");
	assert_eq!(receipt.target, "linux-x86_64:canvas");
	assert_eq!(receipt.components, vec!["game", "sound"]);
	assert!(install_dir.join("APP").is_file());
	assert!(install_dir.join("data/a.dat").is_file());
	assert!(install_dir.join("sound/sub/y.drv").is_file());
	assert_eq!(
		std::fs::read_to_string(install_dir.join("INSTALLED.txt")).unwrap(),
		"installed by script"
	);
	let desktop = root.path().join("data/applications/fixture-app.desktop");
	assert!(std::fs::read_to_string(&desktop).unwrap().contains("APP"));
	assert!(
		root.path()
			.join("data/liminal-installer/registry.json")
			.is_file()
	);
	assert_eq!(read_receipt(&m, &install_dir).unwrap(), receipt);
	// No journal is left behind in the temp directory.
	assert!(
		std::fs::read_dir(root.path().join("tmp"))
			.unwrap()
			.next()
			.is_none()
	);
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		let mode = std::fs::metadata(install_dir.join("APP"))
			.unwrap()
			.permissions()
			.mode();
		assert_ne!(mode & 0o111, 0, "APP stays executable");
	}

	// Modify: drop the sound component, then add it back.
	let r = modify(
		&mut m,
		&project,
		&payload,
		&install_dir,
		&["game".to_owned()],
		&mut RunOptions::default(),
	)
	.unwrap();
	assert_eq!(r.components, vec!["game"]);
	assert!(!install_dir.join("sound").exists());
	assert!(install_dir.join("APP").is_file());
	let r = modify(
		&mut m,
		&project,
		&payload,
		&install_dir,
		&["game".to_owned(), "sound".to_owned()],
		&mut RunOptions::default(),
	)
	.unwrap();
	assert_eq!(r.components, vec!["game", "sound"]);
	assert!(install_dir.join("sound/x.drv").is_file());

	// Repair: delete one file, change another.
	std::fs::remove_file(install_dir.join("APP")).unwrap();
	std::fs::write(install_dir.join("data/a.dat"), b"corrupt").unwrap();
	let report = repair(&mut m, &payload, &install_dir, &mut RunOptions::default()).unwrap();
	assert_eq!(report.restored.len(), 2, "{report:?}");
	assert_eq!(
		std::fs::read(install_dir.join("data/a.dat")).unwrap(),
		b"alpha data\n"
	);
	assert!(install_dir.join("APP").is_file());
	let clean = repair(&mut m, &payload, &install_dir, &mut RunOptions::default()).unwrap();
	assert!(clean.restored.is_empty());

	uninstall(
		&mut m,
		Some(&project),
		&install_dir,
		&mut RunOptions::default(),
	)
	.unwrap();
	assert_eq!(
		tree(root.path()),
		before,
		"uninstall leaves no files behind"
	);
}

#[test]
fn uninstall_keeps_files_the_user_added() {
	let project = project();
	let payload = payload(&project);
	let root = tempfile::tempdir().unwrap();
	let mut m = machine(root.path());
	let install_dir = root.path().join("app");
	install(
		&mut m,
		&project,
		&payload,
		&request(&install_dir),
		&mut RunOptions::default(),
	)
	.unwrap();
	std::fs::write(install_dir.join("saves.dat"), b"mine").unwrap();
	uninstall(&mut m, None, &install_dir, &mut RunOptions::default()).unwrap();
	assert_eq!(
		std::fs::read(install_dir.join("saves.dat")).unwrap(),
		b"mine"
	);
	assert!(!install_dir.join("APP").exists());
}

fn feed_for(version: &str, lip: &[u8], target: &str) -> Feed {
	Feed {
		version: 2,
		publisher: "Liminal HQ".into(),
		timestamp: Some("2026-11-21T00:00:00Z".into()),
		products: vec![FeedProduct {
			id: "hq.liminal.fixture".into(),
			name: "Fixture".into(),
			channels: [("stable".to_owned(), version.to_owned())].into(),
			releases: vec![Release {
				version: version.into(),
				artefacts: vec![Artefact {
					target: target.into(),
					kind: "payload".into(),
					url: "https://example.org/fixture.lip".into(),
					size: lip.len() as u64,
					sha256: sha256_hex(lip),
				}],
			}],
			themes: vec![],
		}],
	}
}

/// The 1.0.0 payload with `APP` changed, `b.dat` dropped and `c.dat` added.
fn newer_lip(project: &li_engine::Project) -> Vec<u8> {
	let old = LipPayload::from_bytes(&pack_payload(project, "linux-x86_64").unwrap()).unwrap();
	let mut b = LipBuilder::new();
	for e in old.0.entries() {
		match e.path.as_str() {
			"dist/shared/b.dat" => continue,
			"dist/linux-x86_64/APP" => b
				.add(&e.path, b"#!/bin/sh\necho version two\n".to_vec(), true)
				.unwrap(),
			_ => b
				.add(&e.path, old.0.read(&e.path).unwrap().to_vec(), e.executable)
				.unwrap(),
		}
	}
	b.add("dist/shared/c.dat", b"charlie data\n".to_vec(), false)
		.unwrap();
	b.build().unwrap()
}

#[test]
fn update_from_a_verified_feed_keeps_choices() {
	let project = project();
	let root = tempfile::tempdir().unwrap();
	let mut m = machine(root.path());
	let install_dir = root.path().join("app");
	let mut req = request(&install_dir);
	req.components = Some(vec!["game".into()]);
	install(
		&mut m,
		&project,
		&payload(&project),
		&req,
		&mut RunOptions::default(),
	)
	.unwrap();
	let before = tree(root.path());

	let lip = newer_lip(&project);
	let feed = feed_for("1.1.0", &lip, "linux-x86_64:canvas");
	let (sk, vk) = generate_keypair().unwrap();
	let (json, sig) = sign_feed(&feed, &sk).unwrap();
	let feed = verify_feed(&json, &sig, &vk, None).unwrap();

	let receipt = read_receipt(&m, &install_dir).unwrap();
	assert_eq!(find_update(&receipt, &feed).unwrap().version, "1.1.0");

	// A payload that does not match the feed's hash is refused, and nothing changes.
	let mut bad = lip.clone();
	*bad.last_mut().unwrap() ^= 1;
	let err = update(
		&mut m,
		&project,
		&install_dir,
		&feed,
		&bad,
		&mut RunOptions::default(),
	)
	.unwrap_err();
	assert!(matches!(err, EngineError::Pack(_)), "{err}");
	assert_eq!(tree(root.path()), before);

	let r = update(
		&mut m,
		&project,
		&install_dir,
		&feed,
		&lip,
		&mut RunOptions::default(),
	)
	.unwrap();
	assert_eq!(r.installed, "1.1.0");
	assert_eq!(r.components, vec!["game"], "choices are kept");
	assert_eq!(r.channel.as_deref(), Some("stable"));
	assert_eq!(
		std::fs::read(install_dir.join("APP")).unwrap(),
		b"#!/bin/sh\necho version two\n"
	);
	assert!(!install_dir.join("data/b.dat").exists());
	assert!(install_dir.join("data/c.dat").is_file());
	assert!(install_dir.join("data/a.dat").is_file());
	assert!(!install_dir.join("sound").exists());

	// Already current.
	assert!(matches!(
		update(
			&mut m,
			&project,
			&install_dir,
			&feed,
			&lip,
			&mut RunOptions::default()
		),
		Err(EngineError::UpToDate(_))
	));

	uninstall(
		&mut m,
		Some(&project),
		&install_dir,
		&mut RunOptions::default(),
	)
	.unwrap();
	let empty_root = tempfile::tempdir().unwrap();
	let _ = machine(empty_root.path());
	assert!(!install_dir.exists());
	assert!(
		std::fs::read_dir(root.path().join("data"))
			.unwrap()
			.next()
			.is_none()
	);
}

#[test]
fn feed_rollback_and_bad_signatures_are_refused() {
	let (sk, vk) = generate_keypair().unwrap();
	let (_, other) = generate_keypair().unwrap();
	let newer = feed_for("2.0.0", b"x", "t");
	let mut older = feed_for("1.0.0", b"x", "t");
	older.timestamp = Some("2026-01-01T00:00:00Z".into());
	let (json, sig) = sign_feed(&older, &sk).unwrap();
	assert!(verify_feed(&json, &sig, &vk, Some(&newer)).is_err());
	assert!(verify_feed(&json, &sig, &other, None).is_err());
	assert!(verify_feed(&json, &sig, &vk, None).is_ok());
}

#[test]
fn sha256_mismatch_is_refused_at_stage_and_at_commit() {
	let project = project();
	let real = payload(&project);
	// Tampered between Plan and Stage: nothing is touched.
	let root = tempfile::tempdir().unwrap();
	let mut m = machine(root.path());
	let snapshot = tree(root.path());
	let flaky = FlakyPayload::new(&real, "dist/shared/a.dat", 1);
	let err = install(
		&mut m,
		&project,
		&flaky,
		&request(&root.path().join("app")),
		&mut RunOptions::default(),
	)
	.unwrap_err();
	assert!(matches!(err, EngineError::HashMismatch { .. }), "{err}");
	assert_eq!(tree(root.path()), snapshot);

	// Tampered between Stage and Commit: the commit rolls back.
	let root = tempfile::tempdir().unwrap();
	let mut m = machine(root.path());
	let snapshot = tree(root.path());
	let flaky = FlakyPayload::new(&real, "dist/shared/a.dat", 2);
	let err = install(
		&mut m,
		&project,
		&flaky,
		&request(&root.path().join("app")),
		&mut RunOptions::default(),
	)
	.unwrap_err();
	match err {
		EngineError::RolledBack(inner) => {
			assert!(matches!(*inner, EngineError::HashMismatch { .. }))
		}
		other => panic!("expected a rollback, got {other}"),
	}
	assert_eq!(tree(root.path()), snapshot);
}

#[test]
fn a_failure_at_any_point_in_commit_rolls_back_fully() {
	let project = project();
	let payload = payload(&project);

	// Learn how many mutating calls a full install makes.
	let root = tempfile::tempdir().unwrap();
	let mut counting = FaultMachine::counting(machine(root.path()));
	install(
		&mut counting,
		&project,
		&payload,
		&request(&root.path().join("app")),
		&mut RunOptions::default(),
	)
	.unwrap();
	let total = counting.calls();
	assert!(total > 10, "{total}");

	for n in 0..total {
		let root = tempfile::tempdir().unwrap();
		let mut m = FaultMachine::failing_at(machine(root.path()), n);
		let before = tree(root.path());
		let err = install(
			&mut m,
			&project,
			&payload,
			&request(&root.path().join("app")),
			&mut RunOptions::default(),
		)
		.expect_err(&format!("call {n} should fail"));
		assert!(matches!(err, EngineError::RolledBack(_)), "call {n}: {err}");
		assert_eq!(tree(root.path()), before, "call {n} left changes behind");
		assert!(m.inner.registry_get("file_type:.fix").is_none(), "call {n}");
	}
}

#[test]
fn a_failed_update_restores_the_old_files() {
	let project = project();
	let payload = payload(&project);
	let root = tempfile::tempdir().unwrap();
	let install_dir = root.path().join("app");
	let mut m = machine(root.path());
	install(
		&mut m,
		&project,
		&payload,
		&request(&install_dir),
		&mut RunOptions::default(),
	)
	.unwrap();
	let before = tree(root.path());

	let lip = newer_lip(&project);
	let feed = feed_for("1.1.0", &lip, "linux-x86_64:canvas");
	// Count the update's calls on a scratch copy of the same state.
	let scratch = tempfile::tempdir().unwrap();
	let mut sm = FaultMachine::counting(machine(scratch.path()));
	install(
		&mut sm,
		&project,
		&payload,
		&request(&scratch.path().join("app")),
		&mut RunOptions::default(),
	)
	.unwrap();
	let base = sm.calls();
	update(
		&mut sm,
		&project,
		&scratch.path().join("app"),
		&feed,
		&lip,
		&mut RunOptions::default(),
	)
	.unwrap();
	let total = sm.calls() - base;

	for n in 0..total {
		let mut fm = FaultMachine::failing_at(machine(root.path()), n);
		let err = update(
			&mut fm,
			&project,
			&install_dir,
			&feed,
			&lip,
			&mut RunOptions::default(),
		)
		.unwrap_err();
		assert!(matches!(err, EngineError::RolledBack(_)), "call {n}: {err}");
		assert_eq!(tree(root.path()), before, "call {n}");
	}
}

#[test]
fn cancel_rolls_back() {
	let project = project();
	let payload = payload(&project);
	let root = tempfile::tempdir().unwrap();
	let mut m = machine(root.path());
	let before = tree(root.path());

	let mut seen = 0;
	let mut cb = |p: &Progress| {
		if p.files_done >= 2 {
			seen += 1;
			return false;
		}
		true
	};
	let mut opts = RunOptions {
		on_progress: Some(&mut cb),
		..RunOptions::default()
	};
	let err = install(
		&mut m,
		&project,
		&payload,
		&request(&root.path().join("app")),
		&mut opts,
	)
	.unwrap_err();
	assert!(matches!(err, EngineError::Cancelled), "{err}");
	assert_eq!(seen, 1);
	assert_eq!(tree(root.path()), before);

	let flag = Arc::new(AtomicBool::new(true));
	let mut opts = RunOptions {
		cancel: Some(flag),
		..RunOptions::default()
	};
	let err = install(
		&mut m,
		&project,
		&payload,
		&request(&root.path().join("app")),
		&mut opts,
	)
	.unwrap_err();
	assert!(matches!(err, EngineError::Cancelled));
	assert_eq!(tree(root.path()), before);
}

#[test]
fn progress_feeds_the_three_meters() {
	let project = project();
	let payload = payload(&project);
	let root = tempfile::tempdir().unwrap();
	let mut m = machine(root.path());
	let mut events: Vec<Progress> = Vec::new();
	let mut cb = |p: &Progress| {
		events.push(p.clone());
		true
	};
	let mut opts = RunOptions {
		on_progress: Some(&mut cb),
		..RunOptions::default()
	};
	install(
		&mut m,
		&project,
		&payload,
		&request(&root.path().join("app")),
		&mut opts,
	)
	.unwrap();
	let files: Vec<&str> = events
		.iter()
		.filter_map(|e| e.current_file.as_deref())
		.collect();
	assert!(files.contains(&"{install}/APP"));
	assert!(files.contains(&"{install}/sound/sub/y.drv"));
	let last_commit = events
		.iter()
		.rev()
		.find(|e| e.phase == Phase::Commit)
		.unwrap();
	assert_eq!(last_commit.files_done, last_commit.files_total);
	assert!((last_commit.fraction() - 1.0).abs() < 1e-9);
	assert!(events.iter().all(|e| e.free_space > 0));
	assert_eq!(events.last().unwrap().phase, Phase::Record);
	let fractions: Vec<f64> = events.iter().map(Progress::fraction).collect();
	assert!(fractions.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn not_enough_space_is_refused_before_any_change() {
	let project = project();
	let payload = payload(&project);
	let mut m = SimMachine::new().with_capacity(10);
	m.mkdir_all(Path::new("/sim/work"));
	let before = m.snapshot();
	let err = install(
		&mut m,
		&project,
		&payload,
		&request(Path::new("/sim/work/app")),
		&mut RunOptions::default(),
	)
	.unwrap_err();
	assert!(
		matches!(err, EngineError::InsufficientSpace { .. }),
		"{err}"
	);
	assert_eq!(m.snapshot(), before);
}

#[test]
fn a_dry_run_on_the_sim_machine_matches_a_real_run() {
	let project = project();
	let payload = payload(&project);

	let sim = SimMachine::new();
	sim.mkdir_all(Path::new("/sim/work"));
	let mut sim = sim;
	let sim_req = request(Path::new("/sim/work/app"));

	let root = tempfile::tempdir().unwrap();
	let mut real = machine(root.path());
	let real_req = request(&root.path().join("app"));

	let sim_plan = preview(&project, &payload, &sim_req, &sim).unwrap();
	let real_plan = preview(&project, &payload, &real_req, &real).unwrap();
	assert_eq!(sim_plan, real_plan);
	assert!(matches!(
		sim_plan.ops.last(),
		Some(Operation::ReceiptWrite { .. })
	));
	assert!(sim_plan.ops.iter().any(
		|o| matches!(o, Operation::WriteFile { path, .. } if path == "{install}/INSTALLED.txt")
	));

	let sim_receipt = install(
		&mut sim,
		&project,
		&payload,
		&sim_req,
		&mut RunOptions::default(),
	)
	.unwrap();
	let real_receipt = install(
		&mut real,
		&project,
		&payload,
		&real_req,
		&mut RunOptions::default(),
	)
	.unwrap();
	assert_eq!(sim_receipt.operations, real_receipt.operations);
	assert_eq!(sim_receipt.components, real_receipt.components);

	// The sim machine really holds the files, and uninstalls cleanly.
	let snap = sim.snapshot();
	assert!(snap.contains_key(Path::new("/sim/work/app/APP")));
	assert!(snap.contains_key(Path::new(
		"/sim/home/.local/share/applications/fixture-app.desktop"
	)));
	assert!(sim.registry().contains_key("file_type:.fix"));
	uninstall(
		&mut sim,
		Some(&project),
		Path::new("/sim/work/app"),
		&mut RunOptions::default(),
	)
	.unwrap();
	assert!(!sim.snapshot().contains_key(Path::new("/sim/work/app")));
	assert!(sim.registry().is_empty());
}

#[test]
fn component_selection_is_validated() {
	let project = project();
	let payload = payload(&project);
	let sim = &mut SimMachine::new();
	sim.mkdir_all(Path::new("/sim/work"));
	let mut req = request(Path::new("/sim/work/app"));
	req.components = Some(vec!["nope".into()]);
	assert!(matches!(
		install(sim, &project, &payload, &req, &mut RunOptions::default()),
		Err(EngineError::UnknownComponent(_))
	));
	req.components = Some(vec!["winhelper".into()]);
	assert!(matches!(
		install(sim, &project, &payload, &req, &mut RunOptions::default()),
		Err(EngineError::NotApplicable(..))
	));
	req.components = Some(vec!["winhelper".into()]);
	req.target = "windows-x86_64".into();
	let r = install(sim, &project, &payload, &req, &mut RunOptions::default()).unwrap();
	assert_eq!(r.components, vec!["game", "winhelper"]);
	assert!(
		sim.snapshot()
			.contains_key(Path::new("/sim/work/app/helper.exe"))
	);
}

#[test]
fn launch_offers_an_update_first() {
	let project = project();
	let payload = payload(&project);
	let mut sim = SimMachine::new();
	sim.mkdir_all(Path::new("/sim/work"));
	let dir = Path::new("/sim/work/app");
	install(
		&mut sim,
		&project,
		&payload,
		&request(dir),
		&mut RunOptions::default(),
	)
	.unwrap();

	assert_eq!(
		launch(&mut sim, dir, None, false).unwrap(),
		LaunchOutcome::Launched
	);
	assert_eq!(sim.launched()[0].0, Path::new("/sim/work/app/APP"));

	let feed = feed_for("1.2.0", b"x", "linux-x86_64:canvas");
	assert_eq!(
		launch(&mut sim, dir, Some(&feed), false).unwrap(),
		LaunchOutcome::UpdateAvailable("1.2.0".into())
	);
	assert_eq!(sim.launched().len(), 1);
	assert_eq!(
		launch(&mut sim, dir, Some(&feed), true).unwrap(),
		LaunchOutcome::Launched
	);
	assert_eq!(sim.launched().len(), 2);
	assert!(matches!(
		launch(&mut sim, Path::new("/sim/none"), None, false),
		Err(EngineError::NotInstalled(_))
	));
}

#[test]
fn a_crash_leaves_a_journal_that_the_next_run_replays() {
	let project = project();
	let payload = payload(&project);
	let root = tempfile::tempdir().unwrap();
	let mut m = machine(root.path());
	let before = tree(root.path());

	// A panic in the progress callback stands in for the process dying mid-commit.
	let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		let mut cb = |p: &Progress| {
			assert!(p.files_done < 3, "simulated crash");
			true
		};
		let mut opts = RunOptions {
			on_progress: Some(&mut cb),
			..RunOptions::default()
		};
		let _ = install(
			&mut m,
			&project,
			&payload,
			&request(&root.path().join("app")),
			&mut opts,
		);
	}));
	assert!(crashed.is_err());
	assert_ne!(tree(root.path()), before, "the crash left a half install");

	assert!(li_engine::recover(&mut m, "hq.liminal.fixture").unwrap());
	assert_eq!(tree(root.path()), before);
	assert!(!li_engine::recover(&mut m, "hq.liminal.fixture").unwrap());
}

fn helper_project() -> li_engine::Project {
	li_engine::Project::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/helper"))
		.expect("helper fixture loads")
}

fn helper_request() -> li_engine::InstallRequest {
	request(Path::new("/sim/work/helper"))
}

fn helper_machine() -> SimMachine {
	let sim = SimMachine::new();
	sim.mkdir_all(Path::new("/sim/work"));
	sim
}

const STARTUP_FILE: &str = "/sim/home/.config/autostart/helper-at-login.desktop";

#[test]
fn a_shortcut_in_startup_with_arguments_installs_and_uninstalls() {
	let project = helper_project();
	let payload = payload(&project);
	let mut sim = helper_machine();
	let before = sim.snapshot();

	let plan = preview(&project, &payload, &helper_request(), &sim).unwrap();
	assert!(plan.ops.iter().any(|o| matches!(o,
		Operation::Shortcut { place, args: Some(a), .. } if place == "startup" && a == "--background")));
	// The program to run is the last thing before the receipt, so an undo reaches it first.
	let n = plan.ops.len();
	assert!(
		matches!(&plan.ops[n - 2], Operation::UninstallPre { run, wait_ms: 3000, .. } if run == "{install}/HELPER")
	);

	let receipt = install(
		&mut sim,
		&project,
		&payload,
		&helper_request(),
		&mut RunOptions::default(),
	)
	.unwrap();
	let startup = String::from_utf8(sim.read(Path::new(STARTUP_FILE)).unwrap()).unwrap();
	assert!(
		startup.contains("Exec=\"/sim/work/helper/HELPER\" --background\n"),
		"{startup}"
	);
	let stop = String::from_utf8(
		sim.read(Path::new(
			"/sim/home/.local/share/applications/stop-helper.desktop",
		))
		.unwrap(),
	)
	.unwrap();
	assert!(
		stop.contains("Exec=\"/sim/work/helper/HELPER\" --quit\n"),
		"{stop}"
	);
	// The plain shortcut has no arguments after the quoted program.
	let plain = String::from_utf8(
		sim.read(Path::new(
			"/sim/home/.local/share/applications/helper.desktop",
		))
		.unwrap(),
	)
	.unwrap();
	assert!(
		plain.contains("Exec=\"/sim/work/helper/HELPER\"\n"),
		"{plain}"
	);
	// Nothing ran at install, and the receipt keeps the program for later.
	assert!(sim.runs().is_empty() && sim.launched().is_empty());
	assert!(
		receipt
			.operations
			.iter()
			.any(|o| matches!(o, Operation::UninstallPre { args, .. } if args == "--quit"))
	);

	let report = uninstall(
		&mut sim,
		Some(&project),
		Path::new("/sim/work/helper"),
		&mut RunOptions::default(),
	)
	.unwrap();
	assert!(report.restart_pending.is_empty());
	assert_eq!(
		sim.snapshot(),
		before,
		"the startup shortcut goes with the rest"
	);
}

#[test]
fn the_pre_uninstall_program_runs_first_with_its_arguments_and_waits() {
	let project = helper_project();
	let payload = payload(&project);
	let mut sim = helper_machine();
	install(
		&mut sim,
		&project,
		&payload,
		&helper_request(),
		&mut RunOptions::default(),
	)
	.unwrap();

	// The program holds HELPER open until it is told to quit, as a running helper does.
	let held = Path::new("/sim/work/helper/HELPER");
	sim.lock(held);
	sim.on_run(move |m, program, args| {
		if program == held && args == ["--quit"] {
			m.unlock(held);
			Some(0)
		} else {
			None
		}
	});
	let report = uninstall(
		&mut sim,
		Some(&project),
		Path::new("/sim/work/helper"),
		&mut RunOptions::default(),
	)
	.unwrap();
	assert_eq!(
		sim.runs(),
		vec![(held.to_owned(), vec!["--quit".to_owned()])]
	);
	assert!(
		report.restart_pending.is_empty(),
		"the file was let go in time"
	);
	assert!(!sim.snapshot().contains_key(Path::new("/sim/work/helper")));
}

#[test]
fn a_failing_pre_uninstall_program_stops_the_uninstall_unless_failure_is_ignored() {
	let mut project = helper_project();
	let payload = payload(&project);
	let mut sim = helper_machine();
	install(
		&mut sim,
		&project,
		&payload,
		&helper_request(),
		&mut RunOptions::default(),
	)
	.unwrap();
	sim.on_run(|_, _, _| Some(7));
	let before = sim.snapshot();
	let err = uninstall(
		&mut sim,
		Some(&project),
		Path::new("/sim/work/helper"),
		&mut RunOptions::default(),
	)
	.unwrap_err();
	assert!(err.to_string().contains("exited with 7"), "{err}");
	assert_eq!(sim.snapshot(), before, "nothing was removed");

	// With `ignore_failure` the same exit code lets the uninstall go on.
	project.installer.uninstall.pre[0].ignore_failure = true;
	let mut sim2 = helper_machine();
	install(
		&mut sim2,
		&project,
		&payload,
		&helper_request(),
		&mut RunOptions::default(),
	)
	.unwrap();
	sim2.on_run(|_, _, _| Some(7));
	uninstall(
		&mut sim2,
		Some(&project),
		Path::new("/sim/work/helper"),
		&mut RunOptions::default(),
	)
	.unwrap();
	assert!(!sim2.snapshot().contains_key(Path::new("/sim/work/helper")));
}

#[test]
fn a_file_still_in_use_is_removed_at_the_next_restart_and_reported() {
	let project = helper_project();
	let payload = payload(&project);
	let mut sim = helper_machine();
	install(
		&mut sim,
		&project,
		&payload,
		&helper_request(),
		&mut RunOptions::default(),
	)
	.unwrap();
	let held = Path::new("/sim/work/helper/HELPER");
	sim.lock(held);

	let report = uninstall(
		&mut sim,
		Some(&project),
		Path::new("/sim/work/helper"),
		&mut RunOptions::default(),
	)
	.unwrap();
	// The held file and the folder that holds it wait for the restart; the rest is gone.
	assert_eq!(
		report.restart_pending,
		vec![held.to_owned(), Path::new("/sim/work/helper").to_owned()]
	);
	assert_eq!(sim.scheduled_removals(), report.restart_pending);
	let snap = sim.snapshot();
	assert!(snap.contains_key(held));
	assert!(!snap.contains_key(Path::new("/sim/work/helper/helper.cfg")));
	assert!(!snap.contains_key(Path::new(STARTUP_FILE)));
	assert!(!snap.contains_key(Path::new("/sim/work/helper/.liminal/receipt.json")));
}

#[test]
fn an_update_runs_the_pre_uninstall_program_before_it_replaces_files() {
	let project = helper_project();
	let payload = payload(&project);
	let mut sim = helper_machine();
	install(
		&mut sim,
		&project,
		&payload,
		&helper_request(),
		&mut RunOptions::default(),
	)
	.unwrap();
	// Repair of a damaged file replaces it, so the program runs first; a repair that has nothing
	// to do does not run it.
	repair(
		&mut sim,
		&payload,
		Path::new("/sim/work/helper"),
		&mut RunOptions::default(),
	)
	.unwrap();
	assert!(sim.runs().is_empty());
	sim.remove_file(Path::new("/sim/work/helper/helper.cfg"))
		.unwrap();
	repair(
		&mut sim,
		&payload,
		Path::new("/sim/work/helper"),
		&mut RunOptions::default(),
	)
	.unwrap();
	assert_eq!(sim.runs().len(), 1);
	assert_eq!(sim.runs()[0].1, vec!["--quit".to_owned()]);
	// Installing again over the same files changes nothing and runs nothing.
	install(
		&mut sim,
		&project,
		&payload,
		&helper_request(),
		&mut RunOptions::default(),
	)
	.unwrap();
	assert_eq!(sim.runs().len(), 1);
}
