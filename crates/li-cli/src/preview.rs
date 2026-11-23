// `liminst preview`: a dry run that prints the plan and draws each step as text.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeSet;

use li_model::{Flow, Layout, Node, Step, StepType, Target};
use li_render_tui::{Bindings, Look, MapBindings, Screen, dump_with_controls};
use serde_json::json;

use crate::plan::{Planner, describe};
use crate::project::Project;
use crate::util::to_json_tabs;
use crate::{CliError, CliResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Only {
	All,
	Plan,
	Screens,
}

pub struct PreviewOptions {
	pub flow: String,
	pub only: Only,
	pub look: Look,
	pub width: u16,
	pub height: u16,
	pub json: bool,
}

/// The steps a dry run visits: from the start step along `next`, taking a branch step's
/// fallback `next` (or its first branch when it has none), until a finish step.
pub fn walk(flow: &Flow) -> Vec<&Step> {
	let mut out = Vec::new();
	let mut seen = BTreeSet::new();
	let mut cur = flow.start_id().map(str::to_owned);
	while let Some(id) = cur {
		let Some(step) = flow.step(&id) else { break };
		if !seen.insert(id) {
			break;
		}
		out.push(step);
		if step.kind == StepType::Finish {
			break;
		}
		cur = step
			.next
			.clone()
			.or_else(|| step.branches.first().map(|b| b.next.clone()));
	}
	out
}

/// Sample values so meters, paths and billboards show something.
fn seed(node: &Node, b: &mut MapBindings, slug: &str) {
	let node = node.for_renderer(li_render_tui::RENDERER_ID);
	if let Some(bind) = &node.bind {
		match node.kind.as_str() {
			"meter" | "progress" => b.set(bind, json!(0.5)),
			"path" => b.set(bind, json!(format!("~/.local/share/{slug}"))),
			_ => {}
		}
	}
	for c in &node.children {
		seed(c, b, slug);
	}
}

pub fn screen_text(layout: &Layout, look: Look, w: u16, h: u16, slug: &str) -> String {
	let screen = Screen::new(layout, look);
	let mut b = MapBindings::new();
	seed(&layout.root, &mut b, slug);
	dump_with_controls(&screen, &b, w, h)
}

pub fn preview(
	project: &Project,
	target: &Target,
	planner: &dyn Planner,
	o: &PreviewOptions,
) -> CliResult<String> {
	let flow = project.flows.get(&o.flow).ok_or_else(|| {
		let known: Vec<&str> = project.flows.keys().map(String::as_str).collect();
		CliError::new(format!(
			"no flow named `{}` (the project has: {})",
			o.flow,
			known.join(", ")
		))
	})?;
	let plan = planner.plan(project, target)?;
	let slug = project
		.installer
		.product
		.id
		.rsplit('.')
		.next()
		.unwrap_or("app")
		.to_owned();
	let steps = walk(flow);
	if o.json {
		let screens: Vec<_> = steps
			.iter()
			.map(|s| {
				let layout = s
					.layout
					.as_deref()
					.and_then(|l| project.layout_for(target, l));
				json!({
					"id": s.id,
					"type": s.kind,
					"layout": s.layout,
					"screen": layout.map(|l| screen_text(l, o.look, o.width, o.height, &slug)),
				})
			})
			.collect();
		let v = json!({
			"target": target.full_id(),
			"flow": o.flow,
			"steps": if o.only == Only::Plan { vec![] } else { screens },
			"plan": if o.only == Only::Screens { None } else { Some(&plan) },
		});
		return Ok(to_json_tabs(&v));
	}
	let mut out = format!(
		"Preview of `{}` for {} ({} flow)\n",
		project.installer.product.name,
		target.full_id(),
		o.flow
	);
	if !matches!(target.renderer.as_str(), "tui" | "tui-retro") {
		out.push_str(&format!(
			"Note: the `{}` renderer is not drawn here; steps are shown with the tui renderer.\n",
			target.renderer
		));
	}
	if o.only != Only::Plan {
		for (i, step) in steps.iter().enumerate() {
			out.push_str(&format!(
				"\n== Step {} of {}: {} ({:?}) ==\n",
				i + 1,
				steps.len(),
				step.id,
				step.kind
			));
			match &step.layout {
				None => out.push_str("(no page)\n"),
				Some(l) => match project.layout_for(target, l) {
					Some(layout) => {
						out.push_str(&screen_text(layout, o.look, o.width, o.height, &slug))
					}
					None => out.push_str(&format!("(layout `{l}` not found)\n")),
				},
			}
		}
	}
	if o.only != Only::Screens {
		out.push_str(&format!("\n== Plan ({} operations) ==\n", plan.ops.len()));
		for op in &plan.ops {
			out.push_str(&describe(op));
			out.push('\n');
		}
	}
	Ok(out)
}
