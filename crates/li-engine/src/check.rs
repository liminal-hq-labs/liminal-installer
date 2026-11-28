// Project checks: what `liminst check` reports.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeSet;
use std::fmt;

use crate::cond;
use crate::payload::{DirPayload, expand_sources};
use crate::project::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
	Error,
	Warning,
}

/// One finding. `code` is stable and meant for tests and tools; `location` names the file,
/// flow step, component or target at fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
	pub severity: Severity,
	pub code: &'static str,
	pub message: String,
	pub location: String,
}

impl fmt::Display for Diagnostic {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let level = match self.severity {
			Severity::Error => "error",
			Severity::Warning => "warning",
		};
		write!(
			f,
			"{level}[{}]: {} ({})",
			self.code, self.message, self.location
		)
	}
}

pub(crate) fn error(
	code: &'static str,
	location: impl Into<String>,
	message: impl Into<String>,
) -> Diagnostic {
	Diagnostic {
		severity: Severity::Error,
		code,
		message: message.into(),
		location: location.into(),
	}
}

pub(crate) fn warning(
	code: &'static str,
	location: impl Into<String>,
	message: impl Into<String>,
) -> Diagnostic {
	Diagnostic {
		severity: Severity::Warning,
		..error(code, location, message)
	}
}

fn check_condition(project: &Project, src: &str, location: &str, out: &mut Vec<Diagnostic>) {
	match cond::parse(src) {
		Err(e) => out.push(error("bad-condition", location, format!("`{src}`: {e}"))),
		Ok(expr) => {
			let mut mentioned = Vec::new();
			expr.component_mentions(&mut mentioned);
			for id in mentioned {
				if project.component(&id).is_none() {
					out.push(error(
						"unknown-component",
						location,
						format!(
							"condition names component `{id}`, which is not in components.toml"
						),
					));
				}
			}
		}
	}
}

/// The actions a step may name as `builtin:<name>`. The lifecycle carries out `check-feed` and
/// `launch-app`; the DOS runtime carries out `detect-sound` and `check-disk-space`, which are
/// no-ops everywhere else.
pub const BUILTIN_ACTIONS: &[&str] = &[
	"check-feed",
	"launch-app",
	"detect-sound",
	"check-disk-space",
];

/// The renderers whose runtimes have no Lua interpreter (DOS and Windows 3.1), so a step on one can
/// only name a built-in.
pub const NO_SCRIPT_RENDERERS: &[&str] = &["tui-retro", "win16"];

/// The renderers whose runtimes have a Windows folder and a system folder: Windows 3.x and
/// Windows 95 and later.
pub const SYSTEM_FOLDER_RENDERERS: &[&str] = &["win16", "win32-classic"];

/// A `to` that starts with `{windows}` or `{system}`: the placeholder and what follows it.
fn system_folder_of(to: &str) -> Option<(&'static str, &str)> {
	["{windows}", "{system}"].into_iter().find_map(|name| {
		let rest = to.strip_prefix(name)?;
		(rest.is_empty() || rest.starts_with('/')).then_some((name, rest))
	})
}

/// `{windows}` and `{system}` take a file directly in the folder, and only where there is one.
fn check_system_folders(project: &Project, out: &mut Vec<Diagnostic>) {
	for c in &project.components.components {
		for rule in &c.files {
			let Some((name, rest)) = system_folder_of(&rule.to) else {
				continue;
			};
			let loc = format!("component {}", c.id);
			if let Some(t) = project.installer.targets.iter().find(|t| {
				c.applies_to(&t.id) && !SYSTEM_FOLDER_RENDERERS.contains(&t.renderer.as_str())
			}) {
				out.push(error(
					"system-folder-unsupported",
					&loc,
					format!(
						"component `{}` installs `{}` into the Windows folder, which target `{}` (`{}`) does not have; limit the component's `targets` to the Windows 3.x and Windows 95 targets",
						c.id, rule.to, t.id, t.renderer
					),
				));
			}
			// `{windows}/` and `{windows}/NAME` are fine; any further `/`, even a trailing
			// one, names a folder below the placeholder.
			let below = rest.strip_prefix('/').unwrap_or(rest);
			if below.contains('/') {
				out.push(error(
					"system-folder-subfolder",
					&loc,
					format!(
						"component `{}`: only a file directly in `{name}` can be installed, not one in a folder below it",
						c.id
					),
				));
			}
		}
	}
}

/// The places a shortcut may go, as `in = "..."` names them.
pub const SHORTCUT_PLACES: &[&str] = &["start_menu", "desktop", "startup"];

/// The renderers whose runtimes can run a program before an uninstall removes files and make a
/// shortcut in the Startup folder with a command line: Windows 95 and later and the engine's own.
/// DOS and Windows 3.x do neither, and `check` says so.
const RETRO_WITHOUT_SHELL_EXTRAS: &[&str] = &["tui-retro", "win16"];

