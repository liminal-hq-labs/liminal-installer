// The `liminal-engine` argument parser and lifecycle commands.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use li_engine::bundle::{bundled_target, bundled_version, extract_project, pinned_key};
use li_engine::{
	CommitFn, EngineError, FlowEnv, FlowOutcome, FlowRunner, InstallRequest, LaunchOutcome,
	LipPayload, Machine, Progress, Project, ProjectScripts, RealMachine, RunOptions, State, Ui,
};
use li_model::{Layout, Receipt, Scope, Target};
use li_pack::Lip;
use li_render_tui::Look;
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::stash;
use crate::ui::{ConsoleUi, TuiUi};
use crate::{RunnerError, RunnerResult, feedsrc};

#[derive(Debug, Parser)]
#[command(
	name = "liminal-engine",
	version,
	about = "Install, update, modify, repair, uninstall and launch a product from its bundled project",
	after_help = "Exit codes: 0 success, 1 failure (changes rolled back), 2 bad arguments, 3 cancelled."
)]
pub struct Cli {
	/// Answer every page from `--answer` values instead of drawing it (the default when there
	/// is no terminal)
	#[arg(long, global = true)]
	pub headless: bool,
	/// A flow answer, such as `install_dir=/opt/app` or `components=app,sound` (repeatable)
	#[arg(long = "answer", value_name = "KEY=VALUE", global = true)]
	pub answers: Vec<String>,
	/// The install directory (default: where the `.run` extracted, or where this runner lives)
	#[arg(long, alias = "prefix", value_name = "DIR", global = true)]
	pub install_dir: Option<PathBuf>,
	/// How to draw pages (default: `retro` for `tui-retro` targets, else `plain`)
	#[arg(long, value_enum, global = true)]
	pub look: Option<LookArg>,
	#[command(subcommand)]
	pub command: Command,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LookArg {
	Retro,
	Plain,
}

#[derive(Debug, Subcommand)]
pub enum Command {
	/// Run the install flow
	Install(InstallArgs),
	/// Update to the release a signed feed points at
	Update(UpdateArgs),
	/// Add or remove components
	Modify(ModifyArgs),
	/// Restore files that are missing or changed
	Repair,
	/// Undo the install and remove the receipt
	Uninstall,
	/// Start the installed product
	Launch(LaunchArgs),
}

#[derive(Debug, Args)]
pub struct InstallArgs {
	/// The folder the `.run` extracted to; it holds `payload.lip`
	#[arg(long, value_name = "DIR")]
	pub from: Option<PathBuf>,
	/// A `.lip` to install from, instead of `<from>/payload.lip`
	#[arg(long, value_name = "FILE")]
	pub lip: Option<PathBuf>,
	/// Install only these components, plus the required ones (repeatable)
	#[arg(long = "component", value_name = "ID")]
	pub components: Vec<String>,
	/// The feed to remember for later updates
	#[arg(long, value_name = "URL|PATH")]
	pub feed: Option<String>,
	/// The channel to remember with the feed
	#[arg(long, value_name = "NAME")]
	pub channel: Option<String>,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
	/// The feed: a URL, a `feed.json` or the folder that holds it (default: the receipt's)
	#[arg(long, value_name = "URL|PATH")]
	pub feed: Option<String>,
	/// Verify the feed with this public key instead of the one pinned at install
	#[arg(long, value_name = "FILE")]
	pub feed_key: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ModifyArgs {
	/// The components to have installed afterwards (repeatable; default: those installed now)
	#[arg(long = "component", value_name = "ID")]
	pub components: Vec<String>,
}

#[derive(Debug, Args)]
pub struct LaunchArgs {
	/// A feed to check for a newer release first
	#[arg(long, value_name = "URL|PATH")]
	pub feed: Option<String>,
	/// Verify the feed with this public key instead of the one pinned at install
	#[arg(long, value_name = "FILE")]
	pub feed_key: Option<PathBuf>,
}

/// Parses `key=value` answers. A value is read as JSON when it parses (`true`, `3`, `["a"]`)
/// and as text otherwise.
pub fn parse_answers(answers: &[String]) -> RunnerResult<State> {
	let mut state = State::new();
	for a in answers {
		let (k, v) = a
			.split_once('=')
			.filter(|(k, _)| !k.is_empty())
			.ok_or_else(|| RunnerError::new(format!("`{a}` is not an answer; use KEY=VALUE")))?;
		let value = serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.to_owned()));
		state.insert(k.to_owned(), value);
	}
	Ok(state)
}

