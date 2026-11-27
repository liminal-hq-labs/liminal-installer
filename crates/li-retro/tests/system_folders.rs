// Installing a file into the Windows folder and the system folder: placing, backup, rollback and uninstall.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A hand-built format 5 project with one install-folder file and one file each in the Windows and
//! system folders, installed on the host simulator with its `C:\WINDOWS` and `C:\WINDOWS\SYSTEM`.

use std::path::Path;

use li_lpk::writer::{FileSpec, Options, pack};
use li_retro::Error;
use li_retro::install::{RECEIPT, Receipt, install, uninstall};
use li_retro::model::*;
use li_retro::pack::Pack;
use li_retro::sim::HostPlatform;
use li_retro::ui::Values;

const DIR: &str = "C:\\APP";
const SEEDED: &[u8] = b"someone else's saver";

fn body(tag: u8) -> Vec<u8> {
	vec![tag; 100]
}

/// The destination order the validator checks: `%SYSDIR%` before `%WINDIR%` before the install folder.
fn project(format: u16) -> (Project, Vec<u8>) {
	let files = [
		("%SYSDIR%\\S.DLL", "%SYSDIR%/S.DLL", body(b's')),
		("%WINDIR%\\A.SCR", "%WINDIR%/A.SCR", body(b'a')),
		("MAIN.EXE", "MAIN.EXE", body(b'm')),
	];
	let entries = files
		.iter()
		.map(|(dest, stored, data)| FileEntry {
			component: 0,
			dest: dest.to_string(),
			stored: stored.to_string(),
			size: data.len() as u32,
			crc: li_lpk::crc32(data),
		})
		.collect();
	let project = Project {
		format,
		id: "t".into(),
		name: "T".into(),
		publisher: "P".into(),
		version: "1".into(),
		target: "win16-x86:win16".into(),
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
		files: entries,
		ops: vec![],
		owners: vec![],
	};
	let specs: Vec<FileSpec> = files
		.iter()
		.map(|(_, stored, data)| FileSpec::new(*stored, data.clone()))
		.collect();
	let packed = pack(&specs, &Options::default()).unwrap();
	(project, packed.volumes.into_iter().next().unwrap())
}

fn machine(root: &Path, data: Vec<u8>) -> HostPlatform {
	let mut p = HostPlatform::new(root, data);
	p.with_windows();
	p
}

fn seed(p: &HostPlatform) {
	std::fs::write(p.host("C:\\WINDOWS\\A.SCR"), SEEDED).unwrap();
}

fn run_install(
	p: &mut HostPlatform,
	project: &Project,
	cancel_at: Option<u32>,
) -> Result<(), Error> {
	let mut pk = Pack::open(p)?;
	install(
		p,
		project,
		&mut pk,
		DIR,
		&[],
		&Values::default(),
		&mut |_, pr| cancel_at.is_none_or(|n| pr.files_done < n),
	)
}

fn run_uninstall(p: &mut HostPlatform) -> Result<u32, Error> {
	uninstall(p, DIR, &mut |_, _| true)
}

fn receipt_text(p: &HostPlatform) -> String {
	std::fs::read_to_string(p.host(&format!("{DIR}\\{RECEIPT}"))).unwrap()
}

#[test]
fn a_fresh_install_places_the_files_and_the_uninstall_leaves_the_folders_as_they_were() {
	let root = tempfile::tempdir().unwrap();
	let (project, data) = project(FORMAT);
	project.validate().unwrap();
	let mut p = machine(root.path(), data);
	let before = p.tree();
	run_install(&mut p, &project, None).unwrap();
	assert_eq!(
		std::fs::read(p.host("C:\\WINDOWS\\A.SCR")).unwrap(),
		body(b'a')
	);
	assert_eq!(
		std::fs::read(p.host("C:\\WINDOWS\\SYSTEM\\S.DLL")).unwrap(),
		body(b's')
	);
	assert!(p.host("C:\\APP\\MAIN.EXE").is_file());
	let text = receipt_text(&p);
	assert!(text.contains("F C:\\WINDOWS\\A.SCR 100 "), "{text}");
	assert!(text.contains("F C:\\WINDOWS\\SYSTEM\\S.DLL 100 "), "{text}");
	assert!(!text.contains("\r\nO "), "{text}");
	assert!(!p.tree().iter().any(|t| t.starts_with("APP\\BACKUP")));
	assert!(p.notices.is_empty());
	assert_eq!(run_uninstall(&mut p).unwrap(), 3);
	assert_eq!(p.tree(), before);
}

