// Compiles an installer project into the DOS runtime's `SETUP.LPK`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`compile`] turns a project and a `tui-retro` target into a [`li_retro::model::Project`] and
//! the contents of its files:
//!
//! - the `install` and `uninstall` flows become steps with bytecode conditions ([`expr`]);
//! - each layout the flows use is drawn once by li-render-tui's Retro look at 80 by 25 and
//!   flattened into cells plus the widgets that need live values ([`screens`]);
//! - the target's component files become 8.3 destinations, each with its size and
//!   CRC-32 ([`payload`]); [`Compiled::lpk`] packs the project and the files into `SETUP.LPK`;
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
use li_lpk::targets;
use li_lpk::writer::{FileSpec, Options, pack};
use li_model::{Flow, StepType};
use li_retro::model::{self, Builtin, Kind, Op};
use li_retro::pack::{PROJECT_FILE, Pack};

pub use payload::{dos_rel, rooted_rel, win_rel};
pub use screens::{Context, ScreenKey, tui_layout};

/// Which runtime the project is compiled for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
	/// `SETUP.EXE` for DOS: 80 by 25 text, 8.3 names, `tui-retro`.
	Dos,
	/// `SETUP.EXE` for Windows 95 and later: native controls, long names, `win32-classic`.
	Win32,
	/// `SETUP.EXE` for Windows 3.1: native controls, 8.3 names, `win16`. The runtime is Rust for a
	/// 16-bit code segment, so its pack uses a smaller window (see `WIN16_CHUNK_LOG2`).
	Win16,
}

impl Flavour {
	/// What the runtime is called in a message.
	pub fn name(self) -> &'static str {
		match self {
			Flavour::Dos => "DOS",
			Flavour::Win32 => "Windows",
			Flavour::Win16 => "Windows 3.1",
		}
	}

	/// The renderer a target must use.
	pub fn renderer(self) -> &'static str {
		match self {
			Flavour::Dos => "tui-retro",
			Flavour::Win32 => "win32-classic",
			Flavour::Win16 => "win16",
		}
	}

	fn pack_target(self) -> u8 {
		match self {
			Flavour::Dos => targets::DOS,
			Flavour::Win32 => targets::WIN32,
			Flavour::Win16 => targets::WIN16,
		}
	}

	/// The pack's chunk size as a power of two: the decoder's window. A Windows 3.1 program has one
	/// 64 KB data segment for its heap, stack and this window, so its chunks are 4 KiB.
	pub fn chunk_log2(self) -> u8 {
		match self {
			Flavour::Win16 => WIN16_CHUNK_LOG2,
			_ => Options::default().chunk_log2,
		}
	}
}

/// The chunk size of a Windows 3.1 pack as a power of two (4 KiB).
pub const WIN16_CHUNK_LOG2: u8 = 12;

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
	pub flavour: Flavour,
	pub project: model::Project,
	/// The files' contents, in the order of `project.files`.
	pub data: Vec<Vec<u8>>,
	pub warnings: Vec<String>,
}

impl Compiled {
	/// The `SETUP.LPK` file as one volume: the compiled project first, then the files in install
	/// order, in chunks of 32 KiB that are stored or deflated.
	pub fn lpk(&self) -> Result<Vec<u8>, CompileError> {
		let mut set = self.lpk_set(&[u32::MAX])?;
		Ok(set.remove(0))
	}

