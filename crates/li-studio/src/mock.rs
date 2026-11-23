// A stand-in engine: enough of a dry run and a script test to drive the Studio.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_model::{Node, Operation, Plan, StepType};

use crate::backend::{
	BackendError, HookInfo, Preview, PreviewStep, Problem, Screenshot, ScriptReport, Severity,
	StudioBackend,
};
use crate::checks::check_project;
use crate::project::Project;
use crate::resolve::resolve_for_renderer;

/// Hooks from the spec's hook table.
pub const HOOKS: &[(&str, &str)] = &[
	("detect", "A page opens"),
	("validate", "Before Next"),
	("on_commit", "Plan is built"),
	("on_uninstall", "Uninstall plan is built"),
];

/// Backend that builds a plan from the project files and checks scripts by scanning their text.
#[derive(Debug, Default, Clone, Copy)]
pub struct MockBackend;

/// Evaluates the two condition forms the mock understands; anything else counts as true.
fn eval_when(cond: &str, renderer: &str) -> bool {
	let c = cond.trim();
	if let Some(rest) = c.strip_prefix("target.renderer in [") {
		let list = rest.trim_end_matches(']');
		return list
			.split(',')
			.map(|s| s.trim().trim_matches('\''))
			.any(|s| s == renderer);
	}
	if let Some(rest) = c.strip_prefix("target.renderer ==") {
		return rest.trim().trim_matches('\'') == renderer;
	}
	true
}

fn xml(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
}

fn lines(node: &Node, depth: usize, out: &mut Vec<(usize, String)>) {
	let prop = |k: &str| {
		node.props
			.get(k)
			.and_then(|v| v.as_str())
			.map(str::to_owned)
	};
	match node.kind.as_str() {
		"text" => {
			if let Some(t) = prop("text").or_else(|| prop("source").map(|s| format!("[{s}]"))) {
				out.push((depth, t));
			}
		}
		"button" => out.push((depth, format!("[ {} ]", prop("label").unwrap_or_default()))),
		"meter" | "progress" => out.push((
			depth,
			format!("{} ▓▓▓▓░░░░", prop("label").unwrap_or_default()),
		)),
		"checklist" | "radio" | "field" | "path" => out.push((
			depth,
			format!("{}: ({})", prop("label").unwrap_or_default(), node.kind),
		)),
		"window" | "dialog" | "backdrop" => {
			if let Some(t) = prop("title") {
				out.push((depth, format!("== {t} ==")));
			}
		}
		other => {
			if let Some(l) = prop("label") {
				out.push((depth, format!("<{other}> {l}")));
			}
		}
	}
	for c in &node.children {
		lines(c, depth + 1, out);
	}
}

fn render_svg(root: &Node, renderer: &str) -> Screenshot {
	let tree = resolve_for_renderer(root, renderer);
	let mut ls = Vec::new();
	lines(&tree, 0, &mut ls);
	let height = 24 + ls.len() * 20;
	let mut body = String::new();
	for (i, (depth, text)) in ls.iter().enumerate() {
		body.push_str(&format!(
			"<text x=\"{}\" y=\"{}\" font-family=\"monospace\" font-size=\"13\" fill=\"#e0e0e0\">{}</text>",
			16 + depth * 12,
			28 + i * 20,
			xml(text)
		));
	}
	Screenshot {
		mime: "image/svg+xml".into(),
		encoding: "utf8".into(),
		data: format!(
			"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"640\" height=\"{height}\"><rect width=\"640\" height=\"{height}\" fill=\"#0c0c10\"/>{body}</svg>"
		),
	}
}