/// Shortcut places and arguments, and `[[uninstall.pre]]` with the `run` capability it needs.
fn check_install_extras(project: &Project, out: &mut Vec<Diagnostic>) {
	let inst = &project.installer;
	for cap in &inst.capabilities {
		if !li_model::CAPABILITIES.contains(&cap.as_str()) {
			out.push(error(
				"unknown-capability",
				"installer.toml",
				format!(
					"unknown capability `{cap}`; the capabilities are {}",
					li_model::CAPABILITIES.join(", ")
				),
			));
		}
	}
	let declared = inst.capabilities.iter().any(|c| c == "run");
	if !inst.uninstall.pre.is_empty() && !declared {
		out.push(error(
			"run-undeclared",
			"installer.toml",
			"`[[uninstall.pre]]` starts a program, which needs `capabilities = [\"run\"]` at the top of installer.toml",
		));
	}
	for pre in &inst.uninstall.pre {
		if pre.run.trim().is_empty() {
			out.push(error(
				"bad-uninstall-pre",
				"installer.toml",
				"an `[[uninstall.pre]]` entry has an empty `run`",
			));
		}
		for t in inst
			.targets
			.iter()
			.filter(|t| RETRO_WITHOUT_SHELL_EXTRAS.contains(&t.renderer.as_str()))
		{
			out.push(warning(
				"uninstall-pre-unsupported",
				format!("target {}", t.id),
				format!(
					"`[[uninstall.pre]]` runs `{}`, which target `{}` (`{}`) does not: its runtime cannot start a program, so the entry is left out of that build",
					pre.run, t.id, t.renderer
				),
			));
		}
		for t in inst
			.targets
			.iter()
			.filter(|t| t.renderer == "win32-classic")
		{
			if !pre.run.starts_with("{install}/") {
				out.push(error(
					"uninstall-pre-outside-install",
					format!("target {}", t.id),
					format!(
						"`[[uninstall.pre]]` runs `{}`; the Windows 95 runtime runs only a program of the product, as `{{install}}/NAME.EXE`",
						pre.run
					),
				));
			}
		}
	}
	for c in &project.components.components {
		for s in &c.shortcut {
			let loc = format!("component {}", c.id);
			let place = s.place.trim_matches(['{', '}']);
			if !SHORTCUT_PLACES.contains(&place) {
				out.push(error(
					"unknown-shortcut-place",
					&loc,
					format!(
						"shortcut `{}` is `in = \"{}\"`; the places are {}",
						s.name,
						s.place,
						SHORTCUT_PLACES.join(", ")
					),
				));
			}
			let has_args = s.args.as_deref().is_some_and(|a| !a.is_empty());
			for t in project.installer.targets.iter().filter(|t| {
				c.applies_to(&t.id) && RETRO_WITHOUT_SHELL_EXTRAS.contains(&t.renderer.as_str())
			}) {
				if has_args {
					out.push(warning(
						"shortcut-args-unsupported",
						&loc,
						format!(
							"shortcut `{}` has `args`, which target `{}` (`{}`) leaves out: its launcher takes no fixed command line",
							s.name, t.id, t.renderer
						),
					));
				}
				if place == "startup" {
					out.push(warning(
						"startup-unsupported",
						&loc,
						format!(
							"shortcut `{}` is in `startup`, which target `{}` (`{}`) has no place for; the shortcut is left out of that build",
							s.name, t.id, t.renderer
						),
					));
				}
			}
		}
	}
}

