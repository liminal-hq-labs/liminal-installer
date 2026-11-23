// Compiles Lt. Fizz for DOS and runs the whole install, uninstall and rollback on the host simulator.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use li_engine::Project;
use li_retro::Key;
use li_retro::app;
use li_retro::install::{RECEIPT, Receipt};
use li_retro::sim::HostPlatform;

fn fizz_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lt-fizz")
}

fn compiled() -> li_retro_build::Compiled {
	let project = Project::load(&fizz_dir()).expect("load lt-fizz");
	li_retro_build::compile(&project, "dos-x86", "1.0.0").expect("compile")
}

/// The keys for the whole install with the defaults, accepting the licence.
fn install_keys() -> Vec<Key> {
	vec![
		Key::Enter, // welcome
		// licence: focus starts on the disabled Next; Shift-Tab twice reaches the radio group
		Key::BackTab,
		Key::BackTab,
		Key::Char(b' '),
		Key::Enter,
		Key::Enter, // folder
		Key::Enter, // components
		Key::Enter, // sound card
		Key::Enter, // ready
		Key::Enter, // done
	]
}

#[test]
fn compiles_the_example_and_round_trips() {
	let c = compiled();
	let dat = c.lpk().unwrap();
	let again = li_retro::model::Project::decode(&project_bytes(&dat)).unwrap();
	assert_eq!(again, c.project);
	assert_eq!(c.project.default_dir, "C:\\LTFIZZ");
	assert!(c.project.files.iter().any(|f| f.dest == "SOUND\\ADLIB.DRV"));
	eprintln!(
		"SETUP.LPK is {} bytes; warnings: {:?}",
		dat.len(),
		c.warnings
	);
}

/// The compiled project, which is the first entry of `SETUP.LPK`.
fn project_bytes(lpk: &[u8]) -> Vec<u8> {
	let vols = [lpk];
	let mut src = li_lpk::SliceSource::new(&vols);
	let mut r = li_lpk::Reader::open(&mut src).unwrap();
	let mut dir_bytes = vec![0u8; r.directory_len()];
	let dir = r.read_directory(&mut src, &mut dir_bytes).unwrap();
	let e = dir.entries().next().unwrap().unwrap();
	assert_eq!(e.short_name, li_retro::pack::PROJECT_FILE.as_bytes());
	let mut out = Vec::new();
	let mut window = vec![0u8; 32768];
	r.extract(&mut src, &e, &mut window, &mut |d: &[u8]| {
		out.extend_from_slice(d);
		Ok::<(), li_lpk::SinkError>(())
	})
	.unwrap();
	out
}

fn install(c: &li_retro_build::Compiled, root: &Path) -> (HostPlatform, i32) {
	let mut p = HostPlatform::new(root, c.lpk().unwrap());
	p.keys(&install_keys());
	let rc = app::run(&mut p, "");
	(p, rc)
}

#[test]
fn installs_the_example_into_a_tempdir() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	let (p, rc) = install(&c, tmp.path());
	assert_eq!(rc, 0);
	assert_eq!(
		p.tree(),
		[
			"LTFIZZ\\",
			"LTFIZZ\\FIZZ.BAT",
			"LTFIZZ\\FIZZ.EXE",
			"LTFIZZ\\INSTALL.LIR",
			"LTFIZZ\\LEVELS\\",
			"LTFIZZ\\LEVELS\\EP1.LVL",
			"LTFIZZ\\LEVELS\\EP1.PAL",
			"LTFIZZ\\SETUP.INI",
			"LTFIZZ\\SOUND\\",
			"LTFIZZ\\SOUND\\ADLIB.DRV",
			"LTFIZZ\\SOUND\\SBPRO.DRV",
		]
		.map(|s| s.replace("\\\\", "\\"))
	);
	// Every file is byte for byte what the project holds.
	let dist = fizz_dir().join("installer/dist");
	let game = std::fs::read(tmp.path().join("LTFIZZ/FIZZ.EXE")).unwrap();
	assert_eq!(game, std::fs::read(dist.join("dos-x86/FIZZ.EXE")).unwrap());
	let ep = std::fs::read(tmp.path().join("LTFIZZ/LEVELS/EP1.LVL")).unwrap();
	assert_eq!(ep, std::fs::read(dist.join("levels/EP1.LVL")).unwrap());
	// The receipt.
	let lir = std::fs::read_to_string(tmp.path().join("LTFIZZ").join(RECEIPT)).unwrap();
	assert!(lir.starts_with("LIR 1\r\nproduct=hq.liminal.lt-fizz\r\nversion=1.0.0\r\ntarget=dos-x86:tui-retro\r\ndir=C:\\LTFIZZ\r\ncomponents=game,sound,levels\r\n"), "{lir}");
	let r = Receipt::parse(&lir).unwrap();
	assert_eq!(r.files.len(), 5);
	assert_eq!(r.dirs, [".", "LEVELS", "SOUND"]);
	assert_eq!(r.others, ["SETUP.INI", "FIZZ.BAT"]);
	let bat = std::fs::read_to_string(tmp.path().join("LTFIZZ/FIZZ.BAT")).unwrap();
	assert_eq!(
		bat,
		"@echo off\r\nC:\r\ncd \\LTFIZZ\r\nFIZZ.EXE %1 %2 %3 %4\r\n"
	);
	let ini = std::fs::read_to_string(tmp.path().join("LTFIZZ/SETUP.INI")).unwrap();
	assert_eq!(
		ini,
		"[sound]\r\ncard=none\r\nport=220\r\nirq=5\r\ndma=1\r\n"
	);
}

