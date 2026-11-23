// Compiles an installer project into the DOS runtime's `SETUP.DAT`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`compile`] turns a project and a `tui-retro` target into a [`li_retro::model::Project`] and
//! its payload:
//!
//! - the `install` and `uninstall` flows become steps with bytecode conditions ([`expr`]);
//! - each layout the flows use is drawn once by li-render-tui's Retro look at 80 by 25 and
//!   flattened into cells plus the widgets that need live values ([`screens`]);
//! - the target's component files become 8.3 destinations, each stored or deflated, with its
//!   CRC-32 ([`payload`]);
//! - shortcuts become launcher batch files, and the answers the sound page collects become a
//!   `SETUP.INI`.
//!
//! What the DOS runtime cannot do is reported as [`Compiled::warnings`] or, when the result
//! would be wrong, as a [`CompileError`]: Lua script hooks, file types, URL schemes, `PATH`
//! entries, names that are not 8.3.

mod expr;
mod payload;
mod screens;

use li_engine::Project;
use li_model::{Flow, StepType};
use li_retro::model::{self, Builtin, Kind, Op};

pub use payload::dos_rel;
pub use screens::{Context, tui_layout};

/// What stops a compile.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
	#[error("{0}")]
	Message(String),
}

pub(crate) fn err<T>(m: impl Into<String>) -> Result<T, CompileError> {
	Err(CompileError::Message(m.into()))
}

/// The result of a compile.
pub struct Compiled {
	pub project: model::Project,
	/// The payload bytes the `FileEntry` offsets point into.
	pub payload: Vec<u8>,
	pub warnings: Vec<String>,
}

impl Compiled {
	/// The `SETUP.DAT` file.
	pub fn dat(&self) -> Result<Vec<u8>, CompileError> {
		let project = self
			.project
			.encode()
			.map_err(|e| CompileError::Message(e.to_string()))?;
		Ok(model::dat::write(&project, &self.payload))
	}

	/// The bytes the install needs on disk, counting every component.
	pub fn install_bytes(&self) -> u64 {
		self.project.files.iter().map(|f| u64::from(f.size)).sum()
	}
}

/// The flows the DOS runtime runs.
const FLOWS: &[&str] = &["install", "uninstall"];

/// Compiles `project` for the target `target_id`, whose renderer must be `tui-retro`.
pub fn compile(
	project: &Project,
	target_id: &str,
	version: &str,
) -> Result<Compiled, CompileError> {
	let target = project
		.target(target_id)
		.ok_or_else(|| CompileError::Message(format!("the project has no target `{target_id}`")))?;
	if target.renderer != "tui-retro" {
		return err(format!(
			"target `{target_id}` uses the `{}` renderer; the DOS runtime draws `tui-retro`",
			target.renderer
		));
	}
	let mut warnings = Vec::new();
	let mut cx = expr::Strings::default();

	let components: Vec<&li_model::Component> = project
		.components
		.components
		.iter()
		.filter(|c| c.applies_to(&target.id))
		.collect();
	let rcomps: Vec<model::Component> = components
		.iter()
		.map(|c| model::Component {
			id: c.id.clone(),
			name: c.name.clone(),
			required: c.required,
		})
		.collect();

	let (files, payload_bytes, mut ops) =
		payload::build(project, target, &components, &mut warnings)?;

	// Flows and the screens their steps show.
	let mut screen_ids: Vec<String> = Vec::new();
	let mut flows = Vec::new();
	for id in FLOWS {
		let Some(flow) = project.flows.get(*id) else {
			continue;
		};
		flows.push(compile_flow(
			project,
			target,
			flow,
			&mut cx,
			&mut screen_ids,
			&mut warnings,
		)?);
	}
	for (id, flow) in &project.flows {
		if !FLOWS.contains(&id.as_str()) {
			let _ = flow;
			warnings.push(format!(
				"flow `{id}` is not used by the DOS runtime, which runs `install` and `uninstall`"
			));
		}
	}
	if !flows.iter().any(|f| f.id == "install") {
		return err("the project has no `install` flow");
	}

	let ctx = screens::Context {
		project,
		target,
		components: &components,
		version,
	};
	let mut rscreens = Vec::new();
	for id in &screen_ids {
		rscreens.push(screens::compile(&ctx, id, &mut cx, &mut warnings)?);
	}
	let uses_sound = flows
		.iter()
		.any(|f| f.steps.iter().any(|s| s.builtin == Builtin::DetectSound));
	if uses_sound {
		for key in ["card", "port", "irq", "dma"] {
			ops.push(Op::WriteIni {
				path: "SETUP.INI".into(),
				section: "sound".into(),
				key: key.into(),
				value: format!("{{sound.{key}}}"),
				when_set: "sound.card".into(),
			});
		}
	}

	let id_tail = project
		.installer
		.product
		.id
		.rsplit('.')
		.next()
		.unwrap_or("APP");
	let mut slug: String = id_tail
		.chars()
		.filter(char::is_ascii_alphanumeric)
		.collect::<String>()
		.to_ascii_uppercase();
	slug.truncate(8);
	if slug.is_empty() {
		slug = "APP".into();
	}
	let out = model::Project {
		format: model::FORMAT,
		id: project.installer.product.id.clone(),
		name: project.installer.product.name.clone(),
		publisher: project.installer.product.publisher.clone(),
		version: version.to_owned(),
		target: target.full_id(),
		default_dir: format!("C:\\{slug}"),
		theme: model::Theme::default(),
		strings: cx.into_table(),
		components: rcomps,
		flows,
		screens: rscreens,
		files,
		ops,
	};
	out.validate()
		.map_err(|e| CompileError::Message(e.to_string()))?;
	Ok(Compiled {
		project: out,
		payload: payload_bytes,
		warnings,
	})
}