/// The components a state selects: a list of ids, or one comma-separated string.
pub fn components_from(state: &State) -> Option<Vec<String>> {
	match state.get("components")? {
		Value::Array(a) => Some(
			a.iter()
				.filter_map(|v| v.as_str().map(str::to_owned))
				.collect(),
		),
		Value::String(s) => Some(
			s.split(',')
				.map(str::trim)
				.filter(|s| !s.is_empty())
				.map(str::to_owned)
				.collect(),
		),
		_ => None,
	}
}

fn absolute(p: &Path) -> PathBuf {
	std::path::absolute(p).unwrap_or_else(|_| p.to_owned())
}

fn load_project(lip: &Lip) -> RunnerResult<(TempDir, Project)> {
	let tmp = tempfile::tempdir().map_err(|e| RunnerError::new(e.to_string()))?;
	extract_project(lip, tmp.path())?;
	let project = Project::load(tmp.path())?;
	Ok((tmp, project))
}

fn script_env(
	m: &dyn Machine,
	target: &str,
	install: &Path,
	version: &str,
) -> BTreeMap<String, String> {
	BTreeMap::from([
		("os".to_owned(), m.os().to_owned()),
		("arch".to_owned(), m.arch().to_owned()),
		("target".to_owned(), target.to_owned()),
		("install".to_owned(), install.to_string_lossy().into_owned()),
		("version".to_owned(), version.to_owned()),
	])
}

/// Everything a flow needs besides the work its commit step does.
struct Drive<'a> {
	project: &'a Project,
	target: &'a Target,
	scripts: &'a ProjectScripts,
	env: FlowEnv,
}

impl<'a> Drive<'a> {
	fn new(
		project: &'a Project,
		target: &'a Target,
		scripts: &'a ProjectScripts,
		m: &dyn Machine,
		install_dir: &Path,
	) -> Self {
		Drive {
			project,
			target,
			scripts,
			env: FlowEnv {
				target: json!({
					"id": target.id,
					"renderer": target.renderer,
					"os": m.os(),
					"arch": m.arch(),
				}),
				system: li_engine::system_info(m, install_dir),
			},
		}
	}

	fn layouts(&self, flow_id: &str) -> BTreeMap<String, Layout> {
		let mut out = BTreeMap::new();
		if let Some(flow) = self.project.flows.get(flow_id) {
			for step in &flow.steps {
				let Some(id) = step.layout.as_deref() else {
					continue;
				};
				let kind = format!("{:?}", step.kind).to_lowercase();
				if let Some(l) = self.project.screen_for(self.target, id, &step.id, &kind) {
					out.insert(id.to_owned(), l);
				}
			}
		}
		out
	}

	/// Walks the flow named `flow_id`, handing its pages to `ui`; `work` runs at the commit
	/// step. A project with no such flow just runs `work`, reporting progress to `ui`.
	fn run(
		&self,
		flow_id: &str,
		state: State,
		ui: &mut dyn Ui,
		work: &mut dyn FnMut(&State, &mut RunOptions<'_>) -> Result<(), EngineError>,
	) -> RunnerResult<()> {
		let Some(flow) = self.project.flows.get(flow_id) else {
			let mut report = |p: &Progress| ui.progress(p);
			let mut opts = RunOptions {
				cancel: None,
				on_progress: Some(&mut report),
			};
			return Ok(work(&state, &mut opts)?);
		};
		let layouts = self.layouts(flow_id);
		let mut runner = FlowRunner::new(flow, self.env.clone())
			.with_layouts(&layouts)
			.with_scripts(self.scripts)
			.with_state(state);
		let mut commit: Box<CommitFn<'_>> = Box::new(|st, report| {
			let mut opts = RunOptions {
				cancel: None,
				on_progress: Some(report),
			};
			work(st, &mut opts)
		});
		match runner.run(ui, &mut *commit)? {
			FlowOutcome::Finished => Ok(()),
			FlowOutcome::Cancelled => Err(RunnerError::Cancelled),
		}
	}
}

