// The `liminal-engine` binary.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
	li_runner::run(li_runner::Cli::parse())
}
