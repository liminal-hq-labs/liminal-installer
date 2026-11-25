// Compiles Lt. Fizz and Planboard for Windows and runs the native flow on the host simulator.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The Win32 runtime shares its flow VM, install engine and receipt with the DOS one; these tests
//! run that code with a scripted native window in place of the keyboard and compare what it does
//! (the pages in order, the files, the receipt) with the DOS run of the same project.

use std::path::{Path, PathBuf};

use li_engine::Project;
use li_retro::Key;
use li_retro::app;
use li_retro::install::{RECEIPT, Receipt};
use li_retro::sim::{GuiOp, HostPlatform, ScriptGui};

fn example(name: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../../examples")
		.join(name)
}

fn compile(name: &str, target: &str) -> li_retro_build::Compiled {
	let project = Project::load(&example(name)).expect("load");
	if target.starts_with("windows") {
		li_retro_build::compile_win32(&project, target, "1.0.0").expect("compile")
	} else {
		li_retro_build::compile(&project, target, "1.0.0").expect("compile")
	}
}

fn accept_licence(g: &mut ScriptGui) {
	g.on(
		"licence",
		vec![GuiOp::Choose("licence.accepted".into(), "yes".into())],
	);
}

fn win(c: &li_retro_build::Compiled, root: &Path, gui: ScriptGui) -> HostPlatform {
	std::fs::create_dir_all(root.join("Program Files")).unwrap();
	let mut p = HostPlatform::new(root, c.lpk().unwrap());
	p.native = true;
	p.gui = Some(gui);
	p
}

/// The DOS run of the same project, with the keys that accept every default.
fn dos_steps() -> Vec<String> {
	let c = compile("lt-fizz", "dos-x86");
	let tmp = tempfile::tempdir().unwrap();
	let mut p = HostPlatform::new(tmp.path(), c.lpk().unwrap());
	p.keys(&[
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
	]);
	assert_eq!(app::run(&mut p, ""), 0);
	p.steps
}

#[test]
fn compiles_with_long_names_and_a_title_per_page() {
	let c = compile("lt-fizz", "windows-x86");
	assert_eq!(c.project.target, "windows-x86:win32-classic");
	assert_eq!(
		c.project.default_dir,
		"%PROGRAMFILES%\\Lieutenant Fizz — Episode One"
	);
	assert!(c.project.files.iter().any(|f| f.dest == "LEVELS\\EP1.LVL"));
	// The sound drivers are for DOS and Windows 3.1 only.
	assert!(!c.project.files.iter().any(|f| f.dest.starts_with("SOUND")));
	let welcome = c
		.project
		.screens
		.iter()
		.find(|s| s.id == "welcome")
		.unwrap();
	assert_eq!(welcome.title, "Welcome");
	let lpk = c.lpk().unwrap();
	assert!(lpk.len() > 100);
}

#[test]
fn walks_the_pages_the_dos_runtime_walks() {
	let c = compile("lt-fizz", "windows-x86");
	let tmp = tempfile::tempdir().unwrap();
	let mut g = ScriptGui::new();
	accept_licence(&mut g);
	let mut p = win(&c, tmp.path(), g);
	assert_eq!(app::run(&mut p, ""), 0);
	// The sound card page is for DOS and Windows 3.1 (`when` on the renderer), so only it differs.
	let dos: Vec<String> = dos_steps()
		.into_iter()
		.filter(|s| s != "sound-card")
		.collect();
	assert_eq!(p.steps, dos, "the same steps, in the same order");
	let gui = p.gui.as_ref().unwrap();
	let titles: Vec<&str> = gui.pages.iter().map(|(_, t, _)| t.as_str()).collect();
	assert_eq!(titles[0], "Welcome");
	// The licence page's Next button is disabled until the licence is accepted.
	let licence = gui.buttons.iter().find(|(s, _)| s == "licence").unwrap();
	assert!(
		licence.1.iter().any(|(l, on)| l.contains("Next") && !on),
		"{licence:?}"
	);
	assert!(gui.pages.iter().any(|(s, _, _)| s == "copying"));
	assert!(gui.files_seen.iter().any(|f| f == "FIZZ.EXE"));
	assert_eq!(gui.last_total, 100);
}

