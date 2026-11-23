// Project checks: files, flows, themes, layouts and components.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeSet;

use li_model::{Node, StepType, Target};

use crate::diag::{Diagnostic, Report};
use crate::project::{Project, ThemeData};

/// One set of checks over a loaded project. Implementations add their own findings and are run
/// together by [`check_project`].
pub trait ProjectChecker {
	/// Checks the whole project, or only `targets` when the slice is not empty.
	fn check(&self, project: &Project, targets: &[&Target]) -> Vec<Diagnostic>;
}

/// The CLI's own checks: flow rules, themes, layouts, accessible names and components.
pub struct DefaultChecker;

pub const KNOWN_RENDERERS: &[&str] = &[
	"tui-retro",
	"tui",
	"win16",
	"win32-classic",
	"canvas",
	"web",
];

pub const NODE_KINDS: &[&str] = &[
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
	"custom",
];

const CONTROLS: &[&str] = &["button", "checklist", "radio", "field", "path"];

/// Runs `checkers` over the project (all targets, or `only` when given) and builds the report.
pub fn check_project(
	project: &Project,
	only: &[String],
	checkers: &[&dyn ProjectChecker],
) -> Report {
	let targets: Vec<&Target> = project
		.installer
		.targets
		.iter()
		.filter(|t| only.is_empty() || only.contains(&t.id))
		.collect();
	let mut diags = Vec::new();
	for c in checkers {
		diags.extend(c.check(project, &targets));
	}
	Report::new(targets.iter().map(|t| t.id.clone()).collect(), diags)
}

fn file_of(_project: &Project, name: &str) -> String {
	format!("installer/{name}")
}

/// Finding codes the CLI's own checks already report, in their own wording and severity.
const COVERED_BY_DEFAULT: &[&str] = &[
	"duplicate-target",
	"duplicate-component",
	"unknown-target",
	"unknown-theme",
	"missing-source",
	"missing-script",
	"bad-flow",
	"unknown-layout",
];

/// The engine's checks: condition syntax, components named in conditions and script hooks.
/// Findings [`DefaultChecker`] already makes are left to it.
pub struct EngineChecker;

impl ProjectChecker for EngineChecker {
	fn check(&self, project: &Project, _targets: &[&Target]) -> Vec<Diagnostic> {
		li_engine::check(project)
			.into_iter()
			.filter(|d| !COVERED_BY_DEFAULT.contains(&d.code))
			.map(|d| {
				let message = format!("{} ({})", d.message, d.location);
				let diag = match d.severity {
					li_engine::Severity::Error => Diagnostic::error(d.code, message),
					li_engine::Severity::Warning => Diagnostic::warning(d.code, message),
				};
				match d.location.strip_prefix("flow ") {
					Some(rest) => {
						let id = rest.split(',').next().unwrap_or(rest);
						diag.in_file(format!("installer/flows/{id}.flow.json"))
					}
					None => diag,
				}
			})
			.collect()
	}
}

impl ProjectChecker for DefaultChecker {
	fn check(&self, project: &Project, targets: &[&Target]) -> Vec<Diagnostic> {
		let mut d = Vec::new();
		check_installer(project, &mut d);
		check_components(project, targets, &mut d);
		check_flows(project, targets, &mut d);
		check_themes(project, targets, &mut d);
		d
	}
}

