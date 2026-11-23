// The Windows 3.1 runtime's flow against the DOS runtime's: the same pages in the same order and the same receipt.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The Win16 runtime draws pages with Windows controls, so its pages are driven here through
//! `Platform::native_page` by [`li_retro::sim::accept_defaults`], which makes the choices the DOS
//! tests make with keys. Both run the same flow VM and installer engine over the same project, so
//! the sequence of `(step, screen)` pairs and the receipt must agree. `LI_UPDATE_GOLDEN=1`
//! rewrites the receipt golden.

use std::path::{Path, PathBuf};

use li_engine::Project;
use li_retro::Key;
use li_retro::app;
use li_retro::install::{RECEIPT, Receipt};
use li_retro::sim::{GuiOp, HostPlatform, ScriptGui};

fn fizz_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lt-fizz")
}

fn compile(target: &str) -> li_retro_build::Compiled {
	let project = Project::load(&fizz_dir()).expect("load lt-fizz");
	if target.starts_with("win16") {
		li_retro_build::compile_win16(&project, target, "1.0.0").expect("compile")
	} else {
		li_retro_build::compile(&project, target, "1.0.0").expect("compile")
	}
}

fn dos_keys() -> Vec<Key> {
	vec![
		Key::Enter,
		Key::BackTab,
		Key::BackTab,
		Key::Char(b' '),
		Key::Enter,
		Key::Enter,
		Key::Enter,
		Key::Enter,
		Key::Enter,
		Key::Enter,
	]
}

fn install_dos(root: &Path) -> HostPlatform {
	let c = compile("dos-x86");
	let mut p = HostPlatform::new(root, c.lpk().unwrap());
	p.keys(&dos_keys());
	assert_eq!(app::run(&mut p, ""), 0);
	p
}

/// A scripted native window that accepts the licence and takes every default.
fn accepting() -> ScriptGui {
	let mut g = ScriptGui::default();
	g.on(
		"licence",
		vec![GuiOp::Choose("licence.accepted".into(), "yes".into())],
	);
	g
}

fn win16(c: &li_retro_build::Compiled, root: &Path) -> HostPlatform {
	let mut p = HostPlatform::new(root, c.lpk().unwrap());
	p.gui = Some(accepting());
	p
}

fn install_win16(root: &Path) -> HostPlatform {
	let c = compile("win16-x86");
	let mut p = win16(&c, root);
	assert_eq!(app::run(&mut p, ""), 0);
	p
}

fn receipt(root: &Path) -> String {
	std::fs::read_to_string(root.join("LTFIZZ").join(RECEIPT)).unwrap()
}

#[test]
fn win16_compiles_with_widgets_a_title_and_no_cells() {
	let c = compile("win16-x86");
	assert_eq!(c.project.target, "win16-x86:win16");
	assert_eq!(c.project.default_dir, "C:\\LTFIZZ");
	assert!(c.project.screens.iter().all(|s| s.runs.len() == 4));
	let welcome = c
		.project
		.screens
		.iter()
		.find(|s| s.id == "welcome")
		.unwrap();
	assert_eq!(welcome.title, "Welcome");
	// The explanations are widgets here: the text cells that hold them on DOS are not kept.
	assert!(
		welcome
			.widgets
			.iter()
			.any(|w| matches!(w, li_retro::model::Widget::Text { template, .. } if template.contains("Setup program")))
	);
	assert!(c.project.files.iter().any(|f| f.dest == "SOUND\\ADLIB.DRV"));
}

#[test]
fn the_pages_come_in_the_same_order_as_dos() {
	let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
	let dos = install_dos(a.path());
	let win = install_win16(b.path());
	assert_eq!(dos.steps, win.steps);
	assert_eq!(
		win.steps,
		[
			"welcome",
			"licence",
			"folder",
			"components",
			"sound-card",
			"ready",
			"done"
		]
	);
	let g = win.gui.as_ref().unwrap();
	let titles: Vec<&str> = g.pages.iter().map(|p| p.1.as_str()).collect();
	assert_eq!(titles[0], "Welcome");
}