#[test]
fn installs_with_long_names_shortcuts_and_an_uninstall_entry() {
	let c = compile("lt-fizz", "windows-x86");
	let tmp = tempfile::tempdir().unwrap();
	let mut g = ScriptGui::new();
	accept_licence(&mut g);
	let mut p = win(&c, tmp.path(), g);
	assert_eq!(app::run(&mut p, ""), 0);
	let dir = "Program Files/Lieutenant Fizz — Episode One";
	let tree = p.tree();
	for want in [
		format!("{dir}\\FIZZ.EXE").replace("/", "\\"),
		format!("{dir}\\LEVELS\\EP1.LVL").replace("/", "\\"),
		format!("{dir}\\INSTALL.LIR").replace("/", "\\"),
		format!("{dir}\\UNINST.EXE").replace("/", "\\"),
		format!("{dir}\\UNINST.DAT").replace("/", "\\"),
		"WINDOWS\\Start Menu\\Programs\\Lieutenant Fizz — Episode One\\Lieutenant Fizz.lnk"
			.to_string(),
	] {
		assert!(tree.contains(&want), "{want} missing from {tree:#?}");
	}
	// No batch launchers and no sound INI on Windows.
	assert!(
		!tree
			.iter()
			.any(|t| t.ends_with(".BAT") || t.ends_with("SETUP.INI"))
	);
	let lnk = std::fs::read_to_string(p.host(
		"C:\\WINDOWS\\Start Menu\\Programs\\Lieutenant Fizz — Episode One\\Lieutenant Fizz.lnk",
	))
	.unwrap();
	assert!(
		lnk.contains("target=C:\\Program Files\\Lieutenant Fizz — Episode One\\FIZZ.EXE"),
		"{lnk}"
	);
	// The registry entry has an UninstallString.
	let entry = &p.registry["hq.liminal.lt-fizz"];
	let get = |k: &str| entry.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
	assert_eq!(get("DisplayName"), Some("Lieutenant Fizz — Episode One"));
	assert_eq!(get("Publisher"), Some("Liminal HQ"));
	assert!(
		get("UninstallString")
			.unwrap()
			.contains("UNINST.EXE\" /U \"C:\\Program Files\\Lieutenant Fizz")
	);
	// The receipt is the DOS format with the Windows records added.
	let lir = std::fs::read_to_string(p.host(&format!(
		"C:\\Program Files\\Lieutenant Fizz — Episode One\\{RECEIPT}"
	)))
	.unwrap();
	assert!(lir.starts_with("LIR 1\r\nproduct=hq.liminal.lt-fizz\r\nversion=1.0.0\r\ntarget=windows-x86:win32-classic\r\ndir=C:\\Program Files\\Lieutenant Fizz — Episode One\r\ncomponents=game,levels\r\n"), "{lir}");
	let r = Receipt::parse(&lir).unwrap();
	assert_eq!(
		r.files.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(),
		["FIZZ.EXE", "LEVELS\\EP1.LVL", "LEVELS\\EP1.PAL"]
	);
	// The data files carry the same size and CRC-32 the DOS receipt records for them.
	let dos = compile("lt-fizz", "dos-x86");
	for name in ["LEVELS\\EP1.LVL", "LEVELS\\EP1.PAL"] {
		let a = dos.project.files.iter().find(|f| f.dest == name).unwrap();
		let b = r.files.iter().find(|f| f.0 == name).unwrap();
		assert_eq!((a.size, a.crc), (b.1, b.2), "{name}");
	}
	assert_eq!(r.keys, ["hq.liminal.lt-fizz"]);
	assert_eq!(r.shortcuts.len(), 1);
	assert_eq!(r.extras, ["UNINST.EXE", "UNINST.DAT"]);
	assert_eq!(r.dirs[0], ".");
}

#[test]
fn uninstalls_from_the_kept_project() {
	let c = compile("lt-fizz", "windows-x86");
	let tmp = tempfile::tempdir().unwrap();
	let mut g = ScriptGui::new();
	accept_licence(&mut g);
	let mut p = win(&c, tmp.path(), g);
	assert_eq!(app::run(&mut p, ""), 0);
	let dir = "C:\\Program Files\\Lieutenant Fizz — Episode One";
	std::fs::write(p.host(&format!("{dir}\\SAVE.DAT")), b"mine").unwrap();
	// A fresh run, as the uninstaller starts: no SETUP.LPK, the project kept in the folder.
	let kept = std::fs::read(p.host(&format!("{dir}\\UNINST.DAT"))).unwrap();
	let mut u = HostPlatform::new(tmp.path(), Vec::new());
	u.native = true;
	u.gui = Some(ScriptGui::new());
	u.kept_project = Some(kept);
	u.registry = p.registry.clone();
	let rc = app::run(&mut u, &format!("/U \"{dir}\""));
	assert_eq!(rc, 0, "{:?}", u.gui.as_ref().unwrap().messages);
	assert!(u.registry.is_empty(), "the Add/Remove entry is gone");
	let tree = u.tree();
	// The person's own file stays; everything the install made is gone, shortcut folder included.
	assert!(tree.iter().any(|t| t.ends_with("SAVE.DAT")), "{tree:#?}");
	assert!(
		!tree.iter().any(|t| t.ends_with(".lnk")
			|| t.ends_with("FIZZ.EXE")
			|| t.ends_with("UNINST.EXE")
			|| t.ends_with("INSTALL.LIR")),
		"{tree:#?}"
	);
	assert!(
		!tree
			.iter()
			.any(|t| t.contains("Lieutenant Fizz — Episode One\\") && t.contains("Start Menu")),
		"{tree:#?}"
	);
	assert_eq!(u.steps, ["confirm", "done"]);
}

