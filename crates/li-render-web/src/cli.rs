// The web runner's command line: the runner's own, plus a place to record the events.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory, Parser};
use li_runner::{exit_code, parse_answers};

use crate::session::run_headless;
use crate::wire::RunEvent;

/// `liminal-runner`: the same commands and options as `liminal-engine`, drawn in a window. With
/// `--headless` no window opens and every page is answered from `--answer`.
#[derive(Debug, Parser)]
#[command(
	name = "liminal-runner",
	version,
	about = "Install, update, modify, repair or uninstall a product in a window drawn by the web renderer",
	after_help = "With no command, `install` runs. Exit codes: 0 success, 1 failure (changes rolled back), 2 bad arguments, 3 cancelled."
)]
pub struct RunnerCli {
	/// Write every event the window would get to FILE, one JSON object per line
	#[arg(long, value_name = "FILE", global = true)]
	pub events: Option<PathBuf>,
	#[command(flatten)]
	pub runner: li_runner::Cli,
}

/// Reads a command line. With no lifecycle command, `install` is the command, so double-clicking
/// the runner next to its `payload.lip` installs.
pub fn parse_args<I, T>(args: I) -> Result<RunnerCli, clap::Error>
where
	I: IntoIterator<Item = T>,
	T: Into<std::ffi::OsString> + Clone,
{
	let mut args: Vec<std::ffi::OsString> = args.into_iter().map(Into::into).collect();
	let commands: Vec<String> = li_runner::Cli::command()
		.get_subcommands()
		.map(|c| c.get_name().to_owned())
		.collect();
	let has_command = args
		.iter()
		.skip(1)
		.any(|a| a.to_str().is_some_and(|a| commands.iter().any(|c| c == a)));
	let wants_help = args
		.iter()
		.skip(1)
		.any(|a| matches!(a.to_str(), Some("--help" | "-h" | "--version" | "-V")));
	if !has_command && !wants_help {
		args.push("install".into());
	}
	RunnerCli::try_parse_from(args)
}

/// Runs with no window and returns the exit code: what `--headless` does, and what the
/// `liminal-runner-headless` binary always does.
pub fn run_headless_main(cli: RunnerCli) -> ExitCode {
	let answers = match parse_answers(&cli.runner.answers) {
		Ok(a) => a,
		Err(e) => {
			eprintln!("liminal-runner: {e}");
			return ExitCode::from(2);
		}
	};
	let report = run_headless(cli.runner, answers);
	for event in &report.events {
		if let RunEvent::Page { page } = event {
			println!("page {}", page.step);
		}
	}
	if let Some(path) = &cli.events {
		let lines: Vec<String> = report
			.events
			.iter()
			.filter_map(|e| serde_json::to_string(e).ok())
			.collect();
		if let Err(e) = std::fs::write(path, lines.join("\n") + "\n") {
			eprintln!("liminal-runner: {}: {e}", path.display());
			return ExitCode::from(1);
		}
	}
	exit_code(report.result)
}
