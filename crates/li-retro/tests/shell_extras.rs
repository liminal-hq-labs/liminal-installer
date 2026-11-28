// A shortcut in the Startup folder, a shortcut's command line, a program run before the files go, and a file left for the next restart.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A hand-built format 6 project installed on the host simulator, which behaves as a Windows 95
//! machine and can hold a file open.

use std::path::Path;

use li_lpk::writer::{FileSpec, Options, pack};
use li_retro::Error;
use li_retro::install::{RECEIPT, Receipt, install, uninstall};
use li_retro::model::*;
use li_retro::pack::Pack;
use li_retro::sim::HostPlatform;
use li_retro::ui::Values;

const DIR: &str = "C:\\APP";
const STARTUP: &str = "C:\\WINDOWS\\Start Menu\\Programs\\StartUp\\Helper at login.lnk";
const PROGRAMS: &str = "C:\\WINDOWS\\Start Menu\\Programs\\T";

fn fixture(ignore_failure: bool) -> (Project, Vec<u8>) {
	let files = [
		("HELPER.EXE", vec![b'h'; 64]),
		("HELPER.INI", vec![b'i'; 32]),
	];
	let entries = files
		.iter()
		.enumerate()
		.map(|(i, (dest, data))| FileEntry {
			component: 0,
			dest: dest.to_string(),
			stored: format!("F{:07X}", i + 1),
			size: data.len() as u32,
			crc: li_lpk::crc32(data),
		})
		.collect::<Vec<_>>();
	let ops = vec![
		Op::Shortcut {
			name: "Helper".into(),
			target: "HELPER.EXE".into(),
			batch: String::new(),
		},
		Op::Link {
			name: "Stop Helper".into(),
			target: "HELPER.EXE".into(),
			args: "/quit".into(),
			startup: false,
		},
		Op::Link {
			name: "Helper at login".into(),
			target: "HELPER.EXE".into(),
			args: "/background".into(),
			startup: true,
		},
		Op::PreUninstall {
			run: "HELPER.EXE".into(),
			args: "/quit".into(),
			wait_ms: 3000,
			ignore_failure,
		},
	];
	let project = Project {
		format: 6,
		id: "t".into(),
		name: "T".into(),
		publisher: "P".into(),
		version: "1".into(),
		target: "windows-x86:win32-classic".into(),
		default_dir: DIR.into(),
		theme: Theme::default(),
		strings: vec![],
		components: vec![Component {
			id: "core".into(),
			name: "Core".into(),
			required: true,
			default: true,
		}],
		flows: vec![],
		screens: vec![],
		owners: vec![NO_OWNER; ops.len()],
		ops,
		files: entries.clone(),
	};
	let mut specs: Vec<FileSpec> = entries
		.iter()
		.zip(&files)
		.map(|(e, (_, data))| FileSpec::new(e.stored.as_str(), data.clone()))
		.collect();
	// The kept uninstaller reads the compiled project back from the pack.
	specs.push(FileSpec::new(
		li_retro::pack::PROJECT_FILE,
		project.encode().unwrap(),
	));
	let packed = pack(&specs, &Options::default()).unwrap();
	(project, packed.volumes.into_iter().next().unwrap())
}

fn machine(root: &Path, data: Vec<u8>) -> HostPlatform {
	let mut p = HostPlatform::new(root, data);
	p.native = true;
	p.with_windows();
	p
}

fn run_install(p: &mut HostPlatform, project: &Project) -> Result<(), Error> {
	let mut pk = Pack::open(p)?;
	install(
		p,
		project,
		&mut pk,
		DIR,
		&[],
		&Values::default(),
		&mut |_, _| true,
	)
}

fn receipt_text(p: &HostPlatform) -> String {
	std::fs::read_to_string(p.host(&format!("{DIR}\\{RECEIPT}"))).unwrap()
}

#[test]
fn the_startup_shortcut_and_the_arguments_are_made_recorded_and_removed() {
	let root = tempfile::tempdir().unwrap();
	let (project, data) = fixture(false);
	project.validate().unwrap();
	let mut p = machine(root.path(), data);
	let before = p.tree();
	run_install(&mut p, &project).unwrap();

	// The plain item has no command line; the other two do; only one is in the Startup folder.
	let plain = std::fs::read_to_string(p.host(&format!("{PROGRAMS}\\Helper.lnk"))).unwrap();
	assert!(!plain.contains("arguments="), "{plain}");
	let stop = std::fs::read_to_string(p.host(&format!("{PROGRAMS}\\Stop Helper.lnk"))).unwrap();
	assert!(stop.contains("arguments=/quit\r\n"), "{stop}");
	let startup = std::fs::read_to_string(p.host(STARTUP)).unwrap();
	assert!(
		startup.contains("target=C:\\APP\\HELPER.EXE\r\narguments=/background\r\n"),
		"{startup}"
	);
	assert!(!p.host(&format!("{PROGRAMS}\\Helper at login.lnk")).exists());

	// The receipt says where each one is, and what to run before the files go.
	let text = receipt_text(&p);
	assert!(text.contains(&format!("\r\nS {STARTUP}\r\n")), "{text}");
	assert!(
		text.contains("\r\nU C:\\APP\\HELPER.EXE|/quit|3000|0\r\n"),
		"{text}"
	);
	let parsed = Receipt::parse(&text).unwrap();
	assert_eq!(parsed.pre.len(), 1);
	assert_eq!(parsed.pre[0].wait_ms, 3000);

	uninstall(&mut p, DIR, &mut |_, _| true).unwrap();
	assert!(!p.host(STARTUP).exists(), "the Startup shortcut is removed");
	assert_eq!(
		p.tree().len(),
		before.len() + 3,
		"only the folders the simulator makes remain: {:?}",
		p.tree()
	);
	assert!(!p.host(&format!("{DIR}\\HELPER.EXE")).exists());
}

