// The `liminst` argument parser and command dispatch.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use li_render_tui::Look;

use crate::build::{BuildRequest, Builder, NativeBuilder, resolve_version};
use crate::check::{DefaultChecker, EngineChecker, ProjectChecker, check_project};
use crate::plan::EnginePlanner;
use crate::preview::{Only, PreviewOptions, preview};
use crate::project::{self, Project};
use crate::scaffold::{InitOptions, init};
use crate::util::to_json_tabs;
use crate::{CliError, CliResult, feed, keygen, theme_pack};

#[derive(Debug, Parser)]
#[command(
	name = "liminst",
	version,
	about = "Build, check and preview themed installers from one project",
	after_help = "Exit codes: 0 success, 1 the project has errors, 2 the command could not run."
)]
pub struct Cli {
	/// The folder that holds `installer/` (default: the current folder)
	#[arg(
		short = 'C',
		long = "project",
		global = true,
		value_name = "DIR",
		default_value = "."
	)]
	pub project: PathBuf,
	#[command(subcommand)]
	pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
	/// Create installer/ in a product repository, with a neutral theme
	Init(InitArgs),
	/// Validate the project for every target
	Check(CheckArgs),
	/// Dry run: print the plan and draw each step as text
	Preview(PreviewArgs),
	/// Write the outputs for a target
	Build(BuildArgs),
	/// Make an Ed25519 key pair for signing feeds and themes
	Keygen(KeygenArgs),
	/// Work with release feeds
	#[command(subcommand)]
	Feed(FeedCommand),
	/// Work with themes
	#[command(subcommand)]
	Theme(ThemeCommand),
	/// Serve the project to agents over MCP, without a window
	Studio(StudioArgs),
}

#[derive(Debug, Args)]
pub struct InitArgs {
	/// Product id in reverse-DNS form, for example hq.liminal.planboard
	#[arg(long)]
	pub id: Option<String>,
	/// Product name shown to people
	#[arg(long)]
	pub name: Option<String>,
	/// Publisher name
	#[arg(long, default_value = "Unknown publisher")]
	pub publisher: String,
	/// Overwrite an existing scaffold
	#[arg(long)]
	pub force: bool,
}

#[derive(Debug, Args)]
pub struct KeygenArgs {
	/// Where to write the pair: `<OUT>.key` (secret) and `<OUT>.pub` (public)
	#[arg(long, default_value = "feed", value_name = "OUT")]
	pub out: PathBuf,
	/// Replace existing key files
	#[arg(long)]
	pub force: bool,
}