#[test]
fn a_file_that_was_there_is_backed_up_replaced_and_put_back() {
	let root = tempfile::tempdir().unwrap();
	let (project, data) = project(FORMAT);
	let mut p = machine(root.path(), data);
	seed(&p);
	let before = p.tree();
	run_install(&mut p, &project, None).unwrap();
	assert_eq!(
		std::fs::read(p.host("C:\\WINDOWS\\A.SCR")).unwrap(),
		body(b'a')
	);
	assert_eq!(
		std::fs::read(p.host("C:\\APP\\BACKUP\\WIN\\A.SCR")).unwrap(),
		SEEDED
	);
	assert_eq!(
		p.notices,
		["backup: C:\\WINDOWS\\A.SCR -> BACKUP\\WIN\\A.SCR"]
	);
	let text = receipt_text(&p);
	assert!(
		text.contains("\r\nO C:\\WINDOWS\\A.SCR|BACKUP\\WIN\\A.SCR\r\n"),
		"{text}"
	);
	// The receipt reads back, and a reader that predates `O` would ignore the line.
	let r = Receipt::parse(&text).unwrap();
	assert_eq!(
		r.overwritten,
		[(
			"C:\\WINDOWS\\A.SCR".to_string(),
			"BACKUP\\WIN\\A.SCR".to_string()
		)]
	);
	run_uninstall(&mut p).unwrap();
	assert_eq!(std::fs::read(p.host("C:\\WINDOWS\\A.SCR")).unwrap(), SEEDED);
	assert_eq!(p.tree(), before);
}

#[test]
fn a_cancel_restores_the_file_and_leaves_no_backup() {
	let root = tempfile::tempdir().unwrap();
	let (project, data) = project(FORMAT);
	let mut p = machine(root.path(), data);
	seed(&p);
	let before = p.tree();
	// Two files are done: the system file and the saver, which replaced the seeded one.
	let err = run_install(&mut p, &project, Some(2)).unwrap_err();
	assert!(matches!(err, Error::Cancelled), "{err:?}");
	assert_eq!(std::fs::read(p.host("C:\\WINDOWS\\A.SCR")).unwrap(), SEEDED);
	assert_eq!(p.tree(), before);
}

#[test]
fn a_failed_write_restores_the_file_and_leaves_no_backup() {
	let root = tempfile::tempdir().unwrap();
	let (project, data) = project(FORMAT);
	let mut p = machine(root.path(), data);
	seed(&p);
	let before = p.tree();
	// The first file takes 100 bytes; the saver fails part way, after it was truncated.
	p.fail_after = Some(150);
	let err = run_install(&mut p, &project, None).unwrap_err();
	assert!(matches!(err, Error::Io(_)), "{err:?}");
	assert_eq!(std::fs::read(p.host("C:\\WINDOWS\\A.SCR")).unwrap(), SEEDED);
	assert_eq!(p.tree(), before);
}

#[test]
fn a_reinstall_keeps_the_first_backup_and_makes_no_second() {
	let root = tempfile::tempdir().unwrap();
	let (project, data) = project(FORMAT);
	let mut p = machine(root.path(), data);
	seed(&p);
	let before = p.tree();
	run_install(&mut p, &project, None).unwrap();
	p.notices.clear();
	run_install(&mut p, &project, None).unwrap();
	assert!(p.notices.is_empty(), "{:?}", p.notices);
	assert_eq!(
		std::fs::read(p.host("C:\\APP\\BACKUP\\WIN\\A.SCR")).unwrap(),
		SEEDED
	);
	let text = receipt_text(&p);
	assert_eq!(text.matches("\r\nO ").count(), 1, "{text}");
	run_uninstall(&mut p).unwrap();
	assert_eq!(std::fs::read(p.host("C:\\WINDOWS\\A.SCR")).unwrap(), SEEDED);
	assert_eq!(p.tree(), before);
}

#[test]
fn a_receipt_keeps_a_network_path_whole() {
	// The Windows folder may be a network share; the receipt's full path must stay as written.
	let r = Receipt::parse(
		"LIR 1\r\ndir=C:\\APP\r\nF \\\\SERVER\\WIN\\A.SCR 1 00000001\r\nO \\\\SERVER\\WIN\\A.SCR|BACKUP\\WIN\\A.SCR\r\n",
	)
	.unwrap();
	assert_eq!(r.files[0].0, "\\\\SERVER\\WIN\\A.SCR");
	assert_eq!(r.overwritten.len(), 1);
}

#[test]
fn the_validator_refuses_a_percent_destination_before_format_5() {
	let (project, _) = project(4);
	assert!(project.validate().is_err());
	let (mut project, _) = project_for_validate();
	project.files[0].dest = "%TEMP%\\X".into();
	assert!(project.validate().is_err());
	project.files[0].dest = "%WINDIR%\\SUB\\X".into();
	assert!(project.validate().is_err());
	project.files[0].dest = "%WINDIR%\\".into();
	assert!(project.validate().is_err());
	project.files[0].dest = "%WINDIR%\\X.SCR".into();
	project.validate().unwrap();
}

fn project_for_validate() -> (Project, Vec<u8>) {
	let (mut project, data) = project(FORMAT);
	project.files.truncate(1);
	(project, data)
}

#[test]
fn a_machine_with_no_windows_folder_fails_and_rolls_back() {
	let root = tempfile::tempdir().unwrap();
	let (project, data) = project(FORMAT);
	// A Windows machine whose shell cannot say where its Windows folder is.
	let mut p = HostPlatform::new(root.path(), data);
	p.native = true;
	let before = p.tree();
	let err = run_install(&mut p, &project, None).unwrap_err();
	assert!(
		matches!(&err, Error::Io(m) if m.contains("no Windows folder")),
		"{err:?}"
	);
	assert_eq!(p.tree(), before);
}
