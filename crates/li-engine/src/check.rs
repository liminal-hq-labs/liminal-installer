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

fn error(
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

fn warning(
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
	for t in &project.installer.targets {
		if !target_ids.insert(t.id.as_str()) {
			out.push(error(
				"duplicate-target",
				format!("target {}", t.id),
				format!("target `{}` is declared twice", t.id),
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

	// Pages themes insert.
	for theme in project.themes.values() {
		for (name, page) in &theme.file.pages {
			if !theme.layouts.contains_key(&page.layout) {
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