fn run_keys(
	c: &li_retro_build::Compiled,
	root: &Path,
	keys: &[Key],
	args: &str,
) -> (HostPlatform, i32) {
	let mut p = HostPlatform::new(root, c.lpk().unwrap());
	p.keys(keys);
	let rc = app::run(&mut p, args);
	(p, rc)
}

fn up_to_ready() -> Vec<Key> {
	let mut k = install_keys();
	k.truncate(8); // through the sound page; the next Enter is the Ready page
	k
}

#[test]
fn uninstall_removes_what_install_made() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	let (_, rc) = install(&c, tmp.path());
	assert_eq!(rc, 0);
	// The uninstall flow: confirm, copying, done.
	let (p, rc) = run_keys(&c, tmp.path(), &[Key::Enter, Key::Enter], "/U");
	assert_eq!(rc, 0, "{:?}", p.frames.last());
	assert_eq!(p.tree(), Vec::<String>::new());
	assert!(
		p.frames.iter().any(|f| f.contains("Setup complete")
			|| f.contains("removed")
			|| f.contains("Uninstall"))
	);
}

#[test]
fn uninstall_leaves_files_it_did_not_make() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	install(&c, tmp.path());
	std::fs::write(tmp.path().join("LTFIZZ/SAVE.DAT"), b"mine").unwrap();
	let (_, rc) = run_keys(&c, tmp.path(), &[Key::Enter, Key::Enter], "/U");
	assert_eq!(rc, 0);
	assert!(tmp.path().join("LTFIZZ/SAVE.DAT").is_file());
	assert!(!tmp.path().join("LTFIZZ/FIZZ.EXE").exists());
	assert!(!tmp.path().join("LTFIZZ").join(RECEIPT).exists());
}

#[test]
fn cancelling_while_copying_removes_everything() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	let mut keys = up_to_ready();
	keys.push(Key::Enter); // Install
	keys.push(Key::Esc); // cancel as soon as the first progress report comes
	let (p, rc) = run_keys(&c, tmp.path(), &keys, "");
	assert_eq!(rc, 3);
	assert_eq!(p.tree(), Vec::<String>::new());
}

#[test]
fn a_write_failure_rolls_back() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	let mut keys = up_to_ready();
	keys.extend([Key::Enter, Key::Enter]);
	let mut p = HostPlatform::new(tmp.path(), c.lpk().unwrap());
	p.fail_after = Some(300);
	p.keys(&keys);
	let rc = app::run(&mut p, "");
	assert_eq!(rc, 1);
	assert_eq!(p.tree(), Vec::<String>::new());
}

#[test]
fn a_full_drive_is_refused_before_anything_is_written() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	let mut keys = up_to_ready();
	keys.extend([Key::Enter, Key::Enter, Key::F(3)]); // Install, dismiss the message, quit
	let mut p = HostPlatform::new(tmp.path(), c.lpk().unwrap());
	p.capacity = 2000;
	p.keys(&keys);
	let rc = app::run(&mut p, "");
	assert_eq!(rc, 3);
	assert!(
		p.frames
			.iter()
			.any(|f| f.contains("Not enough disk space on C:"))
	);
	assert_eq!(p.tree(), Vec::<String>::new());
}

