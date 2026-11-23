// The real backend: checks, dry runs and script tests through the installer engine.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::screens::{TreeCtx, prepare_tree, seed_meters};
use li_engine::{
	CondContext, DirPayload, InstallRequest, Machine, ProjectScripts, SimMachine, State, cond,
};
use li_model::{Layout, Node, Operation, Plan, Step, StepType, Target};
use li_render_tui::{Look, MapBindings, Screen, dump_with_controls};
use li_script::{Limits, Page, Script, ScriptContext, ScriptError};
use serde_json::{Value, json};

use crate::backend::{
	BackendError, HookInfo, HookRun, Preview, PreviewProduct, PreviewStep, Problem, Screenshot,
	ScriptReport, Severity, StudioBackend, TuiPage,
};
use crate::checks::check_project;
use crate::mock::HOOKS;
use crate::project::Project;

/// Cells a step's screen is drawn at.
const COLS: u16 = 80;
const ROWS: u16 = 24;

/// Codes the Studio's own checks already report, in their own wording.
const COVERED_BY_STUDIO: &[&str] = &[
	"duplicate-target",
	"duplicate-component",
	"unknown-target",
	"unknown-theme",
	"missing-source",
	"missing-script",
	"bad-flow",
	"unknown-layout",
];

/// The backend that runs the real engine.
///
/// `check` is the Studio's own checks plus the engine's (conditions, components named in
/// conditions, hooks, built-in actions), not the CLI's `DefaultChecker`. The engine reads the
/// project from disk, which the Studio keeps current by writing every change through.
#[derive(Debug, Default, Clone, Copy)]
pub struct EngineBackend;

fn engine_project(project: &Project) -> Result<li_engine::Project, BackendError> {
	let root = project.root.canonicalize().unwrap_or(project.root.clone());
	li_engine::Project::load_installer_dir(&root).map_err(|e| BackendError(e.to_string()))
}

/// The scope and step an engine location such as `flow install, step welcome` refers to.
fn scope_of(location: &str) -> (String, Option<String>) {
	if let Some(rest) = location.strip_prefix("flow ") {
		let mut parts = rest.split(", step ");
		let id = parts.next().unwrap_or(rest);
		return (format!("flow:{id}"), parts.next().map(str::to_owned));
	}
	if let Some((kind, id)) = location.split_once(' ')
		&& matches!(kind, "target" | "component" | "theme")
	{
		return (format!("{kind}:{id}"), None);
	}
	(location.to_owned(), None)
}

fn engine_problems(eng: &li_engine::Project) -> Vec<Problem> {
	li_engine::check(eng)
		.into_iter()
		.filter(|d| !COVERED_BY_STUDIO.contains(&d.code))
		.map(|d| {
			let (scope, step) = scope_of(&d.location);
			let p = match d.severity {
				li_engine::Severity::Error => Problem::error(scope, d.message),
				li_engine::Severity::Warning => Problem::warning(scope, d.message),
			};
			match step {
				Some(s) => p.at_step(s),
				None => p,
			}
		})
		.collect()
}

fn xml(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
}

/// Draws the layout as the `tui` renderer would, as a text dump set in an SVG.
fn screenshot(root: &Node, look: Look, bindings: &MapBindings) -> Screenshot {
	let layout = Layout {
		schema: None,
		version: 1,
		id: "preview".into(),
		root: root.clone(),
	};
	let screen = Screen::new(&layout, look);
	let text = dump_with_controls(&screen, bindings, COLS, ROWS);
	let lines: Vec<&str> = text.lines().collect();
	let (cell_w, cell_h) = (9usize, 18usize);
	let width = usize::from(COLS) * cell_w + 32;
	let height = lines.len() * cell_h + 32;
	let (bg, fg) = match look {
		Look::Retro => ("#0000aa", "#ffffff"),
		Look::Plain => ("#0c0c10", "#e0e0e0"),
	};
	let mut body = String::new();
	for (i, line) in lines.iter().enumerate() {
		body.push_str(&format!(
			"<text x=\"16\" y=\"{}\" xml:space=\"preserve\">{}</text>",
			24 + i * cell_h,
			xml(line)
		));
	}
	Screenshot {
		mime: "image/svg+xml".into(),
		encoding: "utf8".into(),
		data: format!(
			"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\"><rect width=\"{width}\" height=\"{height}\" fill=\"{bg}\"/><g font-family=\"monospace\" font-size=\"15\" fill=\"{fg}\">{body}</g></svg>"
		),
	}
}

/// The sample values a screen's `{placeholders}` take in a preview.
struct Sample<'a> {
	eng: &'a li_engine::Project,
	target: &'a Target,
	plan: &'a Plan,
	display_path: String,
}

