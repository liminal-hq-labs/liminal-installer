// Checks that a tool the Studio is pointed at runs, and opens the platform's file manager on a path.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(5);

/// Runs `program --version` and returns the first line it prints. Fails when the program is missing, exits with an
/// error or does not answer within a few seconds.
pub fn check_version(program: &Path) -> Result<String, String> {
	let mut child = Command::new(program)
		.arg("--version")
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()
		.map_err(|e| e.to_string())?;
	let started = Instant::now();
	let status = loop {
		match child.try_wait().map_err(|e| e.to_string())? {
			Some(status) => break status,
			None if started.elapsed() > WAIT => {
				let _ = child.kill();
				let _ = child.wait();
				return Err("it did not answer in time".into());
			}
			None => std::thread::sleep(Duration::from_millis(20)),
		}
	};
	if !status.success() {
		return Err(format!("it exited with {status}"));
	}
	let mut text = String::new();
	if let Some(mut out) = child.stdout.take() {
		use std::io::Read;
		let _ = out.read_to_string(&mut text);
	}
	let line = text.lines().next().unwrap_or("").trim().to_string();
	if line.is_empty() {
		Err("it printed nothing".into())
	} else {
		Ok(line)
	}
}

/// Shows `path` in the platform's file manager: its folder, with the file selected where the platform can.
pub fn reveal(path: &Path) -> Result<(), String> {
	let folder = if path.is_dir() {
		path
	} else {
		path.parent().unwrap_or(path)
	};
	#[cfg(target_os = "macos")]
	let mut cmd = {
		let mut c = Command::new("open");
		if path.is_file() {
			c.arg("-R").arg(path);
		} else {
			c.arg(folder);
		}
		c
	};
	#[cfg(windows)]
	let mut cmd = {
		let mut c = Command::new("explorer");
		if path.is_file() {
			c.arg(format!("/select,{}", path.display()));
		} else {
			c.arg(folder);
		}
		c
	};
	#[cfg(not(any(target_os = "macos", windows)))]
	let mut cmd = {
		let mut c = Command::new("xdg-open");
		c.arg(folder);
		c
	};
	cmd.stdin(Stdio::null())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.spawn()
		.map(|_| ())
		.map_err(|e| format!("could not open the file manager: {e}"))
}

#[cfg(all(test, unix))]
mod tests {
	use super::*;
	use std::fs;
	use std::os::unix::fs::PermissionsExt;

	fn script(name: &str, body: &str) -> std::path::PathBuf {
		let dir = std::env::temp_dir().join(format!("li-studio-tools-{}", std::process::id()));
		fs::create_dir_all(&dir).unwrap();
		let path = dir.join(name);
		fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
		fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
		path
	}

	#[test]
	fn a_tool_that_answers_gives_its_first_line() {
		let ok = script("ok", "echo 'tool 1.2.3'; echo more");
		assert_eq!(check_version(&ok).unwrap(), "tool 1.2.3");
	}

	#[test]
	fn a_missing_failing_or_silent_tool_is_refused() {
		assert!(check_version(Path::new("/definitely/not/here")).is_err());
		let failing = script("failing", "exit 3");
		assert!(check_version(&failing).unwrap_err().contains("exited"));
		let silent = script("silent", "exit 0");
		assert!(check_version(&silent).unwrap_err().contains("nothing"));
	}
}