fn use_tui(cli: &Cli) -> bool {
	!cli.headless && std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

fn make_ui(cli: &Cli, target: &Target) -> RunnerResult<Box<dyn Ui>> {
	if use_tui(cli) {
		let look = match cli.look {
			Some(LookArg::Retro) => Look::Retro,
			Some(LookArg::Plain) => Look::Plain,
			None if target.renderer == "tui-retro" => Look::Retro,
			None => Look::Plain,
		};
		Ok(Box::new(TuiUi::new(look)))
	} else {
		Ok(Box::new(ConsoleUi::new(parse_answers(&cli.answers)?)))
	}
}

fn detect_target(project: &Project, m: &dyn Machine) -> Option<String> {
	let id = format!("{}-{}", m.os(), m.arch());
	project.target(&id).map(|t| t.id.clone()).or_else(|| {
		match project.installer.targets.as_slice() {
			[only] => Some(only.id.clone()),
			_ => None,
		}
	})
}

fn default_from() -> Option<PathBuf> {
	let exe = std::env::current_exe().ok()?;
	let from = exe.parent()?.parent()?.to_path_buf();
	from.join(stash::PAYLOAD).is_file().then_some(from)
}

fn plain_target(full: &str) -> &str {
	full.split(':').next().unwrap_or(full)
}

fn scope_of(project: &Project) -> Scope {
	match project.installer.defaults.scope {
		Scope::Ask => Scope::User,
		s => s,
	}
}

fn install(cli: &Cli, a: &InstallArgs) -> RunnerResult<()> {
	let from = a.from.clone().map(|p| absolute(&p)).or_else(default_from);
	let lip_path = a
		.lip
		.clone()
		.or_else(|| from.as_ref().map(|f| f.join(stash::PAYLOAD)))
		.ok_or_else(|| RunnerError::new("nothing to install; pass --from DIR or --lip FILE"))?;
	let bytes = std::fs::read(&lip_path)
		.map_err(|e| RunnerError::new(format!("{}: {e}", lip_path.display())))?;
	let lip = Lip::parse(&bytes)?;
	let (_tmp, project) = load_project(&lip)?;
	let mut m = RealMachine::from_environment();
	let target_id = bundled_target(&lip)
		.or_else(|| detect_target(&project, &m))
		.ok_or_else(|| RunnerError::new("the payload does not say which target it is for"))?;
	let target = project
		.target(&target_id)
		.ok_or_else(|| RunnerError::new(format!("the project has no target `{target_id}`")))?
		.clone();
	let version = bundled_version(&lip);
	let key = pinned_key(&lip)?;
	let default_dir = cli
		.install_dir
		.clone()
		.or_else(|| from.clone())
		.map(|p| absolute(&p))
		.ok_or_else(|| RunnerError::new("say where to install with --install-dir DIR"))?;

	let mut state = State::new();
	state.insert("install_dir".into(), json!(default_dir.to_string_lossy()));
	if !a.components.is_empty() {
		state.insert("components".into(), json!(a.components));
	}
	let env = script_env(
		&m,
		&target.id,
		&default_dir,
		version.as_deref().unwrap_or("0.0.0"),
	);
	let scripts = ProjectScripts::load(&project, env, m.script_fs())?;
	let drive = Drive::new(&project, &target, &scripts, &m, &default_dir);
	let mut ui = make_ui(cli, &target)?;
	let payload = LipPayload(lip.clone());
	let feed = a.feed.clone();
	let channel = a
		.channel
		.clone()
		.or_else(|| feed.as_ref().map(|_| "stable".to_owned()));
	let mut done: Option<(Receipt, PathBuf)> = None;
	let mut work = |st: &State, opts: &mut RunOptions<'_>| -> Result<(), EngineError> {
		let dir = st
			.get("install_dir")
			.and_then(Value::as_str)
			.map_or_else(|| default_dir.clone(), |s| absolute(Path::new(s)));
		let req = InstallRequest {
			target: target.id.clone(),
			install_dir: dir.clone(),
			scope: scope_of(&project),
			components: components_from(st),
			version: version.clone(),
			feed: feed.clone(),
			channel: channel.clone(),
		};
		done = Some((
			li_engine::install(&mut m, &project, &payload, &req, opts)?,
			dir,
		));
		Ok(())
	};
	let flow = project
		.flows
		.contains_key("install")
		.then_some("install")
		.ok_or_else(|| RunnerError::new("the project has no install flow"))?;
	drive.run(flow, state, &mut *ui, &mut work)?;
	let (receipt, dir) =
		done.ok_or_else(|| RunnerError::new("the install flow ended without installing"))?;
	stash::save(&dir, &bytes, key.as_ref())?;
	if let Some(from) = &from {
		stash::clean_bootstrap(from, &dir);
	}
	drop(ui);
	println!(
		"Installed {} {} to {}",
		receipt.product,
		receipt.installed,
		dir.display()
	);
	Ok(())
}