#[test]
fn deselecting_a_component_leaves_its_files_out() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	let mut keys = vec![
		Key::Enter,
		Key::BackTab,
		Key::BackTab,
		Key::Char(b' '),
		Key::Enter,
		Key::Enter,
	];
	// Components: the list is first; move to it, down to Sound drivers, untick, accept.
	keys.extend([
		Key::BackTab,
		Key::BackTab,
		Key::Down,
		Key::Char(b' '),
		Key::Enter,
	]);
	keys.extend([Key::Enter, Key::Enter, Key::Enter, Key::Enter]); // sound, ready, (copying), done
	let (p, rc) = run_keys(&c, tmp.path(), &keys, "");
	assert_eq!(rc, 0, "{:?}", p.frames.last());
	let tree = p.tree();
	assert!(tree.iter().any(|t| t.ends_with("FIZZ.EXE")));
	assert!(!tree.iter().any(|t| t.contains("SOUND\\")), "{tree:?}");
	let r =
		Receipt::parse(&std::fs::read_to_string(tmp.path().join("LTFIZZ").join(RECEIPT)).unwrap())
			.unwrap();
	assert_eq!(r.components, ["game", "levels"]);
}

#[test]
fn a_typed_folder_is_used_and_its_parents_are_recorded() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	let mut keys = vec![
		Key::Enter,
		Key::BackTab,
		Key::BackTab,
		Key::Char(b' '),
		Key::Enter,
	];
	keys.extend([Key::BackTab, Key::BackTab]); // the folder field
	keys.extend(std::iter::repeat_n(Key::Backspace, 9));
	keys.extend("c:\\games\\fiz".bytes().map(Key::Char));
	keys.extend([
		Key::Enter,
		Key::Enter,
		Key::Enter,
		Key::Enter,
		Key::Enter,
		Key::Enter,
	]);
	let (p, rc) = run_keys(&c, tmp.path(), &keys, "");
	assert_eq!(rc, 0, "{:?}", p.frames.last());
	assert!(tmp.path().join("GAMES/FIZ/FIZZ.EXE").is_file());
	let r = Receipt::parse(
		&std::fs::read_to_string(tmp.path().join("GAMES/FIZ").join(RECEIPT)).unwrap(),
	)
	.unwrap();
	assert_eq!(r.dir, "C:\\GAMES\\FIZ");
	assert!(r.dirs.contains(&"C:\\GAMES".to_string()));
	let (p, rc) = run_keys(
		&c,
		tmp.path(),
		&[Key::Enter, Key::Enter],
		"/U C:\\GAMES\\FIZ",
	);
	assert_eq!(rc, 0);
	assert_eq!(p.tree(), Vec::<String>::new());
}

#[test]
fn the_licence_must_be_accepted_and_hotkeys_work() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	// N is Next's access key: the welcome page. On the licence page it does nothing until the
	// licence is accepted; then F3 quits.
	let (p, rc) = run_keys(
		&c,
		tmp.path(),
		&[Key::Char(b'n'), Key::Char(b'n'), Key::Enter, Key::F(3)],
		"",
	);
	assert_eq!(rc, 3);
	let last = p.frames.last().unwrap();
	assert!(last.contains("Licence"), "{last}");
}

#[test]
fn the_sound_page_takes_blaster_and_writes_the_ini() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	let mut p = HostPlatform::new(tmp.path(), c.lpk().unwrap());
	p.env.insert("BLASTER".into(), "A240 I7 D3 T6".into());
	p.keys(&install_keys());
	assert_eq!(app::run(&mut p, ""), 0);
	let ini = std::fs::read_to_string(tmp.path().join("LTFIZZ/SETUP.INI")).unwrap();
	assert_eq!(
		ini,
		"[sound]\r\ncard=sbpro\r\nport=240\r\nirq=7\r\ndma=3\r\n"
	);
	assert!(
		p.frames
			.iter()
			.any(|f| f.contains("Setup found: Sound Blaster."))
	);
}

#[test]
fn a_damaged_payload_is_caught_and_rolled_back() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	let mut dat = c.lpk().unwrap();
	// Inside the last file's data, just before the footer.
	let n = dat.len();
	dat[n - li_lpk::FOOTER_LEN - 3] ^= 0xff;
	let mut p = HostPlatform::new(tmp.path(), dat);
	let mut keys = up_to_ready();
	keys.extend([Key::Enter, Key::Enter]);
	p.keys(&keys);
	assert_eq!(app::run(&mut p, ""), 1);
	assert_eq!(p.tree(), Vec::<String>::new());
}

