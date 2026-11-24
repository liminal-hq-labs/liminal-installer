// Optional components: an unticked component is not installed, a ticked one is, the receipt says which, and the uninstall takes back only those.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lt. Fizz has two components that start unticked: `bonus` (files) and `savegames` (a file type with
//! no files). The host simulator plays the wizard's pages, so a test can leave the boxes as they are,
//! tick them, or skip the pages with `/S` and `/COMPONENTS=`, and then look at the files, the
//! receipt, the associations and what the uninstall leaves.

use std::path::{Path, PathBuf};

use li_engine::Project;
use li_retro::app;
use li_retro::install::{RECEIPT, Receipt};
use li_retro::model::{self, NO_OWNER, Op};
use li_retro::sim::{GuiOp, HostPlatform, ScriptGui};

fn fizz() -> Project {
	let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lt-fizz");
	Project::load(&dir).expect("load lt-fizz")
}

fn gui(ticks: &[&str]) -> ScriptGui {
	let mut g = ScriptGui::new();
	g.on(
		"licence",
		vec![GuiOp::Choose("licence.accepted".into(), "yes".into())],
	);
	g.on(
		"components",
		ticks.iter().map(|t| GuiOp::Toggle((*t).into())).collect(),
	);
	g
}

fn compile32() -> li_retro_build::Compiled {
	li_retro_build::compile_win32(&fizz(), "windows-x86", "1.0.0").expect("compile")
}

fn compile16() -> li_retro_build::Compiled {
	li_retro_build::compile_win16(&fizz(), "win16-x86", "1.0.0").expect("compile")
}

fn win32(c: &li_retro_build::Compiled, root: &Path, ticks: &[&str]) -> HostPlatform {
	std::fs::create_dir_all(root.join("Program Files")).unwrap();
	let mut p = HostPlatform::new(root, c.lpk().unwrap());
	p.native = true;
	p.gui = Some(gui(ticks));
	p
}

fn win16(c: &li_retro_build::Compiled, root: &Path, ticks: &[&str]) -> HostPlatform {
	let mut p = HostPlatform::new(root, c.lpk().unwrap());
	p.program_manager = true;
	p.gui = Some(gui(ticks));
	p
}

fn receipt_text(p: &HostPlatform) -> String {
	let name = p
		.tree()
		.into_iter()
		.find(|t| t.ends_with(RECEIPT))
		.expect("a receipt");
	std::fs::read_to_string(p.host(&name)).unwrap()
}

fn has(p: &HostPlatform, file: &str) -> bool {
	p.tree().iter().any(|t| t.ends_with(file))
}

fn components_line(lir: &str) -> &str {
	lir.lines()
		.find(|l| l.starts_with("components="))
		.expect("a components line")
}

#[test]
fn the_compiler_records_the_starting_state_and_which_component_owns_each_operation() {
	let c = compile32();
	let on: Vec<(&str, bool)> = c
		.project
		.components
		.iter()
		.map(|c| (c.id.as_str(), c.default))
		.collect();
	assert_eq!(
		on,
		[
			("game", true),
			("levels", true),
			("bonus", false),
			("savegames", false)
		]
	);
	// One shortcut (the game's) and one file type (the unticked component's).
	let owner_of = |want: fn(&Op) -> bool| {
		let i = c.project.ops.iter().position(want).unwrap();
		c.project.owners[i]
	};
	let index = |id: &str| {
		c.project
			.components
			.iter()
			.position(|c| c.id == id)
			.unwrap() as u16
	};
	assert_eq!(
		owner_of(|o| matches!(o, Op::Shortcut { .. })),
		index("game")
	);
	assert_eq!(
		owner_of(|o| matches!(o, Op::FileType { .. })),
		index("savegames")
	);
	assert_eq!(c.project.owners.len(), c.project.ops.len());
	assert_eq!(c.project.format, model::FORMAT);
	// The format round-trips through the hand-written decoder.
	let back = model::Project::decode(&c.project.encode().unwrap()).unwrap();
	assert_eq!(back, c.project);
}

#[test]
fn the_dos_sound_settings_belong_to_no_component() {
	let c = li_retro_build::compile(&fizz(), "dos-x86", "1.0.0").expect("compile");
	let sound = c
		.project
		.ops
		.iter()
		.position(|o| matches!(o, Op::WriteIni { .. }))
		.unwrap();
	assert_eq!(c.project.owners[sound], NO_OWNER);
}

