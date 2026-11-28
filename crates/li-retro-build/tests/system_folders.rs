// Installing a file into the Windows folder: Lt. Fizz's optional screen saver on both Windows runtimes, and what the compiler refuses.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The `screensaver` component installs `FIZZ.SCR` into the Windows folder. The host simulator
//! plays the wizard and has a Windows folder (`C:\WINDOWS`), so a test can leave the box unticked
//! (nothing is written outside the install folder), tick it over a file of the same name that was
//! already there (a backup, an `O` record, the file put back on uninstall), and look at the
//! compiler's refusals.

use std::path::{Path, PathBuf};

use li_engine::Project;
use li_retro::app;
use li_retro::install::RECEIPT;
use li_retro::model::{self, WINDOWS_TOKEN};
use li_retro::sim::{GuiOp, HostPlatform, ScriptGui};
use li_retro_build::{Flavour, rooted_rel};

const SEEDED: &[u8] = b"someone else's screen saver\r\n";
const WINDOWS_FILE: &str = "C:\\WINDOWS\\FIZZ.SCR";

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

#[derive(Clone, Copy, Debug)]
enum Runtime {
	Win16,
	Win32,
}

fn compile(runtime: Runtime) -> li_retro_build::Compiled {
	match runtime {
		Runtime::Win16 => li_retro_build::compile_win16(&fizz(), "win16-x86", "1.0.0"),
		Runtime::Win32 => li_retro_build::compile_win32(&fizz(), "windows-x86", "1.0.0"),
	}
	.expect("compile")
}