/// An existing install, opened from the runner's stash.
struct Installed {
	dir: PathBuf,
	lip: Lip,
	_tmp: TempDir,
	project: Project,
	receipt: Receipt,
	target: Target,
}

fn open(cli: &Cli, m: &dyn Machine) -> RunnerResult<Installed> {
	let dir = cli
		.install_dir
		.clone()
		.map(|p| absolute(&p))
		.or_else(stash::install_dir_of_current_exe)
		.ok_or_else(|| RunnerError::new("say which install with --install-dir DIR"))?;
	let receipt = li_engine::read_receipt(m, &dir)?;
	let bytes = stash::read(&dir, stash::PAYLOAD)?;
	let lip = Lip::parse(&bytes)?;
	let (tmp, project) = load_project(&lip)?;
	let target = project
		.target(plain_target(&receipt.target))
		.ok_or_else(|| RunnerError::new(format!("the project has no target `{}`", receipt.target)))?
		.clone();
	Ok(Installed {
		dir,
		lip,
		_tmp: tmp,
		project,
		receipt,
		target,
	})
}

fn feed_key(inst: &Installed, file: Option<&Path>) -> RunnerResult<li_pack::VerifyingKey> {
	if let Some(f) = file {
		return Ok(li_pack::read_public_key(f)?);
	}
	stash::pinned_key(&inst.dir)?.ok_or_else(|| {
		RunnerError::new(
			"this install pins no feed key (set `signing.feed_key` in installer.toml) so the feed cannot be trusted; pass --feed-key FILE to choose one",
		)
	})
}

