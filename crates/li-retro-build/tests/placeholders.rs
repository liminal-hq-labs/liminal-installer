// Every page of every project compiles for the three retro targets with its placeholders filled in.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A page's text is a template the runtime fills from the product and the install state. These
//! tests walk the whole flow of each project on each retro target and fail on any `{` left in the
//! text a person would read, and on a welcome page with no body text.

use std::path::{Path, PathBuf};

use li_engine::Project;
use li_retro::app;
use li_retro::sim::{GuiOp, HostPlatform, ScriptGui};
use li_retro_build::Flavour;

fn examples() -> Vec<PathBuf> {
	let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
	let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
		.unwrap()
		.map(|e| e.unwrap().path())
		.filter(|p| p.join("installer/installer.toml").exists())
		.collect();
	// A checkout of Quill's installer, when one is given: `QUILL_INSTALLER=/path/to/quill/installer`.
	if let Some(q) = std::env::var_os("QUILL_INSTALLER") {
		dirs.push(PathBuf::from(q).parent().unwrap().to_path_buf());
	}
	dirs.sort();
	dirs
}

fn flavour_of(renderer: &str) -> Option<Flavour> {
	match renderer {
		"tui-retro" => Some(Flavour::Dos),
		"win16" => Some(Flavour::Win16),
		"win32-classic" => Some(Flavour::Win32),
		_ => None,
	}
}

fn gui() -> ScriptGui {
	let mut g = ScriptGui::default();
	g.on(
		"licence",
		vec![GuiOp::Choose("licence.accepted".into(), "yes".into())],
	);
	g
}

/// The pages of one install, as the native window would describe them.
fn pages(dir: &Path, target: &str, flavour: Flavour) -> Vec<String> {
	let project = Project::load(dir).expect("load");
	let c = li_retro_build::compile_for(&project, target, "1.2.3", flavour).expect("compile");
	let tmp = tempfile::tempdir().unwrap();
	std::fs::create_dir_all(tmp.path().join("Program Files")).unwrap();
	let mut p = HostPlatform::new(tmp.path(), c.lpk().unwrap());
	p.native = flavour == Flavour::Win32;
	p.gui = Some(gui());
	assert_eq!(app::run(&mut p, ""), 0, "{dir:?} {target}");
	p.gui.as_ref().unwrap().described.clone()
}

#[test]
fn no_page_of_any_native_target_keeps_a_placeholder() {
	let mut seen = 0;
	for dir in examples() {
		let project = Project::load(&dir).expect("load");
		for t in &project.installer.targets {
			let Some(flavour) = flavour_of(&t.renderer) else {
				continue;
			};
			// A project may list one id twice, for two renderers; the first is the one that builds.
			if project
				.target(&t.id)
				.is_some_and(|f| f.renderer != t.renderer)
			{
				continue;
			}
			if flavour == Flavour::Dos {
				continue;
			}
			for page in pages(&dir, &t.id, flavour) {
				assert!(
					!page.contains('{'),
					"{dir:?} {}: a placeholder is left in\n{page}",
					t.id
				);
				seen += 1;
			}
		}
	}
	assert!(seen > 0);
}

#[test]
fn the_welcome_page_has_body_text_on_every_native_target() {
	for dir in examples() {
		let project = Project::load(&dir).expect("load");
		for t in &project.installer.targets {
			let Some(flavour) = flavour_of(&t.renderer) else {
				continue;
			};
			// A project may list one id twice, for two renderers; the first is the one that builds.
			if project
				.target(&t.id)
				.is_some_and(|f| f.renderer != t.renderer)
			{
				continue;
			}
			if flavour == Flavour::Dos {
				continue;
			}
			let all = pages(&dir, &t.id, flavour);
			let welcome = all
				.iter()
				.find(|p| p.starts_with("== welcome"))
				.unwrap_or_else(|| panic!("{dir:?} {}: no welcome page", t.id));
			let name = &project.installer.product.name;
			let body = welcome.lines().filter(|l| l.starts_with("text: ")).count();
			assert!(
				body > 0,
				"{dir:?} {}: the welcome page has no text\n{welcome}",
				t.id
			);
			assert!(
				welcome.contains(name.as_str()),
				"{dir:?} {}: the welcome page does not name the product\n{welcome}",
				t.id
			);
		}
	}
}
