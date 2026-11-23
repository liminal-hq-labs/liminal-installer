// Asserts that every example project parses, validates and is internally consistent.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeSet;
use std::fs;

use li_examples_test::{EXAMPLES, Example};
use li_model::{Node, StepType};

const NODE_KINDS: &[&str] = &[
	"window",
	"backdrop",
	"dialog",
	"stack",
	"grid",
	"text",
	"image",
	"billboard",
	"meter",
	"button",
	"checklist",
	"radio",
	"field",
	"path",
	"progress",
	"slot",
	"custom",
];
const CONTROLS: &[&str] = &[
	"button",
	"checklist",
	"radio",
	"field",
	"path",
	"progress",
	"meter",
];
const RENDERERS: &[&str] = &[
	"tui",
	"tui-retro",
	"win16",
	"win32-classic",
	"canvas",
	"web",
];
const LIFECYCLES: &[&str] = &["install", "update", "modify", "uninstall", "launch"];
const SCREENS: &[&str] = &[
	"welcome",
	"licence",
	"folder",
	"components",
	"ready",
	"copying",
	"done",
];

fn walk<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
	out.push(node);
	for child in &node.children {
		walk(child, out);
	}
	for over in node.overrides.values() {
		walk(over, out);
	}
}

fn all_nodes(ex: &Example) -> Vec<(&str, &Node)> {
	let mut out = Vec::new();
	for (id, layout) in &ex.layouts {
		let mut nodes = Vec::new();
		walk(&layout.root, &mut nodes);
		out.extend(nodes.into_iter().map(|n| (id.as_str(), n)));
	}
	out
}

#[test]
fn every_example_loads() {
	for name in EXAMPLES {
		let ex = Example::load(name);
		assert!(!ex.installer.targets.is_empty(), "{name}: no targets");
		assert!(
			!ex.components.components.is_empty(),
			"{name}: no components"
		);
		assert!(!ex.themes.is_empty(), "{name}: no themes");
	}
}

#[test]
fn flows_cover_every_lifecycle_and_validate() {
	for name in EXAMPLES {
		let ex = Example::load(name);
		for life in LIFECYCLES {
			let flow = ex
				.flows
				.get(*life)
				.unwrap_or_else(|| panic!("{name}: missing {life} flow"));
			assert_eq!(flow.version, 1);
			let errors = flow.validate();
			assert!(errors.is_empty(), "{name}/{life}: {errors:?}");
			assert!(flow.start_id().is_some());
			let commits = flow
				.steps
				.iter()
				.filter(|s| s.kind == StepType::Commit)
				.count();
			assert_eq!(commits, 1, "{name}/{life}");
		}
	}
}

#[test]
fn flow_layouts_and_scripts_exist() {
	for name in EXAMPLES {
		let ex = Example::load(name);
		for flow in ex.flows.values() {
			for step in &flow.steps {
				if let Some(layout) = &step.layout {
					assert!(
						ex.layouts.contains_key(layout),
						"{name}/{}: step `{}` names missing layout `{layout}`",
						flow.id,
						step.id
					);
				}
				if step.kind == StepType::Page {
					assert!(
						step.layout.is_some(),
						"{name}: page `{}` has no layout",
						step.id
					);
				}
				if let Some(hook) = &step.on_enter {
					if hook.starts_with("builtin:") {
						continue;
					}
					let (file, func) = hook.split_once('#').expect("script#hook");
					let source = fs::read_to_string(ex.dir.join(file))
						.unwrap_or_else(|e| panic!("{name}: {file}: {e}"));
					assert!(
						source.contains(&format!("function {func}(")),
						"{name}: {file} lacks hook `{func}`"
					);
				}
			}
		}
	}
}

#[test]
fn layouts_have_the_standard_screens_and_known_nodes() {
	for name in EXAMPLES {
		let ex = Example::load(name);
		for screen in SCREENS {
			assert!(
				ex.layouts.contains_key(*screen),
				"{name}: no `{screen}` layout"
			);
		}
		for (id, layout) in &ex.layouts {
			assert_eq!(layout.version, 1, "{name}/{id}");
		}
		for (id, node) in all_nodes(&ex) {
			assert!(
				NODE_KINDS.contains(&node.kind.as_str()),
				"{name}/{id}: unknown node kind `{}`",
				node.kind
			);
			for renderer in node.overrides.keys() {
				assert!(
					RENDERERS.contains(&renderer.as_str()),
					"{name}/{id}: unknown renderer `{renderer}`"
				);
			}
		}
	}
}

#[test]
fn every_control_has_an_accessible_name() {
	for name in EXAMPLES {
		let ex = Example::load(name);
		for (id, node) in all_nodes(&ex) {
			if CONTROLS.contains(&node.kind.as_str()) {
				let label = node.props.get("label").and_then(|v| v.as_str());
				assert!(
					label.is_some_and(|l| !l.is_empty()),
					"{name}/{id}: `{}` has no label",
					node.kind
				);
			}
		}
	}
}

#[test]
fn copying_shows_all_three_meters() {
	for name in EXAMPLES {
		let ex = Example::load(name);
		let copying = &ex.layouts["copying"];
		let mut nodes = Vec::new();
		walk(&copying.root, &mut nodes);
		let sources: BTreeSet<&str> = nodes
			.iter()
			.filter(|n| n.kind == "meter")
			.filter_map(|n| n.props.get("source").and_then(|v| v.as_str()))
			.collect();
		assert_eq!(
			sources,
			BTreeSet::from(["file", "total", "free_space"]),
			"{name}"
		);
	}
}