fn builtin_of(
	spec: &str,
	location: &str,
	warnings: &mut Vec<String>,
) -> Result<Builtin, CompileError> {
	match spec.strip_prefix("builtin:") {
		Some("detect-sound") => Ok(Builtin::DetectSound),
		Some("check-disk-space") => Ok(Builtin::CheckDiskSpace),
		Some(other) => {
			warnings.push(format!(
				"{location}: the built-in `{other}` has no DOS implementation; it is skipped"
			));
			Ok(Builtin::None)
		}
		None => err(format!(
			"{location}: `{spec}` is a script hook, and the DOS runtime does not run Lua scripts; use a `builtin:` action (detect-sound or check-disk-space) or remove the hook"
		)),
	}
}

fn compile_flow(
	project: &Project,
	target: &li_model::Target,
	flow: &Flow,
	cx: &mut expr::Strings,
	screen_ids: &mut Vec<String>,
	warnings: &mut Vec<String>,
) -> Result<model::Flow, CompileError> {
	let index = |id: &str| flow.steps.iter().position(|s| s.id == id).map(|i| i as u16);
	let start = flow
		.start_id()
		.and_then(index)
		.ok_or_else(|| CompileError::Message(format!("flow `{}` has no start step", flow.id)))?;
	let mut steps = Vec::new();
	for s in &flow.steps {
		let loc = format!("flow {}, step {}", flow.id, s.id);
		let when = match &s.when {
			Some(w) => cx.compile(w, &loc)?,
			None => Vec::new(),
		};
		let mut branches = Vec::new();
		for b in &s.branches {
			let next = index(&b.next).ok_or_else(|| {
				CompileError::Message(format!("{loc}: unknown step `{}`", b.next))
			})?;
			branches.push((cx.compile(&b.when, &loc)?, next));
		}
		let next = match &s.next {
			Some(n) => Some(
				index(n)
					.ok_or_else(|| CompileError::Message(format!("{loc}: unknown step `{n}`")))?,
			),
			None => None,
		};
		let builtin = match &s.on_enter {
			Some(spec) => builtin_of(spec, &loc, warnings)?,
			None => Builtin::None,
		};
		let screen = match &s.layout {
			Some(l) => {
				if project.layout_for(target, l).is_none() {
					return err(format!(
						"{loc}: layout `{l}` was not found for target `{}`",
						target.id
					));
				}
				let i = match screen_ids.iter().position(|x| x == l) {
					Some(i) => i,
					None => {
						screen_ids.push(l.clone());
						screen_ids.len() - 1
					}
				};
				Some(i as u16)
			}
			None => None,
		};
		steps.push(model::Step {
			id: s.id.clone(),
			kind: match s.kind {
				StepType::Page => Kind::Page,
				StepType::Action => Kind::Action,
				StepType::Branch => Kind::Branch,
				StepType::Commit => Kind::Commit,
				StepType::Finish => Kind::Finish,
			},
			screen,
			when,
			builtin,
			next,
			branches,
		});
	}
	Ok(model::Flow {
		id: flow.id.clone(),
		start,
		steps,
	})
}