fn machine(
	runtime: Runtime,
	c: &li_retro_build::Compiled,
	root: &Path,
	ticks: &[&str],
) -> HostPlatform {
	let mut p = HostPlatform::new(root, c.lpk().unwrap());
	p.with_windows();
	match runtime {
		Runtime::Win16 => p.program_manager = true,
		Runtime::Win32 => {
			std::fs::create_dir_all(root.join("Program Files")).unwrap();
			p.native = true;
		}
	}
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

fn no_backup(p: &HostPlatform) -> bool {
	!p.tree().iter().any(|t| t.contains("BACKUP"))
}

#[test]
fn the_compiler_stores_the_saver_under_the_windows_token() {
	for runtime in [Runtime::Win16, Runtime::Win32] {
		let c = compile(runtime);
		assert_eq!(c.project.format, model::FORMAT_BASE);
		let file = c
			.project
			.files
			.iter()
			.find(|f| f.dest.starts_with(WINDOWS_TOKEN))
			.unwrap_or_else(|| panic!("{runtime:?}: a rooted file"));
		assert_eq!(file.dest, "%WINDIR%\\FIZZ.SCR");
		assert_eq!(
			c.project.components[file.component as usize].id,
			"screensaver"
		);
		assert!(!c.project.components[file.component as usize].default);
		c.project.validate().expect("valid format 5");
	}
}

#[test]
fn dos_does_not_get_the_component() {
	let c = li_retro_build::compile(&fizz(), "dos-x86", "1.0.0").expect("compile");
	assert!(c.project.components.iter().all(|c| c.id != "screensaver"));
	assert!(c.project.files.iter().all(|f| !f.dest.starts_with('%')));
}

#[test]
fn an_unticked_saver_writes_nothing_outside_the_install_folder() {
	for runtime in [Runtime::Win16, Runtime::Win32] {
		let tmp = tempfile::tempdir().unwrap();
		let c = compile(runtime);
		let mut p = machine(runtime, &c, tmp.path(), &[]);
		assert_eq!(app::run(&mut p, ""), 0, "{runtime:?}");
		assert!(!p.host(WINDOWS_FILE).exists(), "{runtime:?}");
		assert!(no_backup(&p));
		let page = p.gui.as_ref().unwrap().described.concat();
		assert!(page.contains("[ ] Screen saver"), "{runtime:?}: {page}");
		let lir = receipt_text(&p);
		assert!(!lir.contains("FIZZ.SCR"), "{runtime:?}: {lir}");
	}
}

#[test]
fn a_ticked_saver_over_another_file_is_backed_up_and_put_back() {
	for runtime in [Runtime::Win16, Runtime::Win32] {
		let tmp = tempfile::tempdir().unwrap();
		let c = compile(runtime);
		let mut p = machine(runtime, &c, tmp.path(), &["screensaver"]);
		std::fs::write(p.host(WINDOWS_FILE), SEEDED).unwrap();
		assert_eq!(app::run(&mut p, ""), 0, "{runtime:?}");
		let installed = std::fs::read(p.host(WINDOWS_FILE)).unwrap();
		assert_eq!(
			installed, b"Lt. Fizz screen saver (stand-in)\r\n",
			"{runtime:?}"
		);
		let lir = receipt_text(&p);
		assert!(
			lir.contains(&format!("\r\nF {WINDOWS_FILE} ")),
			"{runtime:?}: {lir}"
		);
		let backup = lir
			.lines()
			.find_map(|l| l.strip_prefix(&format!("O {WINDOWS_FILE}|")))
			.unwrap_or_else(|| panic!("{runtime:?}: an O record in {lir}"));
		assert_eq!(backup, "BACKUP\\WIN\\FIZZ.SCR");
		assert!(
			p.notices
				.iter()
				.any(|n| n == &format!("backup: {WINDOWS_FILE} -> BACKUP\\WIN\\FIZZ.SCR")),
			"{runtime:?}: {:?}",
			p.notices
		);
		let kept = p
			.tree()
			.into_iter()
			.find(|t| t.ends_with("BACKUP\\WIN\\FIZZ.SCR"))
			.expect("the backup");
		assert_eq!(std::fs::read(p.host(&kept)).unwrap(), SEEDED);

		// Uninstall on the same machine: the other file is back and no BACKUP is left.
		let mut u = machine(runtime, &c, tmp.path(), &[]);
		u.associations = p.associations.clone();
		u.prog_ids = p.prog_ids.clone();
		u.registry = p.registry.clone();
		u.groups = p.groups.clone();
		u.group_names = p.group_names.clone();
		assert_eq!(app::run(&mut u, "/U"), 0, "{runtime:?}");
		assert_eq!(
			std::fs::read(u.host(WINDOWS_FILE)).unwrap(),
			SEEDED,
			"{runtime:?}"
		);
		assert!(no_backup(&u), "{runtime:?}: {:?}", u.tree());
	}
}

#[test]
fn a_ticked_saver_on_a_clean_machine_is_removed_by_the_uninstall() {
	for runtime in [Runtime::Win16, Runtime::Win32] {
		let tmp = tempfile::tempdir().unwrap();
		let c = compile(runtime);
		let mut p = machine(runtime, &c, tmp.path(), &["screensaver"]);
		assert_eq!(app::run(&mut p, ""), 0, "{runtime:?}");
		assert!(p.host(WINDOWS_FILE).is_file(), "{runtime:?}");
		assert!(no_backup(&p));
		assert!(!receipt_text(&p).contains("\r\nO "));
		let mut u = machine(runtime, &c, tmp.path(), &[]);
		u.associations = p.associations.clone();
		u.prog_ids = p.prog_ids.clone();
		u.registry = p.registry.clone();
		u.groups = p.groups.clone();
		u.group_names = p.group_names.clone();
		assert_eq!(app::run(&mut u, "/U"), 0, "{runtime:?}");
		assert!(!u.host(WINDOWS_FILE).exists(), "{runtime:?}");
	}
}

#[test]
fn the_compiler_refuses_what_the_runtimes_cannot_place() {
	// DOS has no Windows folder.
	let dos = rooted_rel("{windows}/FIZZ.SCR", Flavour::Dos).unwrap_err();
	assert!(dos.to_string().contains("no meaning on DOS"), "{dos}");
	let dos = rooted_rel("{system}/X.DRV", Flavour::Dos).unwrap_err();
	assert!(dos.to_string().contains("no meaning on DOS"), "{dos}");
	// A folder below the Windows folder.
	for flavour in [Flavour::Win16, Flavour::Win32] {
		let sub = rooted_rel("{windows}/SUB/FIZZ.SCR", flavour).unwrap_err();
		assert!(sub.to_string().contains("directly in"), "{sub}");
		let bare = rooted_rel("{system}/", flavour).unwrap_err();
		assert!(bare.to_string().contains("directly in"), "{bare}");
	}
	// The names follow the flavour's rule.
	assert_eq!(
		rooted_rel("{windows}/fizz.scr", Flavour::Win16).unwrap(),
		Some("%WINDIR%\\FIZZ.SCR".to_string())
	);
	assert!(rooted_rel("{system}/LONGERNAME.DRV", Flavour::Win16).is_err());
	assert_eq!(
		rooted_rel("{system}/Long Name.drv", Flavour::Win32).unwrap(),
		Some("%SYSDIR%\\Long Name.drv".to_string())
	);
	// Anything else is not for this function.
	assert_eq!(rooted_rel("{install}/X", Flavour::Win16).unwrap(), None);
	// An install-folder path may not start with a token.
	assert!(li_retro_build::dos_rel("{install}/%WINDIR%/X.SCR").is_err());
	assert!(li_retro_build::win_rel("{install}/%sysdir%/X.SCR").is_err());
}

#[test]
fn a_project_with_a_rooted_file_is_refused_on_dos_and_below_a_subfolder() {
	let tmp = tempfile::tempdir().unwrap();
	let dir = tmp.path().join("p");
	let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lt-fizz");
	copy_tree(&src, &dir);
	let components = |to: &str, targets: &str| {
		format!(
			"[[component]]\nid = \"game\"\nname = \"Game\"\nrequired = true\ntargets = [{targets}]\nfiles = [{{ from = \"dist/screensaver/FIZZ.SCR\", to = \"{to}\" }}]\n"
		)
	};
	let write = |to: &str, targets: &str| {
		std::fs::write(
			dir.join("installer/components.toml"),
			components(to, targets),
		)
		.unwrap();
		Project::load(&dir).unwrap()
	};
	let dos_project = write("{windows}/", "\"dos-x86\"");
	let dos = li_retro_build::compile(&dos_project, "dos-x86", "1.0.0")
		.err()
		.expect("DOS refuses");
	assert!(dos.to_string().contains("no meaning on DOS"), "{dos}");
	let sub_project = write("{windows}/SUB/", "\"win16-x86\"");
	let sub = li_retro_build::compile_win16(&sub_project, "win16-x86", "1.0.0")
		.err()
		.expect("a subfolder is refused");
	assert!(sub.to_string().contains("directly in"), "{sub}");
}

fn copy_tree(from: &Path, to: &Path) {
	std::fs::create_dir_all(to).unwrap();
	for entry in std::fs::read_dir(from).unwrap() {
		let entry = entry.unwrap();
		let target = to.join(entry.file_name());
		if entry.file_type().unwrap().is_dir() {
			copy_tree(&entry.path(), &target);
		} else {
			std::fs::copy(entry.path(), target).unwrap();
		}
	}
}