impl Sample<'_> {
	fn vars(&self) -> BTreeMap<String, String> {
		let p = &self.eng.installer.product;
		let (os, arch) = self
			.target
			.id
			.split_once('-')
			.unwrap_or((&self.target.id, ""));
		let copies = self
			.plan
			.ops
			.iter()
			.filter(|o| matches!(o, Operation::Copy { .. }))
			.count();
		let file = self
			.plan
			.ops
			.iter()
			.find_map(|o| match o {
				Operation::Copy { to, .. } => Some(to.clone()),
				_ => None,
			})
			.unwrap_or_else(|| "data/level-03.lvl".into())
			.replace("{install}", &self.display_path);
		let pairs = [
			("product.id", p.id.clone()),
			("product.name", p.name.clone()),
			("product.publisher", p.publisher.clone()),
			(
				"product.version",
				self.eng.literal_version().unwrap_or("1.0.0").to_owned(),
			),
			("target.id", self.target.id.clone()),
			("target.renderer", self.target.renderer.clone()),
			("target.os", os.to_owned()),
			("target.arch", arch.to_owned()),
			("install.path", self.display_path.clone()),
			("install.size", "48 MB".into()),
			("system.free_space", "12.4 GB".into()),
			("system.free_space_after", "12.3 GB".into()),
			("progress.file", file),
			("progress.percent", "55".into()),
			(
				"plan.summary",
				format!("{} components · {copies} files · 48 MB", self.components()),
			),
			("sound.detected", "Sound Blaster".into()),
			("feed.latest", "1.1.0".into()),
			("receipt.installed", "1.0.0".into()),
			("feed.notes", "Fixes and new levels.".into()),
			("validate.problems", String::new()),
		];
		pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
	}

	fn components(&self) -> usize {
		self.eng
			.components
			.components
			.iter()
			.filter(|c| c.applies_to(&self.target.id))
			.count()
	}
}

/// How the target's platform writes the install folder, for display.
fn display_path(target: &Target, slug: &str) -> String {
	if target.id.starts_with("dos") || target.id.starts_with("win") {
		format!("C:\\{}", slug.to_uppercase())
	} else if target.id.starts_with("macos") {
		format!("/Applications/{slug}")
	} else {
		format!("~/.local/share/{slug}")
	}
}

/// Values as bindings: the sample strings first, then the page state over them.
fn bindings_of(vars: &BTreeMap<String, String>, state: &State) -> MapBindings {
	let mut b = MapBindings::new();
	for (k, v) in vars {
		b.0.insert(k.clone(), json!(v));
	}
	for (k, v) in state {
		b.0.insert(k.clone(), v.clone());
	}
	b
}

fn slug_of(eng: &li_engine::Project) -> String {
	eng.installer
		.product
		.id
		.rsplit('.')
		.next()
		.unwrap_or("app")
		.to_owned()
}

/// The sample machine's `BLASTER` variable, which the scripts and `builtin:detect-sound` read.
const SAMPLE_BLASTER: &str = "A220 I5 D1 T4";

fn script_env(target: &Target, install: &Path, version: &str) -> BTreeMap<String, String> {
	let (os, arch) = target.id.split_once('-').unwrap_or((&target.id, ""));
	BTreeMap::from([
		("os".to_owned(), os.to_owned()),
		("arch".to_owned(), arch.to_owned()),
		("target".to_owned(), target.id.clone()),
		("install".to_owned(), install.to_string_lossy().into_owned()),
		("version".to_owned(), version.to_owned()),
		// The sample machine has a Sound Blaster, as DOS drivers read it.
		("BLASTER".to_owned(), SAMPLE_BLASTER.to_owned()),
	])
}