fn update(cli: &Cli, a: &UpdateArgs) -> RunnerResult<()> {
	let mut m = RealMachine::from_environment();
	let inst = open(cli, &m)?;
	if !li_engine::tauri::updates::installer_owns(&inst.receipt) {
		println!(
			"{} is updated by the app itself (updates mode `{}`); the installer has nothing to do",
			inst.receipt.product,
			li_engine::tauri::updates::mode_of(&inst.receipt).as_str()
		);
		return Ok(());
	}
	let spec = a
		.feed
		.clone()
		.or_else(|| inst.receipt.feed.clone())
		.ok_or_else(|| RunnerError::new("no feed to update from; pass --feed URL|PATH"))?;
	let key = feed_key(&inst, a.feed_key.as_deref())?;
	let (feed, feed_json) = feedsrc::load_feed(&spec, &key, stash::last_feed(&inst.dir).as_ref())?;
	let Some(info) = li_engine::find_update(&inst.receipt, &feed) else {
		println!(
			"{} {} is up to date",
			inst.receipt.product, inst.receipt.installed
		);
		stash::write(&inst.dir, stash::FEED_SEEN, &feed_json)?;
		return Ok(());
	};
	let loc = feedsrc::artefact_location(&feedsrc::feed_location(&spec), &info.artefact.url);
	let bytes = feedsrc::fetch(&loc)?;
	li_pack::verify_artefact(&bytes, &info.artefact)?;
	let new_lip = Lip::parse(&bytes)?;
	let (_tmp, project) = load_project(&new_lip)?;
	let target = project
		.target(plain_target(&inst.receipt.target))
		.ok_or_else(|| RunnerError::new("the new release no longer has this target"))?
		.clone();
	let env = script_env(&m, &target.id, &inst.dir, &info.version);
	let scripts = ProjectScripts::load(&project, env, m.script_fs())?;
	let drive = Drive::new(&project, &target, &scripts, &m, &inst.dir);
	let mut ui = make_ui(cli, &target)?;
	let mut state = State::new();
	state.insert("components".into(), json!(inst.receipt.components));
	// What the `builtin:check-feed` step finds, for the flow's branch and the offer page.
	state.insert(
		"update".into(),
		json!({ "available": true, "version": info.version, "current": inst.receipt.installed }),
	);
	let mut updated: Option<Receipt> = None;
	let mut work = |_: &State, opts: &mut RunOptions<'_>| -> Result<(), EngineError> {
		updated = Some(li_engine::update(
			&mut m, &project, &inst.dir, &feed, &bytes, opts,
		)?);
		Ok(())
	};
	drive.run("update", state, &mut *ui, &mut work)?;
	let receipt =
		updated.ok_or_else(|| RunnerError::new("the update flow ended without updating"))?;
	stash::write(&inst.dir, stash::PAYLOAD, &bytes)?;
	stash::write(&inst.dir, stash::FEED_SEEN, &feed_json)?;
	drop(ui);
	println!(
		"Updated {} from {} to {}",
		receipt.product, inst.receipt.installed, receipt.installed
	);
	Ok(())
}

fn modify(cli: &Cli, a: &ModifyArgs) -> RunnerResult<()> {
	let mut m = RealMachine::from_environment();
	let inst = open(cli, &m)?;
	let env = script_env(&m, &inst.target.id, &inst.dir, &inst.receipt.installed);
	let scripts = ProjectScripts::load(&inst.project, env, m.script_fs())?;
	let drive = Drive::new(&inst.project, &inst.target, &scripts, &m, &inst.dir);
	let mut ui = make_ui(cli, &inst.target)?;
	let mut state = State::new();
	let wanted = if a.components.is_empty() {
		inst.receipt.components.clone()
	} else {
		a.components.clone()
	};
	state.insert("components".into(), json!(wanted));
	let payload = LipPayload(inst.lip.clone());
	let mut result: Option<Receipt> = None;
	let mut work = |st: &State, opts: &mut RunOptions<'_>| -> Result<(), EngineError> {
		let comps = components_from(st).unwrap_or_else(|| wanted.clone());
		result = Some(li_engine::modify(
			&mut m,
			&inst.project,
			&payload,
			&inst.dir,
			&comps,
			opts,
		)?);
		Ok(())
	};
	drive.run("modify", state, &mut *ui, &mut work)?;
	let receipt =
		result.ok_or_else(|| RunnerError::new("the modify flow ended without changes"))?;
	drop(ui);
	println!("Components now: {}", receipt.components.join(", "));
	Ok(())
}

fn repair(cli: &Cli) -> RunnerResult<()> {
	let mut m = RealMachine::from_environment();
	let inst = open(cli, &m)?;
	let payload = LipPayload(inst.lip.clone());
	let mut ui = ConsoleUi::new(State::new());
	let mut report = |p: &Progress| ui.progress(p);
	let mut opts = RunOptions {
		cancel: None,
		on_progress: Some(&mut report),
	};
	let r = li_engine::repair(&mut m, &payload, &inst.dir, &mut opts)?;
	println!("Checked {} items; restored {}", r.checked, r.restored.len());
	for s in &r.restored {
		println!("  restored {s}");
	}
	Ok(())
}