#[test]
fn an_unticked_component_is_not_installed_and_the_receipt_says_so() {
	let tmp = tempfile::tempdir().unwrap();
	let c = compile32();
	let mut p = win32(&c, tmp.path(), &[]);
	p.associations.insert("fzs".into(), "textfile".into());
	assert_eq!(app::run(&mut p, ""), 0);
	assert!(has(&p, "FIZZ.EXE") && has(&p, "EP1.LVL"));
	assert!(!has(&p, "BONUS1.LVL"));
	// The page showed the box unticked.
	let page = p.gui.as_ref().unwrap().described.concat();
	assert!(page.contains("[ ] Bonus levels"), "{page}");
	assert!(
		page.contains("[ ] Open saved games (.FZS) with Fizz"),
		"{page}"
	);
	let lir = receipt_text(&p);
	assert_eq!(components_line(&lir), "components=game,levels");
	assert!(!lir.contains("\r\nA "), "{lir}");
	// The extension was not touched.
	assert_eq!(p.associations["fzs"], "textfile");
	assert!(p.prog_ids.is_empty());
}

#[test]
fn a_ticked_component_is_installed_recorded_and_its_file_type_taken() {
	let tmp = tempfile::tempdir().unwrap();
	let c = compile32();
	let mut p = win32(&c, tmp.path(), &["bonus", "savegames"]);
	p.associations.insert("fzs".into(), "textfile".into());
	assert_eq!(app::run(&mut p, ""), 0);
	assert!(has(&p, "BONUS1.LVL"));
	let lir = receipt_text(&p);
	assert_eq!(
		components_line(&lir),
		"components=game,levels,bonus,savegames"
	);
	assert!(
		lir.contains("\r\nF BONUS\\BONUS1.LVL "),
		"the receipt lists the bonus file: {lir}"
	);
	assert!(
		lir.contains("|textfile\r\n") && lir.contains("\r\nA fzs|hq.liminal.lt-fizz.fzs|"),
		"the receipt keeps the previous owner: {lir}"
	);
	assert_eq!(p.associations["fzs"], "hq.liminal.lt-fizz.fzs");
}

#[test]
fn the_uninstall_takes_back_only_what_was_installed() {
	let tmp = tempfile::tempdir().unwrap();
	let c = compile32();

	// Nothing optional: the uninstall leaves an extension that was never ours alone.
	let mut p = win32(&c, tmp.path(), &[]);
	p.associations.insert("fzs".into(), "textfile".into());
	assert_eq!(app::run(&mut p, ""), 0);
	let mut u = win32(&c, tmp.path(), &[]);
	u.associations = p.associations.clone();
	u.prog_ids = p.prog_ids.clone();
	u.registry = p.registry.clone();
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert_eq!(u.associations["fzs"], "textfile");
	assert!(
		!u.tree().iter().any(|t| !t.ends_with('\\')),
		"{:?}",
		u.tree()
	);

	// Both ticked: the files go and the previous owner comes back.
	let tmp = tempfile::tempdir().unwrap();
	let mut p = win32(&c, tmp.path(), &["bonus", "savegames"]);
	p.associations.insert("fzs".into(), "textfile".into());
	assert_eq!(app::run(&mut p, ""), 0);
	let mut u = win32(&c, tmp.path(), &[]);
	u.associations = p.associations.clone();
	u.prog_ids = p.prog_ids.clone();
	u.registry = p.registry.clone();
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert_eq!(u.associations["fzs"], "textfile");
	assert!(u.prog_ids.is_empty());
	assert!(
		!u.tree().iter().any(|t| !t.ends_with('\\')),
		"{:?}",
		u.tree()
	);

	// Only the file type ticked: the bonus files were never there and the type is given back.
	let tmp = tempfile::tempdir().unwrap();
	let mut p = win32(&c, tmp.path(), &["savegames"]);
	p.associations.insert("fzs".into(), "textfile".into());
	assert_eq!(app::run(&mut p, ""), 0);
	assert!(!has(&p, "BONUS1.LVL"));
	let mut u = win32(&c, tmp.path(), &[]);
	u.associations = p.associations.clone();
	u.prog_ids = p.prog_ids.clone();
	u.registry = p.registry.clone();
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert_eq!(u.associations["fzs"], "textfile");
}

#[test]
fn a_failed_install_leaves_neither_the_files_nor_the_association() {
	let tmp = tempfile::tempdir().unwrap();
	let c = compile32();
	let mut ok = win32(&c, tmp.path(), &["bonus", "savegames"]);
	assert_eq!(app::run(&mut ok, ""), 0);
	let lir = receipt_text(&ok);
	let r = Receipt::parse(&lir).unwrap();
	let total = r.files.iter().map(|f| u64::from(f.1)).sum::<u64>() + lir.len() as u64;

	let b = tempfile::tempdir().unwrap();
	let mut p = win32(&c, b.path(), &["bonus", "savegames"]);
	p.associations.insert("fzs".into(), "textfile".into());
	p.fail_after = Some(total - 1);
	assert_ne!(app::run(&mut p, ""), 0);
	assert_eq!(p.associations["fzs"], "textfile");
	assert!(p.prog_ids.is_empty());
	assert!(
		!p.tree().iter().any(|t| !t.ends_with('\\')),
		"no file is left: {:?}",
		p.tree()
	);
}