/// The flow's steps as one target walks them: a step whose `when` is false is listed as
/// skipped and passed over, a branch takes its first condition that holds, and a page's
/// `detect` hook runs against the sample state.
#[allow(clippy::too_many_arguments)]
fn walk_install(
	eng: &li_engine::Project,
	target: &Target,
	selected: &[String],
	scripts: Option<&ProjectScripts>,
	sim: &SimMachine,
	install: &Path,
	sample: &Sample,
	root: &Path,
	problems: &mut Vec<Problem>,
) -> Vec<PreviewStep> {
	let Some(flow) = eng.flows.get("install") else {
		return Vec::new();
	};
	let look = if target.renderer == "tui-retro" {
		Look::Retro
	} else {
		Look::Plain
	};
	let vars = sample.vars();
	let mut state = State::new();
	state.insert("components".into(), json!(selected));
	state.insert("components.selected".into(), json!(selected));
	state.insert(
		"install.path".into(),
		json!(install.to_string_lossy().into_owned()),
	);
	state.insert("install.scope".into(), json!("user"));
	state.insert("licence.accepted".into(), json!("yes"));
	state.insert("finish.launch".into(), json!(["launch"]));
	let components: Vec<&li_model::Component> = eng
		.components
		.components
		.iter()
		.filter(|c| c.applies_to(&target.id))
		.collect();
	let (os, arch) = target.id.split_once('-').unwrap_or((&target.id, ""));
	let target_value = json!({
		"id": target.id,
		"renderer": target.renderer,
		"os": os,
		"arch": arch,
	});
	let system = li_engine::system_info(sim, install);
	let holds = |expr: &str, state: &State, step: &Step, problems: &mut Vec<Problem>| {
		let ctx = CondContext {
			target: target_value.clone(),
			components: state.get("components").cloned().unwrap_or(json!([])),
			state: Value::Object(state.clone()),
			system: system.clone(),
		};
		match cond::eval_bool(expr, &ctx) {
			Ok(v) => v,
			Err(e) => {
				problems.push(
					Problem::error("flow:install", format!("condition `{expr}`: {e}"))
						.at_step(&step.id),
				);
				false
			}
		}
	};

	let mut steps = Vec::new();
	let mut seen = BTreeSet::new();
	let mut cursor = flow.start_id().map(str::to_owned);
	while let Some(id) = cursor {
		if !seen.insert(id.clone()) {
			break;
		}
		let Some(step) = flow.step(&id) else { break };
		let skipped = match &step.when {
			Some(w) => !holds(w, &state, step, problems),
			None => false,
		};
		let mut screenshot_of = None;
		let mut tree_of = None;
		let mut shown_state = State::new();
		let mut cursor_next = step.next.clone();
		if !skipped {
			if let (Some(spec), Some(scripts)) = (&step.on_enter, scripts)
				&& let Some((path, "detect")) = spec.split_once('#')
				&& let Some(script) = scripts.get(path)
			{
				let mut page = Page {
					id: step.id.clone(),
					fields: state.clone(),
				};
				match script.detect(&mut page) {
					Ok(()) => state = page.fields,
					Err(e) => problems.push(
						Problem::error(format!("script:{path}"), format!("detect: {e}"))
							.at_step(&step.id),
					),
				}
			}
			if step.on_enter.as_deref() == Some("builtin:detect-sound") {
				// What the DOS runtime finds on the sample machine.
				let field =
					|tag| li_retro::sound::blaster_field(SAMPLE_BLASTER, tag).unwrap_or_default();
				state.insert("sound.card".into(), json!("sbpro"));
				state.insert("sound.port".into(), json!(field('A')));
				state.insert("sound.irq".into(), json!(field('I')));
				state.insert("sound.dma".into(), json!(field('D')));
				state.insert("sound.detected".into(), json!("Sound Blaster"));
			}
			if step.kind == StepType::Branch {
				for b in &step.branches {
					if holds(&b.when, &state, step, problems) {
						cursor_next = Some(b.next.clone());
						break;
					}
				}
			}
			if let Some(layout) = step.layout.as_deref().and_then(|l| {
				eng.screen_for(
					target,
					l,
					&step.id,
					&format!("{:?}", step.kind).to_lowercase(),
				)
			}) {
				let mut shown = state.clone();
				shown.insert("install.path".into(), json!(sample.display_path));
				let tree_in = |renderer: &str, st: &State| {
					let cx = TreeCtx {
						renderer,
						components: &components,
						state: st,
						root,
					};
					prepare_tree(&layout.root, &cx)
				};
				let tree = tree_in(&target.renderer, &shown);
				seed_meters(&tree, &mut shown);
				if target.renderer.starts_with("tui") {
					let text_tree = tree_in(li_render_tui::RENDERER_ID, &shown);
					screenshot_of = Some(screenshot(&text_tree, look, &bindings_of(&vars, &shown)));
				}
				tree_of = Some(tree);
				shown_state = shown;
			}
		}
		steps.push(PreviewStep {
			id: step.id.clone(),
			kind: format!("{:?}", step.kind).to_lowercase(),
			layout: step.layout.clone(),
			skipped,
			screenshot: screenshot_of,
			layout_tree: tree_of,
			state: shown_state,
		});
		if step.kind == StepType::Finish && !skipped {
			break;
		}
		cursor = cursor_next;
	}
	steps
}

impl StudioBackend for EngineBackend {
	fn check(&self, project: &Project) -> Result<Vec<Problem>, BackendError> {
		let mut out = check_project(project);
		out.extend(engine_problems(&engine_project(project)?));
		Ok(out)
	}

	fn run_preview(&self, project: &Project, target: &str) -> Result<Preview, BackendError> {
		let eng = engine_project(project)?;
		let t = eng
			.installer
			.targets
			.iter()
			.find(|t| t.full_id() == target)
			.ok_or_else(|| BackendError(format!("unknown target `{target}`")))?
			.clone();
		if !eng.flows.contains_key("install") {
			return Err(BackendError("the project has no install flow".into()));
		}
		let mut problems: Vec<Problem> = check_project(project)
			.into_iter()
			.filter(|p| {
				p.scope.starts_with("flow:install")
					|| p.scope == format!("target:{target}")
					|| p.scope.starts_with("layout:")
			})
			.collect();
		problems.extend(
			engine_problems(&eng)
				.into_iter()
				.filter(|p| p.scope.starts_with("flow:install") || p.scope.starts_with("target:")),
		);

		let sim = SimMachine::new();
		let install = PathBuf::from(format!("/opt/{}", slug_of(&eng)));
		let version = eng.literal_version().unwrap_or("0.0.0").to_owned();
		let selected =
			li_engine::select_components(&eng, &t.id, None).unwrap_or_else(|_| Vec::new());
		let scripts =
			match ProjectScripts::load(&eng, script_env(&t, &install, &version), sim.script_fs()) {
				Ok(s) => Some(s),
				Err(e) => {
					problems.push(Problem::error("script", e.to_string()));
					None
				}
			};
		let plan = match DirPayload::new(&eng.dir) {
			Ok(payload) => {
				let req = InstallRequest {
					target: t.id.clone(),
					install_dir: install.clone(),
					scope: eng.installer.defaults.scope,
					components: None,
					version: None,
					feed: None,
					channel: None,
				};
				li_engine::preview(&eng, &payload, &req, &sim).unwrap_or_else(|e| {
					problems.push(Problem::error(format!("target:{target}"), e.to_string()));
					Plan::default()
				})
			}
			Err(e) => {
				problems.push(Problem::error(format!("target:{target}"), e.to_string()));
				Plan::default()
			}
		};
		let sample = Sample {
			eng: &eng,
			target: &t,
			plan: &plan,
			display_path: display_path(&t, &slug_of(&eng)),
		};
		let steps = walk_install(
			&eng,
			&t,
			&selected,
			scripts.as_ref(),
			&sim,
			&install,
			&sample,
			&project.root,
			&mut problems,
		);
		let p = &eng.installer.product;
		Ok(Preview {
			target: target.to_owned(),
			renderer: t.renderer.clone(),
			theme: eng.theme_id_for(&t).map(str::to_owned),
			product: PreviewProduct {
				id: p.id.clone(),
				name: p.name.clone(),
				version: sample
					.vars()
					.remove("product.version")
					.unwrap_or_else(|| "1.0.0".into()),
				publisher: p.publisher.clone(),
			},
			vars: sample.vars(),
			plan,
			problems,
			steps,
		})
	}