fn check_installer(project: &Project, d: &mut Vec<Diagnostic>) {
	let f = file_of(project, "installer.toml");
	let inst = &project.installer;
	if inst.product.id.trim().is_empty() {
		d.push(Diagnostic::error("empty-product-id", "product.id is empty").in_file(&f));
	}
	if inst.targets.is_empty() {
		d.push(Diagnostic::error("no-targets", "the project declares no [[target]]").in_file(&f));
	}
	let mut seen = BTreeSet::new();
	for t in &inst.targets {
		if !seen.insert(t.id.as_str()) {
			d.push(
				Diagnostic::error(
					"duplicate-target",
					format!("target `{}` is declared twice", t.id),
				)
				.in_file(&f),
			);
		}
		if !KNOWN_RENDERERS.contains(&t.renderer.as_str()) {
			d.push(
				Diagnostic::warning(
					"unknown-renderer",
					format!(
						"target `{}` uses renderer `{}`, which is not one of {}",
						t.id,
						t.renderer,
						KNOWN_RENDERERS.join(", ")
					),
				)
				.in_file(&f),
			);
		}
		match project.theme_id_for(t) {
			Some(name) if !project.themes.contains_key(name) => d.push(
				Diagnostic::error(
					"missing-theme",
					format!(
						"target `{}` uses theme `{name}`, but installer/themes/{name}/ has none",
						t.id
					),
				)
				.in_file(&f),
			),
			None => d.push(
				Diagnostic::error(
					"no-theme",
					format!(
						"target `{}` has no theme and the project sets no default",
						t.id
					),
				)
				.in_file(&f),
			),
			_ => {}
		}
	}
}

fn check_components(project: &Project, targets: &[&Target], d: &mut Vec<Diagnostic>) {
	let f = file_of(project, "components.toml");
	let comps = &project.components.components;
	if comps.is_empty() {
		d.push(Diagnostic::warning("no-components", "the project installs nothing").in_file(&f));
	}
	let mut seen = BTreeSet::new();
	let known: BTreeSet<&str> = project
		.installer
		.targets
		.iter()
		.map(|t| t.id.as_str())
		.collect();
	for c in comps {
		if !seen.insert(c.id.as_str()) {
			d.push(
				Diagnostic::error(
					"duplicate-component",
					format!("component `{}` is declared twice", c.id),
				)
				.in_file(&f),
			);
		}
		for t in &c.targets {
			if !known.contains(t.as_str()) {
				d.push(
					Diagnostic::error(
						"unknown-component-target",
						format!(
							"component `{}` names target `{t}`, which the project does not declare",
							c.id
						),
					)
					.in_file(&f),
				);
			}
		}
		for rule in &c.files {
			for t in targets.iter().filter(|t| c.applies_to(&t.id)) {
				let from = rule.from.replace("{target}", &t.id);
				if !from.contains('*') && !project.dir.join(&from).exists() {
					d.push(
						Diagnostic::warning(
							"missing-source",
							format!(
								"component `{}` file `{from}` does not exist yet (target `{}`)",
								c.id, t.id
							),
						)
						.in_file(&f),
					);
				}
			}
		}
	}
	for t in targets {
		if !comps.is_empty() && !comps.iter().any(|c| c.applies_to(&t.id)) {
			d.push(
				Diagnostic::warning(
					"no-components-for-target",
					format!("no component applies to target `{}`", t.id),
				)
				.in_file(&f),
			);
		}
	}
}

fn has_theme(project: &Project, t: &Target) -> bool {
	project
		.theme_id_for(t)
		.is_some_and(|id| project.themes.contains_key(id))
}

fn theme_dir_name(theme: &ThemeData) -> String {
	theme
		.dir
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_default()
}

fn check_flows(project: &Project, targets: &[&Target], d: &mut Vec<Diagnostic>) {
	if !project.flows.contains_key("install") {
		d.push(
			Diagnostic::error("missing-install-flow", "the project has no install flow")
				.in_file("installer/flows/install.flow.json"),
		);
	}
	for (name, flow) in &project.flows {
		let f = project
			.flow_files
			.get(name)
			.map(|p| crate::project::rel(&project.root, p))
			.unwrap_or_else(|| format!("installer/flows/{name}.flow.json"));
		for e in flow.validate() {
			d.push(Diagnostic::error("invalid-flow", e.to_string()).in_file(&f));
		}
		for step in &flow.steps {
			if let Some(hook) = &step.on_enter {
				let script = hook.split('#').next().unwrap_or(hook);
				if !project.dir.join(script).is_file() {
					d.push(
						Diagnostic::error(
							"missing-script",
							format!("step `{}` runs `{script}`, which does not exist", step.id),
						)
						.in_file(&f),
					);
				}
			}
			let needs_layout = matches!(
				step.kind,
				StepType::Page | StepType::Commit | StepType::Finish
			);
			match (&step.layout, needs_layout) {
				(None, true) if step.kind == StepType::Page => d.push(
					Diagnostic::error(
						"page-without-layout",
						format!("page step `{}` names no layout", step.id),
					)
					.in_file(&f),
				),
				(Some(layout), _) => {
					for t in targets {
						if has_theme(project, t) && project.layout_for(t, layout).is_none() {
							d.push(
								Diagnostic::error(
									"missing-layout",
									format!(
										"step `{}` uses layout `{layout}`, which theme `{}` does not have (target `{}`)",
										step.id,
										project.theme_id_for(t).unwrap_or_default(),
										t.id
									),
								)
								.in_file(&f),
							);
						}
					}
				}
				_ => {}
			}
		}
	}
}