/// Installs, then uninstalls from the kept project. Returns whether the install folder and the
/// person's own file are still there.
fn uninstall_leaves(precreate: bool, own_file: bool) -> (bool, bool) {
	let c = compile("lt-fizz", "windows-x86");
	let tmp = tempfile::tempdir().unwrap();
	let dir = "C:\\Program Files\\Lieutenant Fizz — Episode One";
	let mut g = ScriptGui::new();
	accept_licence(&mut g);
	let mut p = win(&c, tmp.path(), g);
	if precreate {
		std::fs::create_dir_all(p.host(dir)).unwrap();
	}
	assert_eq!(app::run(&mut p, ""), 0);
	if own_file {
		std::fs::write(p.host(&format!("{dir}\\SAVE.DAT")), b"mine").unwrap();
	}
	let kept = std::fs::read(p.host(&format!("{dir}\\UNINST.DAT"))).unwrap();
	let mut u = HostPlatform::new(tmp.path(), Vec::new());
	u.native = true;
	u.gui = Some(ScriptGui::new());
	u.kept_project = Some(kept);
	u.registry = p.registry.clone();
	assert_eq!(app::run(&mut u, &format!("/U \"{dir}\"")), 0);
	(
		p.host(dir).exists(),
		p.host(&format!("{dir}\\SAVE.DAT")).exists(),
	)
}

#[test]
fn an_uninstall_removes_the_install_folder_it_left_empty() {
	// A folder that was there before the install is not in the receipt, and is still removed
	// once the uninstall has emptied it.
	assert_eq!(uninstall_leaves(true, false), (false, false));
	assert_eq!(uninstall_leaves(false, false), (false, false));
}

#[test]
fn an_uninstall_never_removes_a_folder_that_holds_the_persons_files() {
	assert_eq!(uninstall_leaves(true, true), (true, true));
	assert_eq!(uninstall_leaves(false, true), (true, true));
}

#[test]
fn a_deselected_component_is_not_installed() {
	let c = compile("lt-fizz", "windows-x86");
	let tmp = tempfile::tempdir().unwrap();
	let mut g = ScriptGui::new();
	accept_licence(&mut g);
	g.on("components", vec![GuiOp::Toggle("levels".into())]);
	let mut p = win(&c, tmp.path(), g);
	assert_eq!(app::run(&mut p, ""), 0);
	assert!(
		!p.tree().iter().any(|t| t.contains("LEVELS")),
		"{:#?}",
		p.tree()
	);
}

#[test]
fn a_typed_folder_is_used() {
	let c = compile("lt-fizz", "windows-x86");
	let tmp = tempfile::tempdir().unwrap();
	let mut g = ScriptGui::new();
	accept_licence(&mut g);
	g.on(
		"folder",
		vec![GuiOp::Set(
			"install.path".into(),
			"D:\\Games\\Fizz One".into(),
		)],
	);
	let mut p = win(&c, tmp.path(), g);
	assert_eq!(app::run(&mut p, ""), 0);
	assert!(
		p.tree().contains(&"Games\\Fizz One\\FIZZ.EXE".to_string()),
		"{:#?}",
		p.tree()
	);
	let lir = std::fs::read_to_string(p.host("D:\\Games\\Fizz One\\INSTALL.LIR")).unwrap();
	assert!(lir.contains("dir=D:\\Games\\Fizz One\r\n"));
}

#[test]
fn cancelling_while_copying_removes_everything() {
	let c = compile("lt-fizz", "windows-x86");
	let tmp = tempfile::tempdir().unwrap();
	let mut g = ScriptGui::new();
	accept_licence(&mut g);
	g.cancel_after = Some(2);
	let mut p = win(&c, tmp.path(), g);
	assert_eq!(app::run(&mut p, ""), 3);
	let tree = p.tree();
	assert!(
		!tree
			.iter()
			.any(|t| t.contains("FIZZ.EXE") || t.ends_with(".lnk")),
		"{tree:#?}"
	);
	assert!(p.registry.is_empty());
}