#[derive(Debug, Args)]
pub struct CheckArgs {
	/// Check only this target (repeatable)
	#[arg(short, long = "target", value_name = "ID")]
	pub targets: Vec<String>,
	/// Print the report as JSON
	#[arg(long)]
	pub json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OnlyArg {
	All,
	Plan,
	Screens,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StyleArg {
	Retro,
	Plain,
}

#[derive(Debug, Args)]
pub struct PreviewArgs {
	/// The target id, for example linux-x86_64
	#[arg(short, long, value_name = "ID")]
	pub target: String,
	/// Which flow to walk
	#[arg(long, default_value = "install")]
	pub flow: String,
	/// What to print
	#[arg(long, value_enum, default_value = "all")]
	pub only: OnlyArg,
	/// How to draw the screens
	#[arg(long, value_enum, default_value = "plain")]
	pub style: StyleArg,
	/// Screen size in cells, as COLSxROWS
	#[arg(long, default_value = "80x24", value_name = "COLSxROWS")]
	pub size: String,
	/// Print JSON instead of text
	#[arg(long)]
	pub json: bool,
}

#[derive(Debug, Args)]
pub struct BuildArgs {
	/// The target id, for example linux-x86_64
	#[arg(short, long, value_name = "ID")]
	pub target: String,
	/// Where to write outputs (default: build/<target>)
	#[arg(short, long, value_name = "DIR")]
	pub out: Option<PathBuf>,
	/// The product version (default: from installer.toml or the latest git tag)
	#[arg(long)]
	pub version: Option<String>,
	/// Build only these outputs, for example run or deb (repeatable; default: the target's outputs)
	#[arg(long = "output", value_name = "NAME")]
	pub outputs: Vec<String>,
	/// The `liminal-engine` runner to carry in the `.run`; without it the `.run` only extracts
	#[arg(long, value_name = "PATH")]
	pub engine: Option<PathBuf>,
	/// The `SETUP.EXE` DOS runtime to carry in a DOS target's outputs; without it one is found or built from `dos/`
	#[arg(long, value_name = "PATH")]
	pub dos_runtime: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum FeedCommand {
	/// Add a built artefact to a feed
	Add(FeedAddArgs),
}

#[derive(Debug, Args)]
pub struct FeedAddArgs {
	/// The feed file to create or update
	#[arg(long, default_value = "feed.json", value_name = "FILE")]
	pub feed: PathBuf,
	/// The target id the artefact is for
	#[arg(short, long, value_name = "ID")]
	pub target: String,
	/// The built file; its size and SHA-256 are recorded
	#[arg(long, value_name = "FILE")]
	pub file: PathBuf,
	/// Where the file will be hosted
	#[arg(long)]
	pub url: String,
	/// The release version (default: from installer.toml or the latest git tag)
	#[arg(long)]
	pub version: Option<String>,
	/// The artefact kind
	#[arg(long, default_value = "payload")]
	pub kind: String,
	/// The channel to point at this version
	#[arg(long, default_value = "stable")]
	pub channel: String,
	/// Sign the feed with this secret key (from `liminst keygen`); writes `<feed>.sig`
	#[arg(long, value_name = "FILE")]
	pub sign_key: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum ThemeCommand {
	/// Package a theme for sharing
	Pack(ThemePackArgs),
}

#[derive(Debug, Args)]
pub struct ThemePackArgs {
	/// The theme folder name under installer/themes (default: the project's default theme)
	pub theme: Option<String>,
	/// The archive to write (default: <theme id>.litheme in the project folder)
	#[arg(short, long, value_name = "FILE")]
	pub out: Option<PathBuf>,
	/// Sign the theme with this secret key (from `liminst keygen`); writes `<archive>.sig`
	#[arg(long, value_name = "FILE")]
	pub sign_key: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct StudioArgs {
	/// Serve the project over MCP on 127.0.0.1 until Ctrl+C. Prints the URL and a new token once
	#[arg(long)]
	pub mcp: bool,
	/// The port to listen on (default: any free port)
	#[arg(long, default_value_t = 0, value_name = "PORT", requires = "mcp")]
	pub port: u16,
}

fn load(cli: &Cli) -> CliResult<Project> {
	project::load(&cli.project).map_err(Into::into)
}

/// A target by its `os-arch` id, or by its `os-arch:renderer` form when a project has two targets
/// for one platform (the first of those is what a bare id finds).
fn find_target<'a>(project: &'a Project, id: &str) -> CliResult<&'a li_model::Target> {
	let by_full_id = project.installer.targets.iter().find(|t| t.full_id() == id);
	by_full_id.or_else(|| project.target(id)).ok_or_else(|| {
		let known: Vec<String> = project
			.installer
			.targets
			.iter()
			.map(|t| t.id.clone())
			.collect();
		CliError::new(format!(
			"no target named `{id}` (the project has: {})",
			if known.is_empty() {
				"none".to_owned()
			} else {
				known.join(", ")
			}
		))
	})
}

fn parse_size(s: &str) -> CliResult<(u16, u16)> {
	let bad = || {
		CliError::new(format!(
			"`{s}` is not a size; use COLSxROWS, for example 80x24"
		))
	};
	let (w, h) = s.split_once(['x', 'X']).ok_or_else(bad)?;
	let (w, h): (u16, u16) = (w.parse().map_err(|_| bad())?, h.parse().map_err(|_| bad())?);
	if !(10..=500).contains(&w) || !(4..=200).contains(&h) {
		return Err(CliError::new("the size must be between 10x4 and 500x200"));
	}
	Ok((w, h))
}

fn titleize(slug: &str) -> String {
	slug.split(['-', '_'])
		.filter(|p| !p.is_empty())
		.map(|p| {
			let mut c = p.chars();
			c.next()
				.map(|f| f.to_uppercase().collect::<String>() + c.as_str())
				.unwrap_or_default()
		})
		.collect::<Vec<_>>()
		.join(" ")
}

const CHECKERS: &[&dyn ProjectChecker] = &[&DefaultChecker, &EngineChecker];

/// Runs a parsed command line and returns its exit code.
pub fn execute(cli: &Cli) -> CliResult<u8> {
	match &cli.command {
		Command::Init(a) => {
			let dir_name = std::fs::canonicalize(&cli.project)
				.ok()
				.and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
				.unwrap_or_else(|| "product".to_owned());
			let slug: String = dir_name
				.to_lowercase()
				.chars()
				.map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
				.collect();
			let name = a.name.clone().unwrap_or_else(|| titleize(&slug));
			let id = a.id.clone().unwrap_or_else(|| format!("example.{slug}"));
			let opts = InitOptions {
				id,
				name,
				publisher: a.publisher.clone(),
				force: a.force,
			};
			let written = init(&cli.project, &opts)?;
			println!(
				"Created {} files in {}:",
				written.len(),
				cli.project.join("installer").display()
			);
			for p in written {
				println!("  {}", p.display());
			}
			println!("Next: liminst check");
			Ok(0)
		}
		Command::Check(a) => {
			let project = match project::load(&cli.project) {
				Ok(p) => p,
				Err(crate::LoadError::Invalid(d)) => {
					let report = crate::diag::Report::new(vec![], vec![d]);
					print_report(&report, a.json);
					return Ok(report.exit_code());
				}
				Err(e) => return Err(e.into()),
			};
			for t in &a.targets {
				find_target(&project, t)?;
			}
			let report = check_project(&project, &a.targets, CHECKERS);
			print_report(&report, a.json);
			Ok(report.exit_code())
		}
		Command::Preview(a) => {
			let project = load(cli)?;
			let target = find_target(&project, &a.target)?;
			let (width, height) = parse_size(&a.size)?;
			let opts = PreviewOptions {
				flow: a.flow.clone(),
				only: match a.only {
					OnlyArg::All => Only::All,
					OnlyArg::Plan => Only::Plan,
					OnlyArg::Screens => Only::Screens,
				},
				look: match a.style {
					StyleArg::Retro => Look::Retro,
					StyleArg::Plain => Look::Plain,
				},
				width,
				height,
				json: a.json,
			};
			print!("{}", preview(&project, target, &EnginePlanner, &opts)?);
			Ok(0)
		}
		Command::Build(a) => {
			let project = load(cli)?;
			let target = find_target(&project, &a.target)?;
			let report = check_project(&project, std::slice::from_ref(&a.target), CHECKERS);
			if !report.ok {
				eprint!("{}", report.render_text());
				eprintln!("Not building: fix the errors above (run `liminst check`).");
				return Ok(1);
			}
			let version = resolve_version(&project, a.version.as_deref())
				.map_err(|e| CliError::new(e.to_string()))?;
			let out_dir = a
				.out
				.clone()
				.unwrap_or_else(|| project.root.join("build").join(&target.id));
			let req = BuildRequest {
				project: &project,
				target,
				version,
				out_dir,
				outputs: a.outputs.clone(),
				engine: a.engine.clone(),
				dos_runtime: a.dos_runtime.clone(),
			};
			let built = NativeBuilder
				.build(&req)
				.map_err(|e| CliError::new(e.to_string()))?;
			for b in &built {
				match (&b.path, &b.note) {
					(Some(p), None) => println!("{}: {}", b.kind, p.display()),
					(Some(p), Some(n)) => println!("{}: {} ({n})", b.kind, p.display()),
					(None, n) => println!("{}: {}", b.kind, n.as_deref().unwrap_or("skipped")),
				}
			}
			Ok(0)
		}
		Command::Keygen(a) => {
			let keys = keygen::generate(&a.out, a.force)?;
			println!(
				"Secret key: {} (keep it out of the repository)",
				keys.secret.display()
			);
			println!("Public key: {}", keys.public.display());
			println!("Public key (hex): {}", keys.public_hex);
			println!(
				"Next: copy the public key into installer/ and set `signing.feed_key` in installer.toml to its path, then `liminst feed add --sign-key {}`.",
				keys.secret.display()
			);
			Ok(0)
		}
		Command::Feed(FeedCommand::Add(a)) => {
			let project = load(cli)?;
			let target = find_target(&project, &a.target)?;
			let version = resolve_version(&project, a.version.as_deref())
				.map_err(|e| CliError::new(e.to_string()))?;
			let req = feed::AddRequest {
				feed: &a.feed,
				target: &target.full_id(),
				kind: &a.kind,
				file: &a.file,
				url: &a.url,
				version: &version,
				channel: &a.channel,
				sign_key: a.sign_key.as_deref(),
			};
			let (_, signature) = feed::add(&project, &req)?;
			println!(
				"Added {} {} ({}) to {}",
				project.installer.product.id,
				version,
				target.full_id(),
				a.feed.display()
			);
			match signature {
				feed::Signature::Signed(sig) => println!("Signed: {}", sig.display()),
				feed::Signature::Unsigned { removed_stale } => {
					println!("Note: the feed is unsigned; pass --sign-key to sign it.");
					if removed_stale {
						println!("Removed the old signature, which no longer matched.");
					}
				}
			}
			Ok(0)
		}
		Command::Theme(ThemeCommand::Pack(a)) => {
			let project = load(cli)?;
			let name = match &a.theme {
				Some(n) => n.clone(),
				None => project.installer.defaults.theme.clone().ok_or_else(|| {
					CliError::new("name a theme to pack; the project sets no default theme")
				})?,
			};
			let packed =
				theme_pack::pack(&project, &name, a.out.as_deref(), a.sign_key.as_deref())?;
			println!(
				"Packed theme `{name}`: {} files, sha256 {}\n  {}",
				packed.files,
				packed.sha256,
				packed.path.display()
			);
			match &packed.signature {
				Some(sig) => println!("Signed: {}", sig.display()),
				None => println!("Note: the theme is unsigned; pass --sign-key to sign it."),
			}
			Ok(0)
		}
		Command::Studio(a) => {
			if !a.mcp {
				return Err(CliError::new("nothing to serve; pass --mcp"));
			}
			serve_mcp(&cli.project, a.port)
		}
	}
}

/// Opens the project and serves it over MCP until the process is interrupted.
///
/// The first two lines of standard output are `Studio MCP server: <url>` and `Token: <token>`,
/// so a script can read them. The token is printed here once and never again, and it changes on
/// every run. Edits go to the files on disk; the undo history lives in memory for the run.
fn serve_mcp(project: &std::path::Path, port: u16) -> CliResult<u8> {
	use std::io::Write;
	use std::sync::Arc;

	use li_studio::mcp::{McpServer, wait_for_interrupt};
	use li_studio::{EngineBackend, Studio};

	let studio = Arc::new(Studio::new(Box::new(EngineBackend)));
	studio
		.open(project)
		.map_err(|e| CliError::new(format!("cannot open the project: {e}")))?;
	let server = McpServer::start(studio, port)
		.map_err(|e| CliError::new(format!("the MCP server did not start: {e}")))?;
	let mut out = std::io::stdout().lock();
	let line = |out: &mut std::io::StdoutLock<'_>| {
		writeln!(out, "{}", server.banner())?;
		writeln!(out, "Token: {}", server.token())?;
		out.flush()
	};
	line(&mut out).map_err(|e| CliError::new(e.to_string()))?;
	drop(out);
	eprintln!("Serving until Ctrl+C. Send the token as a bearer token.");
	wait_for_interrupt().map_err(|e| CliError::new(format!("cannot wait for Ctrl+C: {e}")))?;
	Ok(0)
}

fn print_report(report: &crate::diag::Report, json: bool) {
	if json {
		print!("{}", to_json_tabs(report));
	} else {
		print!("{}", report.render_text());
	}
}

/// Parses `std::env::args`, runs the command and maps the result to an exit code.
pub fn run(cli: Cli) -> ExitCode {
	match execute(&cli) {
		Ok(code) => ExitCode::from(code),
		Err(e) => {
			eprintln!("liminst: {e}");
			ExitCode::from(2)
		}
	}
}
