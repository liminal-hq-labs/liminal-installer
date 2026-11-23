// The `liminst` argument parser and command dispatch.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use li_render_tui::Look;

use crate::build::{BuildRequest, Builder, NativeBuilder, resolve_version};
use crate::check::{DefaultChecker, check_project};
use crate::plan::StaticPlanner;
use crate::preview::{Only, PreviewOptions, preview};
use crate::project::Project;
use crate::scaffold::{InitOptions, init};
use crate::util::to_json_tabs;
use crate::{CliError, CliResult, feed, theme_pack};

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
	/// Work with release feeds
	#[command(subcommand)]
	Feed(FeedCommand),
	/// Work with themes
	#[command(subcommand)]
	Theme(ThemeCommand),
	/// Serve the project to the Studio and agents (not available yet)
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
	/// An engine binary to carry in the installer and start after extraction
	#[arg(long, value_name = "PATH")]
	pub engine: Option<PathBuf>,
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
	/// Sign the feed with this key (not available yet)
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
	/// Sign the theme with this key (not available yet)
	#[arg(long, value_name = "FILE")]
	pub sign_key: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct StudioArgs {
	/// Serve MCP to the Studio and agents
	#[arg(long)]
	pub mcp: bool,
}

fn load(cli: &Cli) -> CliResult<Project> {
	Project::load(&cli.project).map_err(Into::into)
}

fn find_target<'a>(project: &'a Project, id: &str) -> CliResult<&'a li_model::Target> {
	project.target(id).ok_or_else(|| {
		let known: Vec<&str> = project
			.installer
			.targets
			.iter()
			.map(|t| t.id.as_str())
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
			let project = match Project::load(&cli.project) {
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
			let report = check_project(&project, &a.targets, &[&DefaultChecker]);
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
			print!("{}", preview(&project, target, &StaticPlanner, &opts)?);
			Ok(0)
		}
		Command::Build(a) => {
			let project = load(cli)?;
			let target = find_target(&project, &a.target)?;
			let report = check_project(
				&project,
				std::slice::from_ref(&a.target),
				&[&DefaultChecker],
			);
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
		Command::Feed(FeedCommand::Add(a)) => {
			if a.sign_key.is_some() {
				return Err(CliError::new(
					"feed signing is not available yet; run without --sign-key",
				));
			}
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
			};
			feed::add(&project, &req)?;
			println!(
				"Added {} {} ({}) to {}",
				project.installer.product.id,
				version,
				target.full_id(),
				a.feed.display()
			);
			println!("Note: the feed is unsigned; signing is not available yet.");
			Ok(0)
		}
		Command::Theme(ThemeCommand::Pack(a)) => {
			if a.sign_key.is_some() {
				return Err(CliError::new(
					"theme signing is not available yet; run without --sign-key",
				));
			}
			let project = load(cli)?;
			let name = match &a.theme {
				Some(n) => n.clone(),
				None => project.installer.defaults.theme.clone().ok_or_else(|| {
					CliError::new("name a theme to pack; the project sets no default theme")
				})?,
			};
			let packed = theme_pack::pack(&project, &name, a.out.as_deref())?;
			println!(
				"Packed theme `{name}`: {} files, sha256 {}\n  {}",
				packed.files,
				packed.sha256,
				packed.path.display()
			);
			println!("Note: the theme is unsigned; signing is not available yet.");
			Ok(0)
		}
		Command::Studio(a) => {
			if !a.mcp {
				return Err(CliError::new("nothing to serve; pass --mcp"));
			}
			Err(CliError::new(
				"`liminst studio --mcp` is not available yet: the Studio and its MCP server are still to be built",
			))
		}
	}
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