fn uninstall(cli: &Cli) -> RunnerResult<()> {
	let mut m = RealMachine::from_environment();
	let inst = open(cli, &m)?;
	let env = script_env(&m, &inst.target.id, &inst.dir, &inst.receipt.installed);
	let scripts = ProjectScripts::load(&inst.project, env, m.script_fs())?;
	let drive = Drive::new(&inst.project, &inst.target, &scripts, &m, &inst.dir);
	let mut ui = make_ui(cli, &inst.target)?;
	let mut work = |_: &State, opts: &mut RunOptions<'_>| -> Result<(), EngineError> {
		li_engine::uninstall(&mut m, Some(&inst.project), &inst.dir, opts)
	};
	drive.run("uninstall", State::new(), &mut *ui, &mut work)?;
	drop(ui);
	stash::remove(&inst.dir);
	println!(
		"Uninstalled {} from {}",
		inst.receipt.product,
		inst.dir.display()
	);
	Ok(())
}

fn launch(cli: &Cli, a: &LaunchArgs) -> RunnerResult<()> {
	let mut m = RealMachine::from_environment();
	let mut inst = open(cli, &m)?;
	// The app may have updated itself since the receipt was written; start from what is installed.
	if let Ok(Some(next)) = li_engine::reconcile_receipt(&mut m, &inst.dir) {
		inst.receipt = next;
	}
	let now = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map_or(0, |d| d.as_secs());
	let interval = inst
		.receipt
		.updates
		.as_ref()
		.and_then(|u| u.check_interval_hours);
	let due = li_engine::tauri::updates::check_due(&m, &inst.dir, interval, now);
	let spec = a
		.feed
		.clone()
		.or_else(|| inst.receipt.feed.clone())
		.filter(|_| due && li_engine::tauri::updates::installer_owns(&inst.receipt));
	if spec.is_some() {
		let _ = li_engine::tauri::updates::record_check(&mut m, &inst.dir, now);
	}
	let feed = match (&spec, feed_key(&inst, a.feed_key.as_deref())) {
		(Some(spec), Ok(key)) => {
			feedsrc::load_feed(spec, &key, stash::last_feed(&inst.dir).as_ref())
				.map(|(f, _)| f)
				.map_err(|e| eprintln!("liminal-engine: not checking for updates: {e}"))
				.ok()
		}
		_ => None,
	};
	if let LaunchOutcome::UpdateAvailable(v) =
		li_engine::launch(&mut m, &inst.dir, feed.as_ref(), false)?
	{
		println!(
			"Version {v} is available; run `liminal-engine update` to install it. Starting {} anyway.",
			inst.receipt.installed
		);
		li_engine::launch(&mut m, &inst.dir, None, true)?;
	}
	Ok(())
}

/// Runs a parsed command line and maps the result to an exit code.
pub fn run(cli: Cli) -> ExitCode {
	let result = match &cli.command {
		Command::Install(a) => install(&cli, a),
		Command::Update(a) => update(&cli, a),
		Command::Modify(a) => modify(&cli, a),
		Command::Repair => repair(&cli),
		Command::Uninstall => uninstall(&cli),
		Command::Launch(a) => launch(&cli, a),
	};
	match result {
		Ok(()) => ExitCode::SUCCESS,
		Err(RunnerError::Cancelled) => {
			eprintln!("liminal-engine: cancelled; every change was rolled back");
			ExitCode::from(3)
		}
		Err(e) => {
			eprintln!("liminal-engine: {e}");
			ExitCode::from(1)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn answers_are_json_or_text() {
		let s = parse_answers(&[
			"install_dir=/opt/app".into(),
			"accepted=true".into(),
			"n=3".into(),
			"components=[\"a\",\"b\"]".into(),
		])
		.unwrap();
		assert_eq!(s["install_dir"], json!("/opt/app"));
		assert_eq!(s["accepted"], json!(true));
		assert_eq!(s["n"], json!(3));
		assert_eq!(components_from(&s), Some(vec!["a".into(), "b".into()]));
		assert!(parse_answers(&["nope".into()]).is_err());
		assert!(parse_answers(&["=x".into()]).is_err());
	}

	#[test]
	fn components_may_be_a_comma_list() {
		let s = parse_answers(&["components=app, sound".into()]).unwrap();
		assert_eq!(
			components_from(&s),
			Some(vec!["app".into(), "sound".into()])
		);
		assert_eq!(components_from(&State::new()), None);
	}
}