/// Validates a project for every target.
pub fn check(project: &Project) -> Vec<Diagnostic> {
	let mut out = Vec::new();

	// Targets and themes.
	if project.installer.targets.is_empty() {
		out.push(warning(
			"no-targets",
			"installer.toml",
			"the project has no [[target]]",
		));
	}
	let mut target_ids = BTreeSet::new();
	let mut full_ids = BTreeSet::new();
	for t in &project.installer.targets {
		target_ids.insert(t.id.as_str());
		// One os-arch may appear once per renderer.
		if !full_ids.insert(t.full_id()) {
			out.push(error(
				"duplicate-target",
				format!("target {}", t.id),
				format!("target `{}` is declared twice", t.full_id()),
			));
		}
	}
	let mut missing_themes = BTreeSet::new();
	for t in &project.installer.targets {
		if let Some(id) = project.theme_id_for(t)
			&& !project.themes.contains_key(id)
			&& missing_themes.insert(id)
		{
			out.push(warning(
				"unknown-theme",
				format!("target {}", t.id),
				format!("theme `{id}` is not in this project, so it must be a published theme"),
			));
		}
	}

	// Components.
	let mut ids = BTreeSet::new();
	for c in &project.components.components {
		if !ids.insert(c.id.as_str()) {
			out.push(error(
				"duplicate-component",
				format!("component {}", c.id),
				format!("component `{}` is declared twice", c.id),
			));
		}
		if c.required && !c.default {
			out.push(error(
				"required-default-off",
				format!("component {}", c.id),
				format!(
					"component `{}` is required, so it cannot start unticked (`default = false`)",
					c.id
				),
			));
		}
		for t in &c.targets {
			if !target_ids.contains(t.as_str()) {
				out.push(error(
					"unknown-target",
					format!("component {}", c.id),
					format!(
						"component `{}` names target `{t}`, which is not in installer.toml",
						c.id
					),
				));
			}
		}
	}

	// Source files, for every target the component applies to.
	match DirPayload::new(&project.dir) {
		Ok(dir) => {
			for c in &project.components.components {
				for t in &project.installer.targets {
					if !c.applies_to(&t.id) {
						continue;
					}
					for rule in &c.files {
						let pattern = rule.from.replace("{target}", &t.id);
						if expand_sources(&dir, &pattern).is_empty() {
							out.push(error(
								"missing-source",
								format!("component {}", c.id),
								format!("no file matches `{pattern}` for target `{}`", t.id),
							));
						}
					}
				}
			}
		}
		Err(e) => out.push(error("unreadable-project", "installer/", e.to_string())),
	}

	check_system_folders(project, &mut out);
	check_install_extras(project, &mut out);

	crate::tauri::check::check(project, &mut out);

	// Flows.
	for (id, flow) in &project.flows {
		for e in flow.validate() {
			out.push(error("bad-flow", format!("flow {id}"), e.to_string()));
		}
		for step in &flow.steps {
			let loc = format!("flow {id}, step {}", step.id);
			if let Some(w) = &step.when {
				check_condition(project, w, &loc, &mut out);
			}
			for b in &step.branches {
				check_condition(project, &b.when, &loc, &mut out);
			}
			if let Some(spec) = &step.on_enter {
				match spec.split_once('#') {
					_ if spec.starts_with("builtin:") => {
						let name = &spec["builtin:".len()..];
						if !BUILTIN_ACTIONS.contains(&name) {
							out.push(error(
								"bad-hook",
								&loc,
								format!(
									"unknown built-in action `{name}` (known: {})",
									BUILTIN_ACTIONS.join(", ")
								),
							));
						}
					}
					Some((path, hook)) => {
						if !project.scripts.contains_key(path) {
							out.push(error(
								"missing-script",
								&loc,
								format!("on_enter names `{path}`, which is not in scripts/"),
							));
						}
						if !matches!(hook, "detect" | "validate") {
							out.push(error(
								"bad-hook",
								&loc,
								format!("on_enter hook `{hook}` must be `detect` or `validate`"),
							));
						}
					}
					None => out.push(error(
						"bad-hook",
						&loc,
						format!("on_enter `{spec}` must look like `scripts/name.lua#detect`"),
					)),
				}
			}
			if let Some(layout) = &step.layout {
				for t in &project.installer.targets {
					let Some(theme_id) = project.theme_id_for(t) else {
						continue;
					};
					let Some(theme) = project.themes.get(theme_id) else {
						continue;
					};
					let chain_leaves_project = {
						let mut cur = theme;
						let mut leaves = false;
						let mut hops = 0;
						while let Some(fb) = cur.file.theme.fallback.as_deref() {
							hops += 1;
							match project.themes.get(fb) {
								Some(next) if hops < 16 => cur = next,
								Some(_) => break,
								None => {
									leaves = true;
									break;
								}
							}
						}
						leaves
					};
					if project.layout_for(t, layout).is_none() && !chain_leaves_project {
						out.push(error(
							"unknown-layout",
							&loc,
							format!(
								"layout `{layout}` is not in theme `{theme_id}` for target `{}`",
								t.id
							),
						));
					}
				}
			}
		}
	}

	// The DOS runtime does not run scripts: a hook there cannot work.
	for t in project
		.installer
		.targets
		.iter()
		.filter(|t| NO_SCRIPT_RENDERERS.contains(&t.renderer.as_str()))
	{
		for (id, flow) in &project.flows {
			for step in &flow.steps {
				if let Some(spec) = step
					.on_enter
					.as_deref()
					.filter(|s| !s.starts_with("builtin:"))
				{
					out.push(error(
						"script-on-retro",
						format!("flow {id}, step {}", step.id),
						format!(
							"on_enter `{spec}` runs a script, and target `{}` uses the `{}` renderer, whose runtime has no Lua; use a built-in such as `builtin:detect-sound` or `builtin:check-disk-space`",
							t.id, t.renderer
						),
					));
				}
			}
		}
	}

	// Pages themes insert.
	for theme in project.themes.values() {
		for (name, page) in &theme.file.pages {
			if !theme.layouts.contains_key(&page.layout)
				&& !project.layouts.contains_key(&page.layout)
			{
				out.push(error(
					"unknown-layout",
					format!("theme {}, page {name}", theme.file.theme.id),
					format!("layout `{}` is not in the theme", page.layout),
				));
			}
		}
	}
	out
}