#[test]
fn targets_use_known_themes_and_renderers() {
	for name in EXAMPLES {
		let ex = Example::load(name);
		let default_theme = ex.installer.defaults.theme.clone();
		for target in &ex.installer.targets {
			assert!(
				RENDERERS.contains(&target.renderer.as_str()),
				"{name}: renderer `{}`",
				target.renderer
			);
			let theme_id = target
				.theme
				.clone()
				.or_else(|| default_theme.clone())
				.expect("a theme");
			let theme = ex
				.themes
				.get(&theme_id)
				.unwrap_or_else(|| panic!("{name}: unknown theme `{theme_id}`"));
			assert!(
				theme.theme.renderers.contains(&target.renderer),
				"{name}: theme `{theme_id}` does not support renderer `{}`",
				target.renderer
			);
		}
	}
}

#[test]
fn theme_pages_and_assets_exist() {
	for name in EXAMPLES {
		let ex = Example::load(name);
		let install = &ex.flows["install"];
		let uses_billboard = all_nodes(&ex).iter().any(|(_, n)| n.kind == "billboard");
		for (id, theme) in &ex.themes {
			let theme_dir = ex.dir.join("themes").join(id);
			for (page, insert) in &theme.pages {
				assert!(
					ex.layouts.contains_key(&insert.layout),
					"{name}/{id}: page `{page}` layout `{}`",
					insert.layout
				);
				for anchor in insert.before.iter().chain(insert.after.iter()) {
					assert!(
						install.step(anchor).is_some(),
						"{name}/{id}: anchor `{anchor}`"
					);
				}
			}
			if let Some(audio) = &theme.audio {
				assert!(theme_dir.join(&audio.music).exists(), "{name}/{id}: music");
			}
			if uses_billboard {
				let stems: BTreeSet<String> = fs::read_dir(theme_dir.join("assets"))
					.expect("assets")
					.filter_map(Result::ok)
					.filter_map(|e| {
						e.path()
							.file_stem()
							.and_then(|s| s.to_str())
							.map(str::to_owned)
					})
					.collect();
				for (layout, node) in all_nodes(&ex) {
					if node.kind != "billboard" {
						continue;
					}
					for slide in node.props["slides"].as_array().expect("slides") {
						let slide = slide.as_str().expect("slide name");
						assert!(
							stems.contains(slide),
							"{name}/{id}: layout {layout} slide `{slide}` has no asset"
						);
					}
				}
			}
		}
	}
}

#[test]
fn component_payloads_exist_for_every_applicable_target() {
	for name in EXAMPLES {
		let ex = Example::load(name);
		for component in &ex.components.components {
			let mut applied = 0;
			for target in &ex.installer.targets {
				if !component.applies_to(&target.id) {
					continue;
				}
				applied += 1;
				for rule in &component.files {
					let found = ex.expand_source(&rule.from, &target.id);
					assert!(
						!found.is_empty(),
						"{name}: component `{}` has no payload for `{}` ({})",
						component.id,
						target.id,
						rule.from
					);
				}
			}
			assert!(
				applied > 0,
				"{name}: component `{}` applies to no target",
				component.id
			);
		}
	}
}

#[test]
fn project_files_named_in_installer_toml_exist() {
	for name in EXAMPLES {
		let ex = Example::load(name);
		let licence = ex.installer.product.licence.as_deref().expect("licence");
		assert!(ex.dir.join(licence).exists(), "{name}: licence");
		let key = ex.installer.signing.feed_key.as_deref().expect("feed key");
		assert!(ex.dir.join(key).exists(), "{name}: feed key");
		let channels = fs::read_to_string(ex.dir.join("feed/channels.toml")).expect("channels");
		channels
			.parse::<toml::Table>()
			.expect("channels.toml parses");
	}
}

#[test]
fn fizz_gates_the_sound_card_to_retro_renderers() {
	let ex = Example::load("lt-fizz");
	let step = ex.flows["install"]
		.step("sound-card")
		.expect("sound-card step");
	let when = step.when.as_deref().expect("a condition");
	assert!(when.contains("tui-retro") && when.contains("win16"));
	let sound = ex
		.components
		.components
		.iter()
		.find(|c| c.id == "sound")
		.expect("sound component");
	assert!(sound.applies_to("dos-x86") && !sound.applies_to("windows-x86_64"));
	// The retro runtime has no Lua, so the page names the built-in; the script remains for the
	// runners that run scripts.
	assert_eq!(step.on_enter.as_deref(), Some("builtin:detect-sound"));
	let script = fs::read_to_string(ex.dir.join("scripts/sound-setup.lua")).expect("script");
	assert!(script.contains("function detect(") && script.contains("function validate("));
}

#[test]
fn linux_payloads_are_executable_shell_scripts() {
	use std::os::unix::fs::PermissionsExt;
	for name in EXAMPLES {
		let ex = Example::load(name);
		let dir = ex.dir.join("dist/linux-x86_64");
		for entry in fs::read_dir(dir).expect("linux dist") {
			let path = entry.expect("entry").path();
			// Resource folders hold data, not programs.
			if path.is_dir() {
				continue;
			}
			let text = fs::read_to_string(&path).expect("text");
			assert!(text.starts_with("#!/bin/sh"), "{}", path.display());
			let mode = fs::metadata(&path).expect("meta").permissions().mode();
			assert!(mode & 0o111 != 0, "{} is not executable", path.display());
		}
	}
}