fn build_plan(project: &Project, target_id: &str) -> Plan {
	let mut ops = vec![Operation::Mkdir {
		path: "{install}".into(),
	}];
	for c in &project.components.components {
		if !c.applies_to(target_id) {
			continue;
		}
		for rule in &c.files {
			let dir = rule.to.trim_end_matches('/');
			if dir != "{install}" {
				ops.push(Operation::Mkdir { path: dir.into() });
			}
			for file in project.expand_source(&rule.from, target_id) {
				let name = file
					.file_name()
					.and_then(|n| n.to_str())
					.unwrap_or_default();
				let from = file
					.strip_prefix(&project.root)
					.unwrap_or(&file)
					.display()
					.to_string();
				ops.push(Operation::Copy {
					from,
					to: format!("{dir}/{name}"),
					sha256: None,
				});
			}
		}
		for s in &c.shortcut {
			ops.push(Operation::Shortcut {
				name: s.name.clone(),
				target: s.target.clone(),
				place: s.place.clone(),
			});
		}
		for f in &c.file_types {
			ops.push(Operation::FileType {
				extension: f.extension.clone(),
				name: f.name.clone(),
				open_with: f.open_with.clone(),
			});
		}
		for scheme in &c.url_schemes {
			ops.push(Operation::UrlScheme {
				scheme: scheme.clone(),
				open_with: "{install}".into(),
			});
		}
		for entry in &c.path {
			ops.push(Operation::PathAdd {
				entry: entry.clone(),
			});
		}
	}
	ops.push(Operation::OsRegister {
		product: project.installer.product.id.clone(),
		version: match &project.installer.product.version {
			li_model::project::VersionSource::Literal(v) => v.clone(),
			li_model::project::VersionSource::From { from } => format!("from {from}"),
		},
	});
	ops.push(Operation::ReceiptWrite {
		path: "{install}/.liminal/receipt.json".into(),
	});
	Plan { ops }
}

impl StudioBackend for MockBackend {
	fn check(&self, project: &Project) -> Result<Vec<Problem>, BackendError> {
		Ok(check_project(project))
	}

	fn run_preview(&self, project: &Project, target: &str) -> Result<Preview, BackendError> {
		let t = project
			.installer
			.targets
			.iter()
			.find(|t| t.full_id() == target)
			.ok_or_else(|| BackendError(format!("unknown target `{target}`")))?;
		let flow = project
			.flows
			.get("install")
			.ok_or_else(|| BackendError("the project has no install flow".into()))?;
		let mut steps = Vec::new();
		let mut seen = std::collections::BTreeSet::new();
		let mut cursor = flow.start_id().map(str::to_owned);
		while let Some(id) = cursor {
			if !seen.insert(id.clone()) {
				break;
			}
			let Some(step) = flow.step(&id) else { break };
			let skipped = step
				.when
				.as_deref()
				.is_some_and(|w| !eval_when(w, &t.renderer));
			let screenshot = match (&step.layout, skipped) {
				(Some(l), false) => project
					.find_layout(l)
					.map(|layout| render_svg(&layout.root, &t.renderer)),
				_ => None,
			};
			steps.push(PreviewStep {
				id: step.id.clone(),
				kind: format!("{:?}", step.kind).to_lowercase(),
				layout: step.layout.clone(),
				skipped,
				screenshot,
				layout_tree: match (&step.layout, skipped) {
					(Some(l), false) => project
						.find_layout(l)
						.map(|layout| resolve_for_renderer(&layout.root, &t.renderer)),
					_ => None,
				},
				state: Default::default(),
			});
			cursor = if step.kind == StepType::Finish {
				None
			} else {
				// Branch conditions need runtime state, so the dry run takes the fallback.
				step.next.clone()
			};
		}
		let problems = check_project(project)
			.into_iter()
			.filter(|p| {
				p.scope.starts_with("flow:install")
					|| p.scope == format!("target:{target}")
					|| p.scope.starts_with("layout:")
			})
			.collect();
		let product = &project.installer.product;
		Ok(Preview {
			target: target.to_owned(),
			renderer: t.renderer.clone(),
			theme: t.theme.clone().or(project.installer.defaults.theme.clone()),
			product: crate::backend::PreviewProduct {
				id: product.id.clone(),
				name: product.name.clone(),
				version: "1.0.0".into(),
				publisher: product.publisher.clone(),
			},
			vars: Default::default(),
			plan: build_plan(project, &t.id),
			problems,
			steps,
		})
	}

