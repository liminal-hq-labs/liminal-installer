// File types: the compiled operation, the decoder's refusal of what it does not know, and the install, uninstall and rollback of an association.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Planboard declares a file type on its Windows 95 component (`.planboard`) and on its Windows 3.1
//! one (`.pbd`, because Windows 3.x associates three letters). The host simulator keeps the
//! associations a machine has, so a test can seed what owned an extension, install, and check what
//! the receipt remembers and what the uninstall puts back.

use std::path::{Path, PathBuf};

use li_engine::Project;
use li_retro::app;
use li_retro::install::{RECEIPT, Receipt};
use li_retro::model::{self, Op};
use li_retro::sim::{GuiOp, HostPlatform, ScriptGui};

fn planboard() -> Project {
	let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/planboard");
	Project::load(&dir).expect("load planboard")
}

fn gui() -> ScriptGui {
	let mut g = ScriptGui::new();
	g.on(
		"licence",
		vec![GuiOp::Choose("licence.accepted".into(), "yes".into())],
	);
	g
}

fn win32(c: &li_retro_build::Compiled, root: &Path) -> HostPlatform {
	std::fs::create_dir_all(root.join("Program Files")).unwrap();
	let mut p = HostPlatform::new(root, c.lpk().unwrap());
	p.native = true;
	p.gui = Some(gui());
	p
}

fn win16(c: &li_retro_build::Compiled, root: &Path) -> HostPlatform {
	let mut p = HostPlatform::new(root, c.lpk().unwrap());
	p.program_manager = true;
	p.gui = Some(gui());
	p
}

fn compile32() -> li_retro_build::Compiled {
	li_retro_build::compile_win32(&planboard(), "windows-x86", "1.0.0").expect("compile")
}

fn compile16() -> li_retro_build::Compiled {
	li_retro_build::compile_win16(&planboard(), "win16-x86", "1.0.0").expect("compile")
}

fn receipt_text(p: &HostPlatform) -> String {
	let name = p
		.tree()
		.into_iter()
		.find(|t| t.ends_with(RECEIPT))
		.expect("a receipt");
	std::fs::read_to_string(p.host(&name)).unwrap()
}

fn file_types(c: &li_retro_build::Compiled) -> Vec<&Op> {
	c.project
		.ops
		.iter()
		.filter(|o| matches!(o, Op::FileType { .. }))
		.collect()
}

#[test]
fn the_compiler_emits_the_operation_for_windows_and_warns_for_dos() {
	let c = compile32();
	assert_eq!(c.project.format, model::FORMAT_BASE);
	assert_eq!(
		file_types(&c),
		[&Op::FileType {
			ext: "planboard".into(),
			prog_id: "hq.liminal.planboard.planboard".into(),
			name: "Planboard board".into(),
			// The component is shared with other systems, which name the program without `.exe`.
			target: "planboard.exe".into(),
		}]
	);
	let c = compile16();
	assert_eq!(
		file_types(&c),
		[&Op::FileType {
			ext: "pbd".into(),
			prog_id: "hq.liminal.planboard.pbd".into(),
			name: "Planboard board".into(),
			target: "PLANBRD.EXE".into(),
		}]
	);
}

#[test]
fn dos_keeps_the_warning_and_makes_no_operation() {
	let mut project = Project::load(&planboard_dir().join("../lt-fizz")).unwrap();
	project.components.components[0]
		.file_types
		.push(li_model::project::FileTypeRule {
			extension: ".fiz".into(),
			name: "Fizz game".into(),
			open_with: Some("{install}/FIZZ.EXE".into()),
		});
	let c = li_retro_build::compile(&project, "dos-x86", "1.0.0").expect("compile");
	assert!(file_types(&c).is_empty());
	assert!(
		c.warnings
			.iter()
			.any(|w| w.contains("`.fiz`") && w.contains("skipped")),
		"{:?}",
		c.warnings
	);
}

fn planboard_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/planboard")
}

#[test]
fn windows_3_1_skips_an_extension_longer_than_three_letters() {
	let mut project = planboard();
	for c in project.components.components.iter_mut() {
		for ft in c.file_types.iter_mut() {
			ft.extension = ".planboard".into();
		}
	}
	let c = li_retro_build::compile_win16(&project, "win16-x86", "1.0.0").expect("compile");
	assert!(file_types(&c).is_empty());
	assert!(
		c.warnings
			.iter()
			.any(|w| w.contains("one to three letters")),
		"{:?}",
		c.warnings
	);
}