#[test]
fn a_damaged_setup_lpk_is_refused() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	let mut dat = c.lpk().unwrap();
	dat[0] = b'X';
	let mut p = HostPlatform::new(tmp.path(), dat);
	assert_eq!(app::run(&mut p, ""), 2);
	let mut dat = c.lpk().unwrap();
	dat.truncate(40);
	let mut p = HostPlatform::new(tmp.path(), dat);
	assert_eq!(app::run(&mut p, ""), 2);
	let mut p = HostPlatform::new(tmp.path(), c.lpk().unwrap());
	assert_eq!(app::run(&mut p, "/X"), 2);
	assert_eq!(app::run(&mut p, "/V"), 0);
}

/// The frame a test compares: the last one whose text contains `marker`.
fn frame<'a>(p: &'a HostPlatform, marker: &str) -> &'a str {
	p.frames
		.iter()
		.rev()
		.find(|f| f.contains(marker))
		.unwrap_or_else(|| panic!("no frame has `{marker}`"))
}

/// Compares `text` with `tests/golden/<name>.txt`; `LI_UPDATE_GOLDEN=1` rewrites it.
pub fn golden(name: &str, text: &str) {
	let path = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("tests/golden")
		.join(format!("{name}.txt"));
	if std::env::var_os("LI_UPDATE_GOLDEN").is_some() {
		std::fs::create_dir_all(path.parent().unwrap()).unwrap();
		std::fs::write(&path, text).unwrap();
	}
	let want = std::fs::read_to_string(&path)
		.unwrap_or_else(|_| panic!("{} is missing; run with LI_UPDATE_GOLDEN=1", path.display()));
	assert_eq!(text, want, "{name} differs from {}", path.display());
}

#[test]
fn the_screens_match_their_goldens() {
	let c = compiled();
	let tmp = tempfile::tempdir().unwrap();
	let mut p = HostPlatform::new(tmp.path(), c.lpk().unwrap());
	p.capacity = 64 * 1024 * 1024;
	p.keys(&install_keys());
	assert_eq!(app::run(&mut p, ""), 0);
	golden(
		"welcome",
		frame(&p, "Welcome to the Lieutenant Fizz setup."),
	);
	golden("components", frame(&p, "Choose what to install."));
	golden("copying", frame(&p, "Copying SOUND\\SBPRO.DRV"));
	golden("done", frame(&p, "Setup complete"));
}

fn copy_dir(from: &Path, to: &Path) {
	std::fs::create_dir_all(to).unwrap();
	for e in std::fs::read_dir(from).unwrap().flatten() {
		let dest = to.join(e.file_name());
		if e.path().is_dir() {
			copy_dir(&e.path(), &dest);
		} else {
			std::fs::copy(e.path(), dest).unwrap();
		}
	}
}

#[test]
fn a_script_hook_on_a_retro_target_is_flagged_and_refused() {
	let tmp = tempfile::tempdir().unwrap();
	copy_dir(&fizz_dir(), tmp.path());
	let flow = tmp.path().join("installer/flows/install.flow.json");
	let src = std::fs::read_to_string(&flow)
		.unwrap()
		.replace("builtin:detect-sound", "scripts/sound-setup.lua#detect");
	std::fs::write(&flow, src).unwrap();
	let project = Project::load(tmp.path()).unwrap();
	let found = li_engine::check(&project);
	let d = found
		.iter()
		.find(|d| d.code == "script-on-retro")
		.expect("script-on-retro");
	assert!(d.message.contains("builtin:detect-sound"), "{}", d.message);
	let err = match li_retro_build::compile(&project, "dos-x86", "1.0.0") {
		Ok(_) => panic!("a script hook must not compile for DOS"),
		Err(e) => e.to_string(),
	};
	assert!(err.contains("Lua"), "{err}");
	// The shipped example is clean.
	let clean = Project::load(&fizz_dir()).unwrap();
	assert!(
		li_engine::check(&clean)
			.iter()
			.all(|d| d.code != "script-on-retro")
	);
}

#[test]
fn names_that_are_not_8_3_are_refused_with_the_name() {
	let tmp = tempfile::tempdir().unwrap();
	copy_dir(&fizz_dir(), tmp.path());
	let comps = tmp.path().join("installer/components.toml");
	let src = std::fs::read_to_string(&comps)
		.unwrap()
		.replace("{install}/SOUND/", "{install}/SOUNDDRIVERS/");
	std::fs::write(&comps, src).unwrap();
	let project = Project::load(tmp.path()).unwrap();
	let err = match li_retro_build::compile(&project, "dos-x86", "1.0.0") {
		Ok(_) => panic!("a long folder name must not compile for DOS"),
		Err(e) => e.to_string(),
	};
	assert!(err.contains("SOUNDDRIVERS") && err.contains("8.3"), "{err}");
}