	/// The pack as a set of volumes, each at most the matching size in `volume_sizes` (the last
	/// size repeats). One volume when the sizes allow it.
	pub fn lpk_set(&self, volume_sizes: &[u32]) -> Result<Vec<Vec<u8>>, CompileError> {
		let project = self
			.project
			.encode()
			.map_err(|e| CompileError::Message(e.to_string()))?;
		let mut specs = vec![FileSpec {
			attr: 0x02,
			targets: self.flavour.pack_target(),
			..FileSpec::new(PROJECT_FILE, project)
		}];
		for (f, data) in self.project.files.iter().zip(&self.data) {
			specs.push(FileSpec {
				targets: self.flavour.pack_target(),
				long_name: if f.stored == Pack::name_of(&f.dest) {
					String::new()
				} else {
					Pack::name_of(&f.dest)
				},
				..FileSpec::new(f.stored.clone(), data.clone())
			});
		}
		let opts = Options {
			volume_sizes: volume_sizes.to_vec(),
			chunk_log2: self.flavour.chunk_log2(),
			..Options::default()
		};
		let packed = pack(&specs, &opts).map_err(|e| CompileError::Message(e.to_string()))?;
		Ok(packed.volumes)
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
	compile_for(project, target_id, version, Flavour::Dos)
}

/// Compiles `project` for the target `target_id`, whose renderer must be `win32-classic`: the
/// same compiled project, with long file names, Start menu shortcuts and the native window's
/// pages in place of the text screens.
pub fn compile_win32(
	project: &Project,
	target_id: &str,
	version: &str,
) -> Result<Compiled, CompileError> {
	compile_for(project, target_id, version, Flavour::Win32)
}

/// Compiles `project` for the target `target_id`, whose renderer must be `win16`: DOS's 8.3 names
/// and receipt with the pages as widgets for Windows controls, packed for a 64 KB data segment.
pub fn compile_win16(
	project: &Project,
	target_id: &str,
	version: &str,
) -> Result<Compiled, CompileError> {
	compile_for(project, target_id, version, Flavour::Win16)
}

/// Compiles for any of the runtimes.
pub fn compile_for(
	project: &Project,
	target_id: &str,
	version: &str,
	flavour: Flavour,
) -> Result<Compiled, CompileError> {
	let target = project
		.target(target_id)
		.ok_or_else(|| CompileError::Message(format!("the project has no target `{target_id}`")))?;
	if target.renderer != flavour.renderer() {
		return err(format!(
			"target `{target_id}` uses the `{}` renderer; this runtime draws `{}`",
			target.renderer,
			flavour.renderer()
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
	if let Some(c) = components.iter().find(|c| c.required && !c.default) {
		return err(format!(
			"component `{}` is required, so it cannot start unticked (`default = false`)",
			c.id
		));
	}
	let rcomps: Vec<model::Component> = components
		.iter()
		.map(|c| model::Component {
			id: c.id.clone(),
			name: c.name.clone(),
			required: c.required,
			default: c.default,
		})
		.collect();

	let (files, data, mut ops, mut owners) =
		payload::build(project, target, &components, flavour, &mut warnings)?;

	// Flows and the screens their steps show.
	let mut screen_keys: Vec<screens::ScreenKey> = Vec::new();
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
			&mut screen_keys,
			flavour,
			&mut warnings,
		)?);
	}
	for (id, flow) in &project.flows {
		if !FLOWS.contains(&id.as_str()) {
			let _ = flow;
			warnings.push(format!(
				"flow `{id}` is not used by the {} runtime, which runs `install` and `uninstall`",
				flavour.name()
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
	for (i, key) in screen_keys.iter().enumerate() {
		// A layout drawn for more than one step keeps its own id on the first and `layout@step` after.
		let id = if screen_keys[..i].iter().any(|k| k.layout == key.layout) {
			format!("{}@{}", key.layout, key.step)
		} else {
			key.layout.clone()
		};
		rscreens.push(screens::compile(&ctx, key, &id, &mut cx, &mut warnings)?);
	}
	let uses_sound = flavour != Flavour::Win32
		&& flows
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
			owners.push(model::NO_OWNER);
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
		// Format 6 only for a project that needs it, so the others stay readable by the DOS and Windows 3.1 runtimes.
		format: if ops.iter().any(model::Op::needs_format_6) {
			model::FORMAT
		} else {
			model::FORMAT_BASE
		},
		id: project.installer.product.id.clone(),
		name: project.installer.product.name.clone(),
		publisher: project.installer.product.publisher.clone(),
		version: version.to_owned(),
		target: target.full_id(),
		default_dir: match flavour {
			Flavour::Dos | Flavour::Win16 => format!("C:\\{slug}"),
			Flavour::Win32 => format!(
				"%PROGRAMFILES%\\{}",
				project
					.installer
					.product
					.name
					.chars()
					.filter(|c| !"\\/:*?\"<>|".contains(*c))
					.collect::<String>()
					.trim()
			),
		},
		theme: model::Theme::default(),
		strings: cx.into_table(),
		components: rcomps,
		flows,
		screens: rscreens,
		files,
		ops,
		owners,
	};
	out.validate()
		.map_err(|e| CompileError::Message(e.to_string()))?;
	Ok(Compiled {
		flavour,
		project: out,
		data,
		warnings,
	})
}

fn builtin_of(
	spec: &str,
	location: &str,
	flavour: Flavour,
	warnings: &mut Vec<String>,
) -> Result<Builtin, CompileError> {
	match spec.strip_prefix("builtin:") {
		Some("detect-sound") if flavour == Flavour::Win32 => {
			warnings.push(format!(
				"{location}: `builtin:detect-sound` probes DOS sound hardware and is skipped on Windows"
			));
			Ok(Builtin::None)
		}
		Some("detect-sound") => Ok(Builtin::DetectSound),
		Some("check-disk-space") => Ok(Builtin::CheckDiskSpace),
		Some(other) => {
			warnings.push(format!(
				"{location}: the built-in `{other}` has no DOS implementation; it is skipped"
			));
			Ok(Builtin::None)
		}
		None => err(format!(
			"{location}: `{spec}` is a script hook, and the {} runtime does not run Lua scripts; use a `builtin:` action (detect-sound or check-disk-space) or remove the hook",
			flavour.name()
		)),
	}
}

fn compile_flow(
	project: &Project,
	target: &li_model::Target,
	flow: &Flow,
	cx: &mut expr::Strings,
	screen_ids: &mut Vec<screens::ScreenKey>,
	flavour: Flavour,
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
			Some(spec) => builtin_of(spec, &loc, flavour, warnings)?,
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
				let key = screens::ScreenKey {
					layout: l.clone(),
					step: s.id.clone(),
					kind: format!("{:?}", s.kind).to_lowercase(),
				};
				let i = match screen_ids.iter().position(|x| *x == key) {
					Some(i) => i,
					None => {
						screen_ids.push(key);
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