/// The postcard encoding of `p` as a compiler of format 3 and older wrote it: a component has no
/// starting state and the project has no owners.
fn legacy(p: &model::Project) -> Vec<u8> {
	let components: Vec<_> = p
		.components
		.iter()
		.map(|c| (&c.id, &c.name, c.required))
		.collect();
	postcard::to_allocvec(&(
		p.format,
		&p.id,
		&p.name,
		&p.publisher,
		&p.version,
		&p.target,
		&p.default_dir,
		&p.theme,
		&p.strings,
		components,
		&p.flows,
		&p.screens,
		&p.files,
		&p.ops,
	))
	.unwrap()
}

#[test]
fn the_decoder_reads_format_3_and_refuses_what_a_format_2_project_cannot_hold() {
	let c = compile32();
	let bytes = c.project.encode().unwrap();
	let back = model::Project::decode(&bytes).unwrap();
	assert_eq!(back, c.project);
	let by_serde: model::Project = postcard::from_bytes(&bytes).unwrap();
	assert_eq!(back, by_serde);

	// The same project as an older compiler wrote it, which had no starting state for a component
	// and no owner for an operation. Format 2's tag 3 is not an operation.
	let mut old = c.project.clone();
	old.format = 2;
	let err = model::Project::decode(&legacy(&old)).unwrap_err();
	assert!(format!("{err:?}").contains("newer"), "{err:?}");

	// Without the file type, the same project is a good format 2 project, with every component
	// ticked and every operation unconditional.
	old.ops.retain(|o| !matches!(o, Op::FileType { .. }));
	let read = model::Project::decode(&legacy(&old)).unwrap();
	assert_eq!(read.format, 2);
	assert!(read.components.iter().all(|c| c.default) && read.owners.is_empty());

	// Format 3 reads the same way, file type included.
	let mut three = c.project.clone();
	three.format = 3;
	let read = model::Project::decode(&legacy(&three)).unwrap();
	assert_eq!(read.format, 3);
	assert_eq!(read.ops, c.project.ops);

	// A format this runtime has never heard of is refused before anything is read.
	let mut future = c.project.clone();
	future.format = model::FORMAT + 1;
	let err = model::Project::decode(&future.encode().unwrap()).unwrap_err();
	assert!(format!("{err:?}").contains("newer"), "{err:?}");
}

#[test]
fn a_windows_95_install_takes_the_type_and_the_uninstall_gives_it_back() {
	let tmp = tempfile::tempdir().unwrap();
	let c = compile32();
	let mut p = win32(&c, tmp.path());
	p.associations.insert("planboard".into(), "textfile".into());
	assert_eq!(app::run(&mut p, ""), 0);
	assert_eq!(
		p.associations["planboard"],
		"hq.liminal.planboard.planboard"
	);
	assert_eq!(
		p.prog_ids["hq.liminal.planboard.planboard"],
		"Planboard board"
	);
	let lir = receipt_text(&p);
	assert!(
		lir.contains("\r\nA planboard|hq.liminal.planboard.planboard|C:\\Program Files\\Planboard\\planboard.exe|textfile\r\n"),
		"{lir}"
	);

	let mut u = win32(&c, tmp.path());
	u.associations = p.associations.clone();
	u.prog_ids = p.prog_ids.clone();
	u.registry = p.registry.clone();
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert_eq!(u.associations["planboard"], "textfile");
	assert!(u.prog_ids.is_empty());
}

#[test]
fn an_extension_nobody_owned_is_deleted_again() {
	let tmp = tempfile::tempdir().unwrap();
	let c = compile32();
	let mut p = win32(&c, tmp.path());
	assert_eq!(app::run(&mut p, ""), 0);
	let lir = receipt_text(&p);
	assert!(
		lir.contains("planboard.exe\r\n") && !lir.contains("planboard.exe|"),
		"no previous owner, no fourth field: {lir}"
	);
	let mut u = win32(&c, tmp.path());
	u.associations = p.associations.clone();
	u.prog_ids = p.prog_ids.clone();
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert!(u.associations.is_empty());
}

#[test]
fn an_empty_previous_owner_is_no_owner() {
	// An extension whose line was there but empty has nothing to give back: the uninstall deletes
	// the line and does not write `ext=` again.
	let tmp = tempfile::tempdir().unwrap();
	let c = compile32();
	let mut p = win32(&c, tmp.path());
	p.associations.insert("planboard".into(), String::new());
	assert_eq!(app::run(&mut p, ""), 0);
	let lir = receipt_text(&p);
	assert!(!lir.contains("planboard.exe|"), "{lir}");
	let mut u = win32(&c, tmp.path());
	u.associations = p.associations.clone();
	u.prog_ids = p.prog_ids.clone();
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert!(u.associations.is_empty(), "{:?}", u.associations);
}

