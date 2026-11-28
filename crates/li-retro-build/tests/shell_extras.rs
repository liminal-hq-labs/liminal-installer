// The helper example compiled and run: a Startup shortcut, a command line, a program to run first and a file left for the restart.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `examples/startup-helper` has all four. The compiler writes format 6 for it (and format 5 for every other
//! project), the Windows 95 flow performs the operations on the host simulator, and DOS and Windows 3.1
//! leave them out with a warning.

use std::path::{Path, PathBuf};

use li_engine::Project;
use li_retro::app;
use li_retro::install::{RECEIPT, Receipt};
use li_retro::model::{self, Op};
use li_retro::sim::{GuiOp, HostPlatform, ScriptGui};

fn helper_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/startup-helper")
}

fn helper() -> Project {
	Project::load(&helper_dir()).expect("load helper")
}

fn win32(c: &li_retro_build::Compiled, root: &Path) -> HostPlatform {
	std::fs::create_dir_all(root.join("Program Files")).unwrap();
	let mut p = HostPlatform::new(root, c.lpk().unwrap());
	p.native = true;
	p.with_windows();
	let mut gui = ScriptGui::new();
	gui.on(
		"licence",
		vec![GuiOp::Choose("licence.accepted".into(), "yes".into())],
	);
	p.gui = Some(gui);
	p
}

const DIR: &str = "C:\\Program Files\\Helper Sample";

#[test]
fn the_compiler_writes_format_6_for_the_helper_and_5_for_the_others() {
	let c = li_retro_build::compile_win32(&helper(), "windows-x86", "1.0.0").unwrap();
	assert_eq!(c.project.format, model::FORMAT);
	assert!(
		c.project.ops.contains(&Op::Shortcut {
			name: "Helper Sample".into(),
			target: "HELPER.EXE".into(),
			batch: String::new()
		}),
		"{:?}",
		c.project.ops
	);
	assert!(c.project.ops.contains(&Op::Link {
		name: "Stop Helper Sample".into(),
		target: "HELPER.EXE".into(),
		args: "/quit".into(),
		startup: false
	}));
	assert!(c.project.ops.contains(&Op::Link {
		name: "Helper Sample at login".into(),
		target: "HELPER.EXE".into(),
		args: "/background".into(),
		startup: true
	}));
	assert!(c.project.ops.contains(&Op::PreUninstall {
		run: "STOP.BAT".into(),
		args: "/quit".into(),
		wait_ms: 5000,
		ignore_failure: true
	}));
	// The program to run belongs to no component, so it is always recorded.
	let at = c
		.project
		.ops
		.iter()
		.position(|o| matches!(o, Op::PreUninstall { .. }))
		.unwrap();
	assert_eq!(c.project.owners[at], model::NO_OWNER);
	// It round-trips through the hand-written decoder.
	let bytes = c.project.encode().unwrap();
	assert_eq!(model::Project::decode(&bytes).unwrap(), c.project);

	// A project that uses none of it stays at 5, so the DOS and Windows 3.1 runtimes still read it.
	let plain = Project::load(&helper_dir().join("../lt-fizz")).unwrap();
	let c = li_retro_build::compile_win32(&plain, "windows-x86", "1.0.0").unwrap();
	assert_eq!(c.project.format, model::FORMAT_BASE);
}

#[test]
fn the_windows_95_flow_makes_the_links_runs_the_program_and_leaves_a_locked_file() {
	let tmp = tempfile::tempdir().unwrap();
	let c = li_retro_build::compile_win32(&helper(), "windows-x86", "1.0.0").unwrap();
	let mut p = win32(&c, tmp.path());
	assert_eq!(app::run(&mut p, ""), 0);
	let startup = "C:\\WINDOWS\\Start Menu\\Programs\\StartUp\\Helper Sample at login.lnk";
	let lnk = std::fs::read_to_string(p.host(startup)).unwrap();
	assert!(lnk.contains("arguments=/background\r\n"), "{lnk}");
	let stop = std::fs::read_to_string(
		p.host("C:\\WINDOWS\\Start Menu\\Programs\\Helper Sample\\Stop Helper Sample.lnk"),
	)
	.unwrap();
	assert!(stop.contains("arguments=/quit\r\n"), "{stop}");
	let text = std::fs::read_to_string(p.host(&format!("{DIR}\\{RECEIPT}"))).unwrap();
	assert!(text.contains(&format!("\r\nS {startup}\r\n")), "{text}");
	assert!(
		text.contains(&format!("\r\nU {DIR}\\STOP.BAT|/quit|5000|1\r\n")),
		"{text}"
	);
	assert_eq!(
		Receipt::parse(&text).unwrap().pre[0].run,
		format!("{DIR}\\STOP.BAT")
	);

	// The helper is running (HELPER.LOG held open) and nothing lets go of it.
	let mut u = win32(&c, tmp.path());
	u.locked.push(format!("{DIR}\\HELPER.LOG"));
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert_eq!(
		u.programs,
		[(format!("{DIR}\\STOP.BAT"), "/quit".to_string(), 5000)]
	);
	assert!(!u.host(startup).exists());
	assert_eq!(
		u.restart_removals,
		[format!("{DIR}\\HELPER.LOG"), DIR.to_string()]
	);
	assert!(u.host(&format!("{DIR}\\HELPER.LOG")).exists());
	assert!(!u.host(&format!("{DIR}\\HELPER.EXE")).exists());
}

#[test]
fn dos_and_windows_3_1_leave_the_four_out_and_say_so() {
	// The helper's own files are for Windows 95; give DOS and Windows 3.1 the same files under their own targets' names.
	let mut project = helper();
	project.components.components.truncate(1);
	project.components.components[0].targets.clear();
	for rule in &mut project.components.components[0].files {
		rule.from = rule.from.replace("{target}", "windows-x86");
	}
	for (id, renderer) in [("dos-x86", "tui-retro"), ("win16-x86", "win16")] {
		project.installer.targets.push(li_model::Target {
			id: id.into(),
			renderer: renderer.into(),
			theme: None,
			outputs: vec![],
		});
	}
	for (c, name) in [
		(
			li_retro_build::compile(&project, "dos-x86", "1.0.0").unwrap(),
			"DOS",
		),
		(
			li_retro_build::compile_win16(&project, "win16-x86", "1.0.0").unwrap(),
			"Windows 3.1",
		),
	] {
		assert_eq!(c.project.format, model::FORMAT_BASE, "{name}");
		let w = c.warnings.join("\n");
		assert!(
			w.contains("`Helper Sample at login`") && w.contains("`startup`"),
			"{name}: {w}"
		);
		assert!(
			w.contains("`Stop Helper Sample` has arguments"),
			"{name}: {w}"
		);
		assert!(w.contains("`[[uninstall.pre]]` runs"), "{name}: {w}");
		// The plain items remain: the program and the two others as launchers or items, never the startup one.
		let names: Vec<_> = c
			.project
			.ops
			.iter()
			.filter_map(|o| match o {
				Op::Shortcut { name, .. } => Some(name.as_str()),
				_ => None,
			})
			.collect();
		assert_eq!(names, ["Helper Sample", "Stop Helper Sample"], "{name}");
	}
}