#[test]
fn the_program_runs_before_the_files_go_and_lets_go_of_its_file() {
	let root = tempfile::tempdir().unwrap();
	let (project, data) = fixture(false);
	let mut p = machine(root.path(), data);
	run_install(&mut p, &project).unwrap();
	// HELPER.EXE is held open until HELPER.EXE /quit is run.
	p.locked.push("C:\\APP\\HELPER.EXE".into());
	p.releases
		.insert("HELPER.EXE".into(), vec!["C:\\APP\\HELPER.EXE".into()]);
	uninstall(&mut p, DIR, &mut |_, _| true).unwrap();
	assert_eq!(
		p.programs,
		vec![("C:\\APP\\HELPER.EXE".to_string(), "/quit".to_string(), 3000)]
	);
	assert!(p.restart_removals.is_empty(), "it was let go in time");
	assert!(p.reports.is_empty());
	assert!(!p.host(DIR).exists());
}

#[test]
fn a_failing_program_stops_the_uninstall_before_anything_is_removed_unless_failure_is_ignored() {
	let root = tempfile::tempdir().unwrap();
	let (project, data) = fixture(false);
	let mut p = machine(root.path(), data);
	run_install(&mut p, &project).unwrap();
	p.exit_codes.insert("HELPER.EXE".into(), 7);
	let err = uninstall(&mut p, DIR, &mut |_, _| true).unwrap_err();
	assert!(err.to_string().contains("exited with 7"), "{err}");
	assert!(p.host(&format!("{DIR}\\HELPER.EXE")).exists());
	assert!(
		p.host(&format!("{DIR}\\{RECEIPT}")).exists(),
		"the receipt stays, so it can be run again"
	);

	let root = tempfile::tempdir().unwrap();
	let (project, data) = fixture(true);
	let mut p = machine(root.path(), data);
	run_install(&mut p, &project).unwrap();
	p.exit_codes.insert("HELPER.EXE".into(), 7);
	uninstall(&mut p, DIR, &mut |_, _| true).unwrap();
	assert!(!p.host(DIR).exists());

	// A program that outlasts its wait is a failure too.
	let root = tempfile::tempdir().unwrap();
	let (project, data) = fixture(false);
	let mut p = machine(root.path(), data);
	run_install(&mut p, &project).unwrap();
	p.exit_codes.insert("HELPER.EXE".into(), u32::MAX);
	let err = uninstall(&mut p, DIR, &mut |_, _| true).unwrap_err();
	assert!(
		err.to_string().contains("still running after 3000 ms"),
		"{err}"
	);
}

#[test]
fn a_file_still_in_use_is_left_for_the_restart_and_the_person_is_told() {
	let root = tempfile::tempdir().unwrap();
	let (project, data) = fixture(true);
	let mut p = machine(root.path(), data);
	run_install(&mut p, &project).unwrap();
	// Nothing lets go of it.
	p.locked.push("C:\\APP\\HELPER.EXE".into());
	uninstall(&mut p, DIR, &mut |_, _| true).unwrap();
	assert_eq!(p.restart_removals, ["C:\\APP\\HELPER.EXE", "C:\\APP"]);
	assert_eq!(
		p.reports,
		["1 file is still in use. Windows will remove it the next time it starts."]
	);
	assert!(
		p.notices
			.iter()
			.any(|n| n == "removed at restart: C:\\APP\\HELPER.EXE")
	);
	assert!(!p.host(&format!("{DIR}\\HELPER.INI")).exists());
	assert!(!p.host(&format!("{DIR}\\{RECEIPT}")).exists());
}

#[test]
fn an_install_over_an_earlier_one_runs_its_program_first() {
	let root = tempfile::tempdir().unwrap();
	let (project, data) = fixture(false);
	let mut p = machine(root.path(), data);
	run_install(&mut p, &project).unwrap();
	assert!(p.programs.is_empty(), "a first install runs nothing");
	run_install(&mut p, &project).unwrap();
	assert_eq!(p.programs.len(), 1);
	assert_eq!(p.programs[0].1, "/quit");
}

#[test]
fn the_new_operations_need_format_6_and_the_old_ones_do_not() {
	let (mut project, _) = fixture(false);
	project.format = 5;
	assert!(project.validate().is_err());
	project.ops.retain(|o| !o.needs_format_6());
	project.owners.truncate(project.ops.len());
	project.validate().unwrap();
	let (project, _) = fixture(false);
	let bytes = project.encode().unwrap();
	assert_eq!(Project::decode(&bytes).unwrap(), project);
}
