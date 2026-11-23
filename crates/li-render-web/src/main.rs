// `liminal-runner-headless`: the web runner's flow with no window, for CI and scripts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::process::ExitCode;

fn main() -> ExitCode {
	match li_render_web::cli::parse_args(std::env::args_os()) {
		Ok(cli) => li_render_web::cli::run_headless_main(cli),
		Err(e) => {
			let code = if e.use_stderr() { 2 } else { 0 };
			let _ = e.print();
			ExitCode::from(code)
		}
	}
}