	fn tui_page(
		&self,
		project: &Project,
		target: &str,
		page: &str,
		state: &serde_json::Map<String, Value>,
	) -> Result<TuiPage, BackendError> {
		let eng = engine_project(project)?;
		let t = eng
			.installer
			.targets
			.iter()
			.find(|t| t.full_id() == target)
			.ok_or_else(|| BackendError(format!("unknown target `{target}`")))?;
		if !t.renderer.starts_with("tui") {
			return Err(BackendError(format!(
				"`{target}` is not a terminal target, so it has no terminal screen"
			)));
		}
		// A flow step is drawn inside its theme's stage, as the rich preview draws it. A layout
		// id is drawn the same way as a `page` step, except a stage, which is drawn alone.
		let step = eng.flows.get("install").and_then(|f| f.step(page));
		let layout_id = step.and_then(|s| s.layout.as_deref()).unwrap_or(page);
		let (step_id, step_kind) = match step {
			Some(s) => (s.id.as_str(), format!("{:?}", s.kind).to_lowercase()),
			None => (layout_id, "page".to_owned()),
		};
		let layout = match layout_id.split_once('/') {
			Some((theme, id)) => {
				let page = eng
					.themes
					.get(theme)
					.and_then(|t| t.layouts.get(id))
					.ok_or_else(|| BackendError(format!("no layout for `{page}`")))?;
				match eng.stage_for(t) {
					Some(stage) if id != li_model::theme::STAGE_LAYOUT => {
						Layout::composed(stage, page, step_id, &step_kind)
					}
					_ => page.clone(),
				}
			}
			None => eng
				.screen_for(t, layout_id, step_id, &step_kind)
				.ok_or_else(|| BackendError(format!("no layout for `{page}`")))?,
		};
		let components: Vec<&li_model::Component> = eng
			.components
			.components
			.iter()
			.filter(|c| c.applies_to(&t.id))
			.collect();
		let cx = TreeCtx {
			renderer: li_render_tui::RENDERER_ID,
			components: &components,
			state,
			root: &project.root,
		};
		Ok(TuiPage {
			layout: Layout {
				schema: None,
				version: layout.version,
				id: layout.id.clone(),
				root: prepare_tree(&layout.root, &cx),
			},
			look: if t.renderer == "tui-retro" {
				Look::Retro
			} else {
				Look::Plain
			},
		})
	}