fn check_themes(project: &Project, targets: &[&Target], d: &mut Vec<Diagnostic>) {
	for theme in project.themes.values() {
		let name = theme_dir_name(theme);
		let f = format!("installer/themes/{name}/theme.toml");
		if theme.file.theme.id != name {
			d.push(
				Diagnostic::warning(
					"theme-id-mismatch",
					format!(
						"theme id `{}` differs from its folder name `{name}`",
						theme.file.theme.id
					),
				)
				.in_file(&f),
			);
		}
		if let Some(fb) = &theme.file.theme.fallback
			&& !project.themes.contains_key(fb)
		{
			d.push(
				Diagnostic::error(
					"missing-fallback-theme",
					format!("theme `{name}` falls back to `{fb}`, which the project does not have"),
				)
				.in_file(&f),
			);
		}
		for (page, insert) in &theme.file.pages {
			if !theme.layouts.contains_key(&insert.layout) {
				d.push(
					Diagnostic::error(
						"missing-layout",
						format!(
							"theme page `{page}` uses layout `{}`, which the theme does not have",
							insert.layout
						),
					)
					.in_file(&f),
				);
			}
		}
		for (id, layout) in &theme.layouts {
			let lf = format!("installer/themes/{name}/layouts/{id}.layout.json");
			check_node(&layout.root, &lf, d);
		}
	}
	for t in targets {
		if let Some(theme) = project
			.theme_id_for(t)
			.and_then(|id| project.themes.get(id))
			&& !theme.file.theme.renderers.contains(&t.renderer)
			&& theme.file.theme.fallback.is_none()
		{
			d.push(
				Diagnostic::warning(
					"renderer-not-supported",
					format!(
						"theme `{}` does not list renderer `{}` (target `{}`) and has no fallback",
						theme.file.theme.id, t.renderer, t.id
					),
				)
				.in_file("installer/installer.toml"),
			);
		}
	}
}

fn check_node(node: &Node, file: &str, d: &mut Vec<Diagnostic>) {
	if !NODE_KINDS.contains(&node.kind.as_str()) {
		d.push(
			Diagnostic::error(
				"unknown-node",
				format!("unknown layout node kind `{}`", node.kind),
			)
			.in_file(file),
		);
	}
	if CONTROLS.contains(&node.kind.as_str()) {
		let named = ["name", "label", "text"].iter().any(|k| {
			node.props
				.get(*k)
				.and_then(|v| v.as_str())
				.is_some_and(|s| !s.is_empty())
		});
		if !named {
			d.push(
				Diagnostic::error(
					"no-accessible-name",
					format!(
						"a `{}` control{} has no accessible name (set `name` or `label`)",
						node.kind,
						node.bind
							.as_deref()
							.map_or(String::new(), |b| format!(" bound to `{b}`"))
					),
				)
				.in_file(file),
			);
		}
	}
	for child in &node.children {
		check_node(child, file, d);
	}
	for (renderer, over) in &node.overrides {
		if !KNOWN_RENDERERS.contains(&renderer.as_str()) {
			d.push(
				Diagnostic::warning(
					"unknown-override",
					format!("override for unknown renderer `{renderer}`"),
				)
				.in_file(file),
			);
		}
		check_node(over, file, d);
	}
}