#[test]
fn a_choice_made_since_the_install_is_left_alone() {
	let tmp = tempfile::tempdir().unwrap();
	let c = compile32();
	let mut p = win32(&c, tmp.path());
	p.associations.insert("planboard".into(), "textfile".into());
	assert_eq!(app::run(&mut p, ""), 0);
	let mut u = win32(&c, tmp.path());
	u.associations = p.associations.clone();
	u.prog_ids = p.prog_ids.clone();
	// The person pointed the extension at another program afterwards.
	u.associations
		.insert("planboard".into(), "someone.else".into());
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert_eq!(u.associations["planboard"], "someone.else");
	assert!(u.prog_ids.is_empty(), "our own type's keys go either way");
}

#[test]
fn installing_again_keeps_what_the_first_install_replaced() {
	let tmp = tempfile::tempdir().unwrap();
	let c = compile32();
	let mut p = win32(&c, tmp.path());
	p.associations.insert("planboard".into(), "textfile".into());
	assert_eq!(app::run(&mut p, ""), 0);
	let mut again = win32(&c, tmp.path());
	again.associations = p.associations.clone();
	again.prog_ids = p.prog_ids.clone();
	again.registry = p.registry.clone();
	assert_eq!(app::run(&mut again, ""), 0);
	let lir = receipt_text(&again);
	assert!(lir.contains("planboard.exe|textfile\r\n"), "{lir}");
	let mut u = win32(&c, tmp.path());
	u.associations = again.associations.clone();
	u.prog_ids = again.prog_ids.clone();
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert_eq!(u.associations["planboard"], "textfile");
}

#[test]
fn a_failed_install_puts_the_previous_owner_back() {
	let tmp = tempfile::tempdir().unwrap();
	let c = compile32();
	let mut ok = win32(&c, tmp.path());
	assert_eq!(app::run(&mut ok, ""), 0);
	// Everything written through the platform: the payload's files and the receipt, whose last byte fails.
	let lir = receipt_text(&ok);
	let r = Receipt::parse(&lir).unwrap();
	let total = r.files.iter().map(|f| u64::from(f.1)).sum::<u64>() + lir.len() as u64;

	let b = tempfile::tempdir().unwrap();
	let mut p = win32(&c, b.path());
	p.associations.insert("planboard".into(), "textfile".into());
	p.fail_after = Some(total - 1);
	assert_ne!(app::run(&mut p, ""), 0);
	assert_eq!(p.associations["planboard"], "textfile");
	assert!(p.prog_ids.is_empty());
	assert!(
		!p.tree().iter().any(|t| !t.ends_with('\\')),
		"no file is left: {:?}",
		p.tree()
	);
}

#[test]
fn a_windows_3_1_install_writes_the_extensions_line_and_restores_it() {
	let tmp = tempfile::tempdir().unwrap();
	let c = compile16();
	let mut p = win16(&c, tmp.path());
	p.associations
		.insert("pbd".into(), "C:\\WINDOWS\\NOTEPAD.EXE ^.pbd".into());
	assert_eq!(app::run(&mut p, ""), 0);
	let lir = receipt_text(&p);
	assert!(
		lir.contains(
			"\r\nA pbd|hq.liminal.planboard.pbd|C:\\PLANBOAR\\PLANBRD.EXE|C:\\WINDOWS\\NOTEPAD.EXE ^.pbd\r\n"
		),
		"{lir}"
	);
	assert_eq!(p.associations["pbd"], "C:\\PLANBOAR\\PLANBRD.EXE ^.pbd");

	let mut u = win16(&c, tmp.path());
	u.associations = p.associations.clone();
	u.group_names = p.group_names.clone();
	u.groups = p.groups.clone();
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert_eq!(u.associations["pbd"], "C:\\WINDOWS\\NOTEPAD.EXE ^.pbd");

	// With nothing there before, the line goes.
	let tmp = tempfile::tempdir().unwrap();
	let mut p = win16(&c, tmp.path());
	assert_eq!(app::run(&mut p, ""), 0);
	let mut u = win16(&c, tmp.path());
	u.associations = p.associations.clone();
	u.group_names = p.group_names.clone();
	u.groups = p.groups.clone();
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert!(u.associations.is_empty());
}

#[test]
fn a_shell_without_associations_records_nothing() {
	use li_retro::platform::Shell;
	let tmp = tempfile::tempdir().unwrap();
	let mut plain = HostPlatform::new(tmp.path(), Vec::new());
	assert!(!plain.file_types());
	assert_eq!(
		plain
			.register_file_type("x", "p", "n", "C:\\X.EXE")
			.unwrap(),
		None
	);
}