#[test]
fn windows_3_1_takes_an_unticked_type_only_when_it_is_ticked() {
	let c = compile16();
	let old = "C:\\WINDOWS\\NOTEPAD.EXE ^.fzs";

	let tmp = tempfile::tempdir().unwrap();
	let mut p = win16(&c, tmp.path(), &[]);
	p.associations.insert("fzs".into(), old.into());
	assert_eq!(app::run(&mut p, ""), 0);
	assert_eq!(p.associations["fzs"], old);
	assert!(!receipt_text(&p).contains("\r\nA "));

	let tmp = tempfile::tempdir().unwrap();
	let mut p = win16(&c, tmp.path(), &["savegames"]);
	p.associations.insert("fzs".into(), old.into());
	assert_eq!(app::run(&mut p, ""), 0);
	assert_eq!(p.associations["fzs"], "C:\\LTFIZZ\\FIZZ.EXE ^.fzs");
	let mut u = win16(&c, tmp.path(), &[]);
	u.associations = p.associations.clone();
	u.group_names = p.group_names.clone();
	u.groups = p.groups.clone();
	assert_eq!(app::run(&mut u, "/U"), 0);
	assert_eq!(u.associations["fzs"], old);
}

#[test]
fn a_silent_install_takes_the_defaults_and_components_names_the_rest() {
	let c = compile32();

	// `/S` alone installs what starts ticked, with no page shown.
	let tmp = tempfile::tempdir().unwrap();
	let mut p = win32(&c, tmp.path(), &[]);
	assert_eq!(app::run(&mut p, "/S"), 0);
	assert!(p.gui.as_ref().unwrap().pages.is_empty());
	assert!(has(&p, "FIZZ.EXE") && has(&p, "EP1.LVL") && !has(&p, "BONUS1.LVL"));
	assert_eq!(components_line(&receipt_text(&p)), "components=game,levels");

	// `/COMPONENTS=` is exactly those, besides the required ones.
	let tmp = tempfile::tempdir().unwrap();
	let mut p = win32(&c, tmp.path(), &[]);
	p.associations.insert("fzs".into(), "textfile".into());
	assert_eq!(app::run(&mut p, "/S /COMPONENTS=bonus,savegames"), 0);
	assert!(has(&p, "BONUS1.LVL") && !has(&p, "EP1.LVL"));
	assert_eq!(
		components_line(&receipt_text(&p)),
		"components=game,bonus,savegames"
	);
	assert_eq!(p.associations["fzs"], "hq.liminal.lt-fizz.fzs");

	// The switch also chooses what the pages start with, in either order.
	let tmp = tempfile::tempdir().unwrap();
	let mut p = win32(&c, tmp.path(), &[]);
	assert_eq!(app::run(&mut p, "/components=Bonus"), 0);
	assert!(has(&p, "BONUS1.LVL"));
	let page = p.gui.as_ref().unwrap().described.concat();
	assert!(
		page.contains("[x] Bonus levels") && page.contains("[ ] Episode one levels"),
		"{page}"
	);

	// A name that is not a component is refused before anything is written.
	let tmp = tempfile::tempdir().unwrap();
	let mut p = win32(&c, tmp.path(), &[]);
	assert_eq!(app::run(&mut p, "/S /COMPONENTS=nothing"), 2);
	assert!(p.reports.concat().contains("unknown component nothing"));
	assert!(!p.tree().iter().any(|t| !t.ends_with('\\')));
}

#[test]
fn a_required_component_cannot_start_unticked() {
	let mut project = fizz();
	project.components.components[0].default = false;
	let err = li_retro_build::compile_win32(&project, "windows-x86", "1.0.0")
		.err()
		.expect("refused");
	assert!(err.to_string().contains("cannot start unticked"), "{err}");

	// The decoder refuses the same thing in a pack that somehow carries it.
	let mut c = compile32();
	c.project.components[0].default = false;
	let err = model::Project::decode(&c.project.encode().unwrap()).unwrap_err();
	assert!(format!("{err:?}").contains("wrong"), "{err:?}");
}

#[test]
fn an_operation_owned_by_a_missing_component_is_refused() {
	let mut c = compile32();
	c.project.owners[0] = 99;
	let err = model::Project::decode(&c.project.encode().unwrap()).unwrap_err();
	assert!(format!("{err:?}").contains("wrong"), "{err:?}");
	let mut c = compile32();
	c.project.owners.pop();
	assert!(model::Project::decode(&c.project.encode().unwrap()).is_err());
}