	fn test_script(
		&self,
		project: &Project,
		path: &str,
		source: &str,
		target: Option<&str>,
	) -> Result<ScriptReport, BackendError> {
		let scope = format!("script:{path}");
		let mut diagnostics = Vec::new();
		let sample = match target {
			Some(full_id) => sample_for(project, full_id)?,
			None => ScriptSample::fixed(),
		};
		let ctx = ScriptContext {
			env: sample.env.clone(),
			fs: SimMachine::new().script_fs(),
			net: None,
		};
		let no_hooks = |diagnostics: Vec<Problem>| ScriptReport {
			ok: false,
			hooks: HOOKS
				.iter()
				.map(|(n, w)| HookInfo {
					name: (*n).into(),
					when: (*w).into(),
					present: false,
				})
				.collect(),
			capabilities: Vec::new(),
			diagnostics,
			output: Vec::new(),
			target: target.map(str::to_owned),
			tests: Vec::new(),
		};
		let script = match Script::load(path, source, ctx, Limits::default()) {
			Ok(s) => s,
			Err(e) => {
				return Ok(no_hooks(vec![script_problem(&scope, None, &e)]));
			}
		};
		let hooks: Vec<HookInfo> = HOOKS
			.iter()
			.map(|(name, when)| HookInfo {
				name: (*name).into(),
				when: (*when).into(),
				present: script.has_hook(name),
			})
			.collect();
		if !hooks.iter().any(|h| h.present) {
			diagnostics.push(Problem::error(&scope, "the script defines no hook"));
		}
		let mut output = Vec::new();
		let mut tests: Vec<HookRun> = Vec::new();
		let mut page = Page {
			id: "test".into(),
			fields: json!({
				"install.path": sample.install,
				"components": sample.components,
			})
			.as_object()
			.cloned()
			.unwrap_or_default(),
		};
		let notes = |script: &Script| -> Vec<String> {
			script
				.take_ui()
				.into_iter()
				.map(|item| {
					format!(
						"ui {}: {}",
						match item.kind {
							li_script::UiKind::Note => "note",
							li_script::UiKind::Dialog => "dialog",
						},
						item.text
					)
				})
				.collect()
		};
		// Records one hook's result in the test list and in the flat output.
		let mut record = |tests: &mut Vec<HookRun>,
		                  hook: &str,
		                  ok: bool,
		                  summary: String,
		                  lines: Vec<String>| {
			output.push(format!("{hook}: {summary}"));
			output.extend(lines.iter().map(|l| format!("  {l}")));
			tests.push(HookRun {
				hook: hook.to_owned(),
				ok,
				summary,
				lines,
			});
		};

		if script.has_hook("detect") {
			let before = page.fields.clone();
			match script.detect(&mut page) {
				Ok(()) => {
					let changed: Vec<&String> = page
						.fields
						.iter()
						.filter(|(k, v)| before.get(*k) != Some(*v))
						.map(|(k, _)| k)
						.collect();
					let summary = format!(
						"ok, set {}",
						if changed.is_empty() {
							"nothing".to_owned()
						} else {
							changed
								.iter()
								.map(|k| k.as_str())
								.collect::<Vec<_>>()
								.join(", ")
						}
					);
					record(&mut tests, "detect", true, summary, notes(&script));
				}
				Err(e) => {
					let p = script_problem(&scope, Some("detect"), &e);
					record(
						&mut tests,
						"detect",
						false,
						p.message.clone(),
						notes(&script),
					);
					diagnostics.push(p);
				}
			}
		}
		if script.has_hook("validate") {
			match script.validate(&page) {
				Ok(found) => {
					let summary = if found.is_empty() {
						"ok, no problems".to_owned()
					} else {
						format!("{} problem(s) on the sample page", found.len())
					};
					let mut lines = found.clone();
					lines.extend(notes(&script));
					record(&mut tests, "validate", true, summary, lines);
				}
				Err(e) => {
					let p = script_problem(&scope, Some("validate"), &e);
					record(
						&mut tests,
						"validate",
						false,
						p.message.clone(),
						notes(&script),
					);
					diagnostics.push(p);
				}
			}
		}
		let plan = Plan {
			ops: vec![
				Operation::Mkdir {
					path: "{install}".into(),
				},
				Operation::ReceiptWrite {
					path: "{install}/.liminal/receipt.json".into(),
				},
			],
		};
		for (name, run) in [
			(
				"on_commit",
				Script::on_commit as fn(&Script, &Plan) -> Result<Vec<Operation>, ScriptError>,
			),
			("on_uninstall", Script::on_uninstall),
		] {
			if !script.has_hook(name) {
				continue;
			}
			match run(&script, &plan) {
				Ok(ops) => {
					let mut lines: Vec<String> = ops
						.iter()
						.map(|op| format!("plan.add {}", op_line(op)))
						.collect();
					lines.extend(notes(&script));
					record(
						&mut tests,
						name,
						true,
						format!("ok, adds {} operation(s)", ops.len()),
						lines,
					);
				}
				Err(e) => {
					let p = script_problem(&scope, Some(name), &e);
					record(&mut tests, name, false, p.message.clone(), notes(&script));
					diagnostics.push(p);
				}
			}
		}

		let capabilities = modules(&script);
		if capabilities.iter().any(|c| c == "li.net") {
			diagnostics.push(Problem::warning(
				&scope,
				"`net` is granted, but a preview never reaches the network",
			));
		}
		let ok = diagnostics.iter().all(|d| d.severity != Severity::Error);
		Ok(ScriptReport {
			ok,
			hooks,
			capabilities,
			diagnostics,
			output,
			target: target.map(str::to_owned),
			tests,
		})
	}
}

/// What a script test runs against: the machine's environment and the sample page.
struct ScriptSample {
	env: BTreeMap<String, String>,
	install: String,
	components: Vec<String>,
}

impl ScriptSample {
	/// The sample a test uses when no target is named.
	fn fixed() -> ScriptSample {
		let sim = SimMachine::new();
		ScriptSample {
			env: BTreeMap::from([
				("os".to_owned(), sim.os().to_owned()),
				("arch".to_owned(), sim.arch().to_owned()),
				("install".to_owned(), "/opt/app".to_owned()),
				("version".to_owned(), "0.0.0".to_owned()),
			]),
			install: "/opt/app".into(),
			components: vec!["main".into()],
		}
	}
}

/// The sample for one target, as a dry run of it would set up: the target's own `os` and `arch`,
/// the install folder, the version, and the components the target gets.
fn sample_for(project: &Project, full_id: &str) -> Result<ScriptSample, BackendError> {
	let eng = engine_project(project)?;
	let t = eng
		.installer
		.targets
		.iter()
		.find(|t| t.full_id() == full_id)
		.ok_or_else(|| BackendError(format!("unknown target `{full_id}`")))?
		.clone();
	let install = PathBuf::from(format!("/opt/{}", slug_of(&eng)));
	let version = eng.literal_version().unwrap_or("0.0.0").to_owned();
	let components = li_engine::select_components(&eng, &t.id, None).unwrap_or_default();
	Ok(ScriptSample {
		env: script_env(&t, &install, &version),
		install: install.to_string_lossy().into_owned(),
		components,
	})
}

/// The `li.*` modules the script's granted capabilities open.
fn modules(script: &Script) -> Vec<String> {
	let mut out: Vec<String> = script
		.capabilities()
		.iter()
		.map(|c| format!("li.{}", c.split('.').next().unwrap_or(c)))
		.collect();
	out.sort();
	out.dedup();
	out
}

