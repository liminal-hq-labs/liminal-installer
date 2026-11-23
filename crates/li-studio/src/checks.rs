// Project checks that need no engine: references, targets and payload files.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_model::{FlowError, Node, StepType};

use crate::backend::Problem;
use crate::project::Project;

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
const NEEDS_NAME: &[&str] = &[
	"button",
	"checklist",
	"radio",
	"field",
	"path",
	"progress",
	"meter",
];
pub const RENDERERS: &[&str] = &[
	"tui",
	"tui-retro",
	"win16",
	"win32-classic",
	"canvas",
	"web",
];

/// The flow error as a problem, with the step it concerns where there is one.
pub fn flow_problem(flow_id: &str, err: &FlowError) -> Problem {
	let scope = format!("flow:{flow_id}");
	let p = Problem::error(scope, err.to_string());
	match err {
		FlowError::DuplicateStep(s) | FlowError::DeadEnd(s) | FlowError::NoFinish(s) => {
			p.at_step(s.clone())
		}
		FlowError::UnknownStep { from, .. } => p.at_step(from.clone()),
		FlowError::CommitCount(_) | FlowError::Empty => p,
	}
}

pub fn flow_problems(flow: &li_model::Flow) -> Vec<Problem> {
	flow.validate()
		.iter()
		.map(|e| flow_problem(&flow.id, e))
		.collect()
}

fn walk<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
	out.push(node);
	for c in &node.children {
		walk(c, out);
	}
	for o in node.overrides.values() {
		walk(o, out);
	}
}

/// Checks that need no engine, over the whole project.
pub fn check_project(project: &Project) -> Vec<Problem> {
	let mut out = Vec::new();
	let target_ids: Vec<&str> = project
		.installer
		.targets
		.iter()
		.map(|t| t.id.as_str())
		.collect();

	for flow in project.flows.values() {
		out.extend(flow_problems(flow));
		for step in &flow.steps {
			let scope = format!("flow:{}", flow.id);
			if let Some(layout) = &step.layout
				&& project.find_layout(layout).is_none()
			{
				out.push(
					Problem::error(&scope, format!("layout `{layout}` does not exist"))
						.at_step(&step.id),
				);
			}
			if step.kind == StepType::Page && step.layout.is_none() {
				out.push(Problem::error(&scope, "a page step needs a layout").at_step(&step.id));
			}
			if let Some(hook) = &step.on_enter
				&& !hook.starts_with("builtin:")
			{
				match hook.split_once('#') {
					None => out.push(
						Problem::error(&scope, format!("`{hook}` should look like path#hook"))
							.at_step(&step.id),
					),
					Some((file, name)) => match project.scripts.get(file) {
						None => out.push(
							Problem::error(&scope, format!("script `{file}` does not exist"))
								.at_step(&step.id),
						),
						Some(src) if !src.contains(&format!("function {name}(")) => out.push(
							Problem::error(&scope, format!("`{file}` does not define `{name}`"))
								.at_step(&step.id),
						),
						Some(_) => {}
					},
				}
			}
		}
	}

	for layout in project.layouts.values() {
		let scope = format!("layout:{}", layout.id);
		let mut nodes = Vec::new();
		walk(&layout.root, &mut nodes);
		for n in &nodes {
			if !NODE_KINDS.contains(&n.kind.as_str()) {
				out.push(Problem::error(
					&scope,
					format!("unknown node kind `{}`", n.kind),
				));
			}
			if NEEDS_NAME.contains(&n.kind.as_str())
				&& n.props
					.get("label")
					.and_then(|v| v.as_str())
					.is_none_or(str::is_empty)
			{
				out.push(Problem::error(
					&scope,
					format!("`{}` needs an accessible name (`label`)", n.kind),
				));
			}
			for r in n.overrides.keys() {
				if !RENDERERS.contains(&r.as_str()) {
					out.push(Problem::warning(&scope, format!("unknown renderer `{r}`")));
				}
			}
		}
	}

	for target in &project.installer.targets {
		let scope = format!("target:{}", target.full_id());
		if !RENDERERS.contains(&target.renderer.as_str()) {
			out.push(Problem::error(
				&scope,
				format!("unknown renderer `{}`", target.renderer),
			));
		}
		match project.theme_for(target) {
			None => out.push(Problem::warning(
				&scope,
				"no theme; the neutral default is used",
			)),
			Some(id) => match project.themes.get(&id) {
				None => out.push(Problem::error(
					&scope,
					format!("theme `{id}` does not exist"),
				)),
				Some(t) if !t.theme.renderers.contains(&target.renderer) => {
					out.push(Problem::error(
						&scope,
						format!("theme `{id}` does not support `{}`", target.renderer),
					))
				}
				Some(_) => {}
			},
		}
		for c in &project.components.components {
			if !c.applies_to(&target.id) {
				continue;
			}
			for rule in &c.files {
				if project.expand_source(&rule.from, &target.id).is_empty() {
					out.push(Problem::error(
						&scope,
						format!("component `{}`: nothing matches `{}`", c.id, rule.from),
					));
				}
			}
		}
	}
	for c in &project.components.components {
		for t in &c.targets {
			if !target_ids.contains(&t.as_str()) {
				out.push(Problem::warning(
					format!("component:{}", c.id),
					format!("targets `{t}`, which the project does not define"),
				));
			}
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::backend::Severity;

	fn example() -> Project {
		let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lt-fizz");
		Project::open(&dir).expect("example opens")
	}

	#[test]
	fn the_examples_have_no_errors() {
		for name in ["lt-fizz", "planboard", "neutral"] {
			let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
				.join("../../examples")
				.join(name);
			let p = Project::open(&dir).unwrap();
			let errors: Vec<_> = check_project(&p)
				.into_iter()
				.filter(|p| p.severity == Severity::Error)
				.collect();
			assert!(errors.is_empty(), "{name}: {errors:?}");
		}
	}

	#[test]
	fn reports_a_missing_layout_and_hook() {
		let mut p = example();
		let flow = p.flows.get_mut("install").unwrap();
		flow.steps[0].layout = Some("nope".into());
		let sc = flow
			.steps
			.iter_mut()
			.find(|s| s.id == "sound-card")
			.unwrap();
		sc.on_enter = Some("scripts/sound-setup.lua#missing".into());
		let problems = check_project(&p);
		assert!(problems.iter().any(|x| x.message.contains("`nope`")));
		assert!(
			problems
				.iter()
				.any(|x| x.message.contains("does not define `missing`"))
		);
	}
}