	fn test_script(
		&self,
		_project: &Project,
		path: &str,
		source: &str,
		target: Option<&str>,
	) -> Result<ScriptReport, BackendError> {
		let hooks: Vec<HookInfo> = HOOKS
			.iter()
			.map(|(name, when)| HookInfo {
				name: (*name).into(),
				when: (*when).into(),
				present: source.contains(&format!("function {name}(")),
			})
			.collect();
		let mut capabilities: Vec<String> = Vec::new();
		for part in source.split("require").skip(1) {
			let rest = part.trim_start_matches(['(', ' ']);
			if let Some(rest) = rest.strip_prefix(['"', '\''])
				&& let Some(end) = rest.find(['"', '\''])
				&& rest[..end].starts_with("li.")
				&& !capabilities.contains(&rest[..end].to_owned())
			{
				capabilities.push(rest[..end].to_owned());
			}
		}
		let scope = format!("script:{path}");
		let mut diagnostics = Vec::new();
		if !hooks.iter().any(|h| h.present) {
			diagnostics.push(Problem::error(&scope, "the script defines no hook"));
		}
		for cap in &capabilities {
			if !matches!(
				cap.as_str(),
				"li.env"
					| "li.fs" | "li.ui"
					| "li.plan" | "li.net"
					| "li.retro.sound"
					| "li.retro.video"
			) {
				diagnostics.push(Problem::error(&scope, format!("unknown module `{cap}`")));
			}
		}
		if capabilities.iter().any(|c| c == "li.net") {
			diagnostics.push(Problem::warning(
				&scope,
				"`li.net` needs the `net` capability",
			));
		}
		let output = hooks
			.iter()
			.filter(|h| h.present)
			.map(|h| format!("{}: ok (mock run)", h.name))
			.collect();
		let ok = diagnostics.iter().all(|d| d.severity != Severity::Error);
		Ok(ScriptReport {
			ok,
			hooks,
			capabilities,
			diagnostics,
			output,
			target: target.map(str::to_owned),
			tests: Vec::new(),
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::Path;

	fn fizz() -> Project {
		Project::open(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lt-fizz"))
			.unwrap()
	}

	#[test]
	fn preview_skips_the_sound_card_for_modern_renderers() {
		let p = fizz();
		let dos = MockBackend.run_preview(&p, "dos-x86:tui-retro").unwrap();
		let win = MockBackend
			.run_preview(&p, "windows-x86_64:win32-classic")
			.unwrap();
		let skipped = |pv: &Preview| {
			pv.steps
				.iter()
				.find(|s| s.id == "sound-card")
				.unwrap()
				.skipped
		};
		assert!(!skipped(&dos));
		assert!(skipped(&win));
		assert!(dos.steps.iter().any(|s| s.screenshot.is_some()));
		assert_eq!(dos.steps.last().unwrap().kind, "finish");
	}

	#[test]
	fn plan_ends_with_the_receipt_and_respects_targets() {
		let p = fizz();
		let dos = MockBackend.run_preview(&p, "dos-x86:tui-retro").unwrap();
		let win = MockBackend
			.run_preview(&p, "windows-x86_64:win32-classic")
			.unwrap();
		assert!(matches!(
			dos.plan.ops.last(),
			Some(Operation::ReceiptWrite { .. })
		));
		let copies = |pv: &Preview, needle: &str| {
			pv.plan
				.ops
				.iter()
				.any(|o| matches!(o, Operation::Copy { to, .. } if to.contains(needle)))
		};
		assert!(copies(&dos, "SOUND/"));
		assert!(!copies(&win, "SOUND/"));
	}

	#[test]
	fn unknown_target_is_an_error() {
		assert!(MockBackend.run_preview(&fizz(), "nope:x").is_err());
	}

	#[test]
	fn script_report_lists_hooks_and_capabilities() {
		let p = fizz();
		let src = p.scripts["scripts/sound-setup.lua"].clone();
		let r = MockBackend
			.test_script(&p, "scripts/sound-setup.lua", &src, None)
			.unwrap();
		assert!(r.ok, "{:?}", r.diagnostics);
		let present: Vec<_> = r
			.hooks
			.iter()
			.filter(|h| h.present)
			.map(|h| h.name.as_str())
			.collect();
		assert_eq!(present, ["detect", "validate"]);
		let with_module = MockBackend
			.test_script(
				&p,
				"scripts/y.lua",
				"local s = require(\"li.retro.sound\")\nfunction detect(page) end",
				None,
			)
			.unwrap();
		assert!(
			with_module
				.capabilities
				.contains(&"li.retro.sound".to_string())
		);
		let bad = MockBackend
			.test_script(&p, "scripts/x.lua", "-- nothing", None)
			.unwrap();
		assert!(!bad.ok);
	}
}