fn script_problem(scope: &str, hook: Option<&str>, e: &ScriptError) -> Problem {
	let prefix = hook.map(|h| format!("{h}: ")).unwrap_or_default();
	match e {
		ScriptError::Denied(cap) => Problem::error(
			scope,
			format!(
				"{prefix}capability `{cap}` is denied; name it in a `-- capabilities:` header comment"
			),
		),
		other => Problem::error(scope, format!("{prefix}{other}")),
	}
}

fn op_line(op: &Operation) -> String {
	serde_json::to_string(op).unwrap_or_else(|_| format!("{op:?}"))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::project::{Doc, DocKey};

	fn open(name: &str) -> Project {
		Project::open(
			&Path::new(env!("CARGO_MANIFEST_DIR"))
				.join("../../examples")
				.join(name),
		)
		.unwrap()
	}

	fn step<'a>(p: &'a Preview, id: &str) -> &'a PreviewStep {
		p.steps.iter().find(|s| s.id == id).unwrap()
	}

	#[test]
	fn a_project_folder_not_named_installer_checks_and_previews() {
		let tmp = tempfile::tempdir().unwrap();
		let dir = tmp.path().join("my-product-files");
		crate::studio::testutil::copy_dir(
			&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lt-fizz/installer"),
			&dir,
		);
		let p = Project::open(&dir).unwrap();
		EngineBackend.check(&p).unwrap();
		let preview = EngineBackend.run_preview(&p, "dos-x86:tui-retro").unwrap();
		assert!(!preview.steps.is_empty());
	}

	#[test]
	fn fizz_skips_the_sound_card_unless_the_renderer_is_retro() {
		let p = open("lt-fizz");
		let retro = EngineBackend.run_preview(&p, "dos-x86:tui-retro").unwrap();
		let modern = EngineBackend.run_preview(&p, "linux-x86_64:tui").unwrap();
		assert!(!step(&retro, "sound-card").skipped);
		assert!(step(&retro, "sound-card").screenshot.is_some());
		assert!(step(&modern, "sound-card").skipped);
		assert!(step(&modern, "sound-card").screenshot.is_none());
		assert!(!retro.plan.ops.is_empty());
		assert!(matches!(
			retro.plan.ops.last(),
			Some(Operation::ReceiptWrite { .. })
		));
		assert_eq!(retro.steps.last().unwrap().kind, "finish");
		let shot = step(&retro, "welcome").screenshot.as_ref().unwrap();
		assert_eq!(shot.mime, "image/svg+xml");
		assert!(shot.data.contains("<text"));
		let errors: Vec<_> = retro
			.problems
			.iter()
			.filter(|p| p.severity == Severity::Error)
			.collect();
		assert!(errors.is_empty(), "{errors:?}");
	}

	fn find<'a>(n: &'a Node, kind: &str, out: &mut Vec<&'a Node>) {
		if n.kind == kind {
			out.push(n);
		}
		for c in &n.children {
			find(c, kind, out);
		}
	}

	#[test]
	fn a_preview_carries_what_the_ui_draws_for_the_chosen_target() {
		let p = open("lt-fizz");
		let win = EngineBackend
			.run_preview(&p, "windows-x86_64:win32-classic")
			.unwrap();
		assert_eq!(win.renderer, "win32-classic");
		assert_eq!(win.theme.as_deref(), Some("fizz-win95"));
		assert_eq!(win.product.name, "Lieutenant Fizz — Episode One");
		assert_eq!(win.vars["target.id"], "windows-x86_64");
		assert_eq!(win.vars["install.path"], "C:\\LT-FIZZ");
		assert!(win.steps.iter().all(|s| s.screenshot.is_none()));
		let comps = step(&win, "components");
		let mut lists = Vec::new();
		find(comps.layout_tree.as_ref().unwrap(), "checklist", &mut lists);
		let items = lists[0].props["items"].as_array().unwrap();
		assert_eq!(items[0]["id"], "game");
		assert_eq!(items[0]["checked"], json!(true));
		assert!(
			!items.iter().any(|i| i["id"] == "sound"),
			"sound drivers do not apply to windows-x86_64"
		);
		let copying = step(&win, "copying");
		assert_eq!(copying.state["engine.meter.total"], json!(0.55));
		assert!(copying.layout_tree.as_ref().unwrap().overrides.is_empty());
		let dos = EngineBackend.run_preview(&p, "dos-x86:tui-retro").unwrap();
		assert_eq!(dos.theme.as_deref(), Some("fizz-dos"));
		let sound = step(&dos, "sound-card");
		let mut radios = Vec::new();
		find(sound.layout_tree.as_ref().unwrap(), "radio", &mut radios);
		assert_eq!(radios[0].props["options"].as_array().unwrap().len(), 5);
		assert_eq!(sound.state["sound.card"], json!("sbpro"));
	}

	#[test]
	fn the_text_version_has_no_raw_placeholders() {
		let p = open("lt-fizz");
		for t in ["dos-x86:tui-retro", "linux-x86_64:tui"] {
			let pv = EngineBackend.run_preview(&p, t).unwrap();
			for st in pv.steps.iter().filter(|s| s.screenshot.is_some()) {
				let data = &st.screenshot.as_ref().unwrap().data;
				assert!(!data.contains("{product."), "{t}/{}", st.id);
				assert!(!data.contains("{install."), "{t}/{}", st.id);
				assert!(!data.contains("{plan."), "{t}/{}", st.id);
			}
			let welcome = step(&pv, "welcome").screenshot.as_ref().unwrap();
			assert!(welcome.data.contains("Lieutenant Fizz"), "{t}");
		}
	}

	#[test]
	fn the_detect_hook_runs_for_the_sound_card_page() {
		let p = open("lt-fizz");
		let pv = EngineBackend.run_preview(&p, "dos-x86:tui-retro").unwrap();
		assert!(pv.problems.iter().all(|x| !x.message.starts_with("detect")));
		// A project that still runs the Lua hook, on a target that runs scripts.
		let broken = {
			let dir = tempfile::tempdir().unwrap();
			copy_dir(&examples().join("lt-fizz"), dir.path());
			let flow = dir.path().join("installer/flows/install.flow.json");
			let src = std::fs::read_to_string(&flow).unwrap();
			let src = src
				.replace("builtin:detect-sound", "scripts/sound-setup.lua#detect")
				.replace("target.renderer in ['tui-retro', 'win16']", "true");
			std::fs::write(&flow, src).unwrap();
			let script = dir.path().join("installer/scripts/sound-setup.lua");
			let src = std::fs::read_to_string(&script).unwrap();
			std::fs::write(&script, src.replace("li.env.get", "li.net.get")).unwrap();
			let p = Project::open(&dir.path().join("installer")).unwrap();
			EngineBackend.run_preview(&p, "linux-x86_64:tui").unwrap()
		};
		assert!(
			broken
				.problems
				.iter()
				.any(|x| x.message.starts_with("detect:") && x.message.contains("not granted"))
		);
	}

	#[test]
	fn planboard_previews_a_plan_for_each_target() {
		let p = open("planboard");
		for t in ["linux-x86_64:canvas", "linux-x86_64:tui"] {
			let pv = EngineBackend.run_preview(&p, t).unwrap();
			assert!(pv.plan.ops.len() > 3, "{t}");
			assert!(pv.steps.iter().all(|s| !s.skipped), "{t}");
			assert!(pv.steps.iter().any(|s| s.layout_tree.is_some()), "{t}");
			assert_eq!(
				pv.steps.iter().any(|s| s.screenshot.is_some()),
				t.ends_with(":tui"),
				"{t}: only the tui renderer has a text version"
			);
		}
		assert!(EngineBackend.run_preview(&p, "nope:tui").is_err());
	}

	#[test]
	fn a_branch_takes_the_first_condition_that_holds() {
		let dir = tempfile::tempdir().unwrap();
		copy_dir(
			&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/planboard"),
			dir.path(),
		);
		let flow_path = dir.path().join("installer/flows/install.flow.json");
		let mut flow: Value =
			serde_json::from_str(&std::fs::read_to_string(&flow_path).unwrap()).unwrap();
		let steps = flow["steps"].as_array_mut().unwrap();
		let i = steps.iter().position(|s| s["id"] == "folder").unwrap();
		steps[i]["next"] = json!("route");
		steps.push(json!({
			"id": "route", "type": "branch",
			"branches": [{"when": "target.renderer == 'tui'", "next": "components"}],
			"next": "ready"
		}));
		std::fs::write(&flow_path, flow.to_string()).unwrap();
		let p = Project::open(&dir.path().join("installer")).unwrap();
		let tui = EngineBackend.run_preview(&p, "linux-x86_64:tui").unwrap();
		let canvas = EngineBackend
			.run_preview(&p, "linux-x86_64:canvas")
			.unwrap();
		let ids = |pv: &Preview| pv.steps.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
		assert!(ids(&tui).contains(&"components".to_owned()));
		assert!(!ids(&canvas).contains(&"components".to_owned()));
	}

	fn examples() -> PathBuf {
		Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
	}

	fn copy_dir(from: &Path, to: &Path) {
		for e in std::fs::read_dir(from).unwrap() {
			let e = e.unwrap();
			let dest = to.join(e.file_name());
			if e.file_type().unwrap().is_dir() {
				std::fs::create_dir_all(&dest).unwrap();
				copy_dir(&e.path(), &dest);
			} else {
				std::fs::copy(e.path(), dest).unwrap();
			}
		}
	}

	#[test]
	fn check_reports_engine_findings_the_studio_does_not() {
		let dir = tempfile::tempdir().unwrap();
		copy_dir(&examples().join("lt-fizz"), dir.path());
		let mut p = Project::open(&dir.path().join("installer")).unwrap();
		assert!(
			EngineBackend
				.check(&p)
				.unwrap()
				.iter()
				.all(|x| x.severity != Severity::Error)
		);
		let mut flow = p.flows["install"].clone();
		flow.steps[0].when = Some("target.renderer in [".into());
		p.put(&DocKey::Flow("install".into()), Some(Doc::Flow(flow)))
			.unwrap();
		let found = EngineBackend.check(&p).unwrap();
		assert!(
			found
				.iter()
				.any(|x| x.scope == "flow:install" && x.step.as_deref() == Some("welcome")),
			"{found:?}"
		);
	}

	#[test]
	fn the_example_script_passes_its_test() {
		let p = open("lt-fizz");
		let src = p.scripts["scripts/sound-setup.lua"].clone();
		let r = EngineBackend
			.test_script(&p, "scripts/sound-setup.lua", &src, None)
			.unwrap();
		assert!(r.ok, "{:?}", r.diagnostics);
		assert_eq!(
			r.hooks.iter().filter(|h| h.present).count(),
			2,
			"{:?}",
			r.hooks
		);
		assert!(r.capabilities.contains(&"li.env".to_owned()));
		assert!(r.output.iter().any(|l| l.starts_with("detect: ok")));
	}

	#[test]
	fn a_denied_capability_is_reported() {
		let p = open("lt-fizz");
		let src = "-- capabilities: ui\nfunction detect(page)\n\tpage.fields.x = li.env.get('HOME')\nend\n";
		let r = EngineBackend
			.test_script(&p, "scripts/x.lua", src, None)
			.unwrap();
		assert!(!r.ok);
		assert!(
			r.diagnostics
				.iter()
				.any(|d| d.message.contains("capability `env` is denied")),
			"{:?}",
			r.diagnostics
		);
	}

	#[test]
	fn plan_writes_and_load_errors_are_reported() {
		let p = open("lt-fizz");
		let src = "-- capabilities: plan, fs.write\nfunction on_commit(plan)\n\tplan:add({op = 'mkdir', path = '{install}/saves'})\nend\n";
		let r = EngineBackend
			.test_script(&p, "scripts/x.lua", src, None)
			.unwrap();
		assert!(r.ok, "{:?}", r.diagnostics);
		assert!(
			r.output
				.iter()
				.any(|l| l.contains("plan.add") && l.contains("saves"))
		);
		let denied = "-- capabilities: plan\nfunction on_commit(plan)\n\tplan:add({op = 'mkdir', path = '{install}/saves'})\nend\n";
		assert!(
			!EngineBackend
				.test_script(&p, "scripts/x.lua", denied, None)
				.unwrap()
				.ok
		);
		let unknown = EngineBackend
			.test_script(&p, "scripts/x.lua", "-- capabilities: teleport\n", None)
			.unwrap();
		assert!(!unknown.ok);
		let none = EngineBackend
			.test_script(&p, "scripts/x.lua", "-- nothing", None)
			.unwrap();
		assert!(!none.ok);
	}

	#[test]
	fn a_target_changes_what_the_script_sees() {
		let p = open("lt-fizz");
		let src = "-- capabilities: env\nfunction detect(page)\n\tpage.fields.seen = li.env.os() .. '/' .. li.env.arch() .. '/' .. li.env.get('target')\n\tpage.fields.count = #page.fields.components\nend\n";
		let dos = EngineBackend
			.test_script(&p, "scripts/x.lua", src, Some("dos-x86:tui-retro"))
			.unwrap();
		let win = EngineBackend
			.test_script(
				&p,
				"scripts/x.lua",
				src,
				Some("windows-x86_64:win32-classic"),
			)
			.unwrap();
		assert!(
			dos.ok && win.ok,
			"{:?} {:?}",
			dos.diagnostics,
			win.diagnostics
		);
		assert_eq!(dos.target.as_deref(), Some("dos-x86:tui-retro"));
		assert_eq!(dos.tests.len(), 1);
		assert_eq!(dos.tests[0].hook, "detect");
		assert!(dos.tests[0].ok);
		assert!(dos.tests[0].summary.contains("seen"), "{:?}", dos.tests[0]);
		assert!(dos.tests[0].summary.contains("count"), "{:?}", dos.tests[0]);
	}

	#[test]
	fn the_sample_is_the_targets_own() {
		let p = open("lt-fizz");
		let probe = "-- capabilities: env\nfunction validate(page)\n\treturn { li.env.os() .. ' ' .. li.env.arch() .. ' ' .. #page.fields.components .. ' ' .. page.fields['install.path'] }\nend\n";
		let line = |target: &str| {
			let r = EngineBackend
				.test_script(&p, "scripts/x.lua", probe, Some(target))
				.unwrap();
			r.tests[0].lines[0].clone()
		};
		let dos = line("dos-x86:tui-retro");
		let linux = line("linux-x86_64:tui");
		assert!(dos.starts_with("dos x86 "), "{dos}");
		assert!(linux.starts_with("linux x86_64 "), "{linux}");
		assert!(dos.ends_with("/opt/lt-fizz"), "{dos}");
		assert_ne!(dos, linux);
	}

	#[test]
	fn an_unknown_target_is_refused() {
		let p = open("lt-fizz");
		let err = EngineBackend
			.test_script(
				&p,
				"scripts/x.lua",
				"function detect(page) end",
				Some("nope:tui"),
			)
			.unwrap_err();
		assert!(err.0.contains("unknown target"), "{err:?}");
	}

	#[test]
	fn each_hook_reports_its_own_result() {
		let p = open("lt-fizz");
		let src = "-- capabilities: ui\nfunction detect(page)\n\tli.ui.note('hello')\nend\nfunction on_commit(plan)\n\tlocal x = li.env.get('a')\nend\n";
		let r = EngineBackend
			.test_script(&p, "scripts/x.lua", src, None)
			.unwrap();
		assert!(!r.ok);
		let by: Vec<(&str, bool)> = r.tests.iter().map(|t| (t.hook.as_str(), t.ok)).collect();
		assert_eq!(by, [("detect", true), ("on_commit", false)]);
		assert!(r.tests[0].lines.iter().any(|l| l == "ui note: hello"));
		assert!(r.tests[1].summary.contains("capability `env` is denied"));
		assert_eq!(r.target, None);
	}
}