#[test]
fn the_tree_and_receipt_agree_with_dos() {
	let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
	let dos = install_dos(a.path());
	let win = install_win16(b.path());
	assert_eq!(dos.tree(), win.tree());
	let (rd, rw) = (receipt(a.path()), receipt(b.path()));
	// Only the target line and the game program (a different binary for each target) differ.
	let norm = |r: &str| {
		r.lines()
			.filter(|l| !l.starts_with("target=") && !l.starts_with("F FIZZ.EXE"))
			.collect::<Vec<_>>()
			.join("\n")
	};
	assert_eq!(norm(&rd), norm(&rw));
	assert!(rw.contains("target=win16-x86:win16\r\n"), "{rw}");
	let r = Receipt::parse(&rw).unwrap();
	assert_eq!(r.components, ["game", "sound", "levels"]);
	let dist = fizz_dir().join("installer/dist");
	assert_eq!(
		std::fs::read(b.path().join("LTFIZZ/FIZZ.EXE")).unwrap(),
		std::fs::read(dist.join("win16-x86/FIZZ.EXE")).unwrap()
	);
}

#[test]
fn the_win16_receipt_matches_its_golden() {
	let b = tempfile::tempdir().unwrap();
	install_win16(b.path());
	let got = receipt(b.path());
	let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/win16-receipt.txt");
	if std::env::var_os("LI_UPDATE_GOLDEN").is_some() {
		std::fs::write(&golden, &got).unwrap();
	}
	assert_eq!(got, std::fs::read_to_string(&golden).unwrap());
}

#[test]
fn win16_uninstall_removes_what_install_made() {
	let b = tempfile::tempdir().unwrap();
	install_win16(b.path());
	let c = compile("win16-x86");
	let mut p = win16(&c, b.path());
	assert_eq!(app::run(&mut p, "/U"), 0);
	assert_eq!(p.tree(), Vec::<String>::new());
	assert_eq!(p.steps.last().map(String::as_str), Some("done"));
}

#[test]
fn win16_cancelling_the_copy_removes_everything() {
	let b = tempfile::tempdir().unwrap();
	let c = compile("win16-x86");
	let mut p = win16(&c, b.path());
	p.gui.as_mut().unwrap().cancel_after = Some(1);
	assert_eq!(app::run(&mut p, ""), 3);
	assert_eq!(p.tree(), Vec::<String>::new());
}

#[test]
fn the_hand_written_decoder_agrees_with_serde() {
	for target in ["dos-x86", "win16-x86"] {
		let c = compile(target);
		let bytes = c.project.encode().unwrap();
		let by_serde: li_retro::model::Project = postcard::from_bytes(&bytes).unwrap();
		let by_hand = li_retro::model::Project::decode(&bytes).unwrap();
		assert_eq!(by_hand, by_serde, "{target}");
		assert_eq!(by_hand, c.project, "{target}");
		// A cut or padded project is refused, not half read.
		assert!(li_retro::model::Project::decode(&bytes[..bytes.len() - 1]).is_err());
		let mut more = bytes.clone();
		more.push(0);
		assert!(li_retro::model::Project::decode(&more).is_err());
	}
}

#[test]
fn planboard_compiles_for_windows_3_1_with_8_3_names() {
	let project = Project::load(&fizz_dir().join("../planboard")).expect("load planboard");
	let c = li_retro_build::compile_win16(&project, "win16-x86", "1.0.0").expect("compile");
	assert!(c.project.files.iter().any(|f| f.dest == "PLANBRD.EXE"));
	assert!(c.project.files.iter().any(|f| f.dest == "DOCS\\README.TXT"));
}

#[test]
fn a_program_manager_shell_gets_group_items_and_a_receipt_line() {
	let tmp = tempfile::tempdir().unwrap();
	let c = compile("win16-x86");
	let mut p = win16(&c, tmp.path());
	p.program_manager = true;
	assert_eq!(app::run(&mut p, ""), 0);
	assert_eq!(
		p.groups,
		[(
			"Lieutenant Fizz \u{2014} Episode One".to_string(),
			"Lieutenant Fizz".to_string(),
			"C:\\LTFIZZ\\FIZZ.EXE".to_string()
		)]
	);
	let lir = receipt(tmp.path());
	assert!(
		lir.contains("\r\nG Lieutenant Fizz \u{2014} Episode One|Lieutenant Fizz\r\n"),
		"{lir}"
	);
	// A group item replaces the batch launcher.
	assert!(!tmp.path().join("LTFIZZ/FIZZ.BAT").exists());
	let mut p = win16(&c, tmp.path());
	p.program_manager = true;
	p.groups = vec![(
		"Lieutenant Fizz \u{2014} Episode One".to_string(),
		"Lieutenant Fizz".to_string(),
		String::new(),
	)];
	assert_eq!(app::run(&mut p, "/U"), 0);
	assert!(p.groups.is_empty());
	assert_eq!(p.tree(), Vec::<String>::new());
}