#[test]
fn planboard_installs_with_long_names_below_a_bin_folder() {
	let c = compile("planboard", "windows-x86");
	let tmp = tempfile::tempdir().unwrap();
	let mut g = ScriptGui::new();
	accept_licence(&mut g);
	let mut p = win(&c, tmp.path(), g);
	assert_eq!(
		app::run(&mut p, ""),
		0,
		"{:?}",
		p.gui.as_ref().unwrap().messages
	);
	let tree = p.tree();
	for want in [
		"planboard.exe",
		"bin\\pbcli.exe",
		"docs\\GETTING-STARTED.md",
	] {
		assert!(
			tree.iter().any(|t| t.ends_with(want)),
			"{want} missing from {tree:#?}"
		);
	}
	assert!(p.registry.contains_key("hq.liminal.planboard"));
}

#[test]
fn the_dos_compile_refuses_a_win32_target_and_back() {
	let project = Project::load(&example("lt-fizz")).unwrap();
	assert!(li_retro_build::compile(&project, "windows-x86", "1").is_err());
	assert!(li_retro_build::compile_win32(&project, "dos-x86", "1").is_err());
}

/// Compares `text` with `tests/golden/win32/<name>.txt`; `LI_UPDATE_GOLDEN=1` rewrites it. The Wine
/// run (`win32/tools/wine-e2e.sh`) compares the pages its controls show with the same files.
fn golden(name: &str, text: &str) {
	let path = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("tests/golden/win32")
		.join(format!("{name}.txt"));
	if std::env::var_os("LI_UPDATE_GOLDEN").is_some() {
		std::fs::create_dir_all(path.parent().unwrap()).unwrap();
		std::fs::write(&path, text).unwrap();
	}
	let want = std::fs::read_to_string(&path)
		.unwrap_or_else(|_| panic!("{} is missing; run with LI_UPDATE_GOLDEN=1", path.display()));
	assert_eq!(text, want, "{name} differs from {}", path.display());
}

fn pages_of(name: &str) -> String {
	let c = compile(name, "windows-x86");
	let tmp = tempfile::tempdir().unwrap();
	let mut g = ScriptGui::new();
	accept_licence(&mut g);
	let mut p = win(&c, tmp.path(), g);
	assert_eq!(app::run(&mut p, ""), 0);
	let gui = p.gui.as_ref().unwrap();
	gui.described.concat()
}

#[test]
fn the_pages_match_their_goldens() {
	golden("lt-fizz.pages", &pages_of("lt-fizz"));
	golden("planboard.pages", &pages_of("planboard"));
}

#[test]
fn the_uninstall_pages_match_their_golden() {
	let c = compile("lt-fizz", "windows-x86");
	let tmp = tempfile::tempdir().unwrap();
	let mut g = ScriptGui::new();
	accept_licence(&mut g);
	let mut p = win(&c, tmp.path(), g);
	assert_eq!(app::run(&mut p, ""), 0);
	let dir = "C:\\Program Files\\Lieutenant Fizz — Episode One";
	let mut u = HostPlatform::new(tmp.path(), Vec::new());
	u.native = true;
	u.gui = Some(ScriptGui::new());
	u.kept_project = Some(std::fs::read(p.host(&format!("{dir}\\UNINST.DAT"))).unwrap());
	assert_eq!(app::run(&mut u, &format!("/U \"{dir}\"")), 0);
	golden(
		"lt-fizz.uninstall-pages",
		&u.gui.as_ref().unwrap().described.concat(),
	);
}

#[test]
fn the_step_caption_counts_up_and_the_last_page_is_the_last_step() {
	let c = compile("lt-fizz", "windows-x86");
	let tmp = tempfile::tempdir().unwrap();
	let mut g = ScriptGui::new();
	accept_licence(&mut g);
	let mut p = win(&c, tmp.path(), g);
	assert_eq!(app::run(&mut p, ""), 0);
	let n = &p.gui.as_ref().unwrap().numbers;
	let numbered: Vec<_> = n.iter().filter(|(_, _, t)| *t > 0).collect();
	let total = numbered[0].2;
	assert!(numbered.iter().all(|(_, _, t)| *t == total));
	let done = numbered.iter().find(|(s, _, _)| s == "done").unwrap();
	assert_eq!(done.1, total, "{numbered:?}");
	let welcome = numbered.iter().find(|(s, _, _)| s == "welcome").unwrap();
	assert_eq!(welcome.1, 1);
}
