// `liminst build` for a `web` target: stages the web runner's bundle.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A target whose renderer is `web` has a `web` output: a folder, `web/` beside the other outputs,
//! that holds everything the web runner (`apps/runner`) needs and nothing else:
//!
//! | Entry | Is |
//! |---|---|
//! | `liminal-runner` (`.exe` on Windows) | the runner, from `--web-runner PATH` or built with `--build-web-runner`; a text file, `liminal-runner.placeholder`, says how to get one when neither is given |
//! | `payload.lip` | the payload with the project bundled in it, as for every other renderer |
//! | `project/installer/…` | the project files, readable without opening the `.lip` |
//! | `manifest.json` | what the bundle is: product, target, look, window size, file sizes and SHA-256 hashes |
//!
//! Running the runner in that folder installs: with no command it runs `install`, finds
//! `payload.lip` beside itself and draws the project's pages in a window. The real build is
//! `bun run --cwd apps/runner tauri build --no-bundle`, which `--build-web-runner` runs; it is slow
//! and needs the Tauri system libraries, so the staged folder, the placeholder and the tests do not
//! depend on it.

use std::path::{Path, PathBuf};
use std::process::Command;

use li_engine::bundle::PROJECT_DIR;
use li_pack::{Lip, sha256_hex};
use li_package::PackageError;
use li_render_web::look::{look_of, window_size};
use serde_json::{Value, json};

use crate::build::{BuildError, BuildRequest, BuiltArtefact};

fn other(m: impl Into<String>) -> BuildError {
	BuildError::Other(m.into())
}

/// The runner's file name for a target: `.exe` for Windows.
pub fn runner_name(target_id: &str) -> &'static str {
	if target_id.starts_with("windows") {
		"liminal-runner.exe"
	} else {
		"liminal-runner"
	}
}

/// The repository's `apps/runner` folder, found by walking up from `start`.
fn find_runner_app(start: &Path) -> Option<PathBuf> {
	start
		.ancestors()
		.map(|d| d.join("apps/runner"))
		.find(|d| d.join("src-tauri/tauri.conf.json").is_file())
}

/// Builds the runner with Tauri and returns the binary it wrote.
fn build_runner(req: &BuildRequest) -> Result<PathBuf, BuildError> {
	let mut starts = vec![std::env::current_dir().unwrap_or_default()];
	starts.push(req.project.root.clone());
	if let Ok(exe) = std::env::current_exe() {
		starts.extend(exe.parent().map(Path::to_path_buf));
	}
	let app = starts
		.iter()
		.find_map(|s| find_runner_app(s))
		.ok_or_else(|| {
			other(
				"cannot find apps/runner; run `liminst` inside the repository or pass --web-runner PATH",
			)
		})?;
	let status = Command::new("bun")
		.args(["run", "--cwd"])
		.arg(&app)
		.args(["tauri", "build", "--no-bundle"])
		.status()
		.map_err(|e| other(format!("cannot run `bun` to build the web runner: {e}")))?;
	if !status.success() {
		return Err(other("`tauri build` failed; see its output above"));
	}
	let exe = if cfg!(windows) {
		"liminal-runner.exe"
	} else {
		"liminal-runner"
	};
	let target_dir = std::env::var_os("CARGO_TARGET_DIR")
		.map(PathBuf::from)
		.unwrap_or_else(|| app.join("src-tauri/target"));
	let built = target_dir.join("release").join(exe);
	if built.is_file() {
		Ok(built)
	} else {
		Err(other(format!(
			"`tauri build` finished but {} is not there",
			built.display()
		)))
	}
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), BuildError> {
	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent).map_err(|e| PackageError::io(parent, e))?;
	}
	std::fs::write(path, bytes).map_err(|e| PackageError::io(path, e).into())
}

/// Stages the bundle under `<out>/web/`.
pub fn build(req: &BuildRequest) -> Result<Vec<BuiltArtefact>, BuildError> {
	let dir = req.out_dir.join("web");
	if dir.exists() {
		std::fs::remove_dir_all(&dir).map_err(|e| PackageError::io(&dir, e))?;
	}
	std::fs::create_dir_all(&dir).map_err(|e| PackageError::io(&dir, e))?;

	let lip = crate::build::lip_bytes(req)?;
	write(&dir.join("payload.lip"), &lip)?;
	let parsed = Lip::parse(&lip).map_err(|e| other(e.to_string()))?;
	let prefix = format!("{PROJECT_DIR}/");
	for e in parsed.entries() {
		if let Some(rel) = e.path.strip_prefix(&prefix) {
			write(
				&dir.join("project").join(rel),
				parsed.read(&e.path).unwrap_or_default(),
			)?;
		}
	}

	let named = req
		.web_runner
		.clone()
		.or_else(|| std::env::var_os("LIMINST_WEB_RUNNER").map(PathBuf::from));
	let runner = match (named, req.build_web_runner) {
		(Some(p), _) => Some(p),
		(None, true) => Some(build_runner(req)?),
		(None, false) => None,
	};
	let (runner_file, runner_note, runner_hash) = match &runner {
		Some(path) => {
			let bytes = std::fs::read(path).map_err(|e| {
				other(format!(
					"cannot read the web runner {}: {e}",
					path.display()
				))
			})?;
			let name = runner_name(&req.target.id);
			let dest = dir.join(name);
			write(&dest, &bytes)?;
			#[cfg(unix)]
			{
				use std::os::unix::fs::PermissionsExt;
				std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
					.map_err(|e| PackageError::io(&dest, e))?;
			}
			(name, None, Some(sha256_hex(&bytes)))
		}
		None => {
			let name = "liminal-runner.placeholder";
			write(
				&dir.join(name),
				b"This bundle has no runner yet.\n\nBuild one with `bun run --cwd apps/runner tauri build --no-bundle` (or run `liminst build --build-web-runner`), copy the `liminal-runner` binary here, and run it: with no command it installs payload.lip beside it. `liminst build --web-runner PATH` stages it for you.\n",
			)?;
			(
				name,
				Some(
					"no runner yet; build one with `bun run --cwd apps/runner tauri build --no-bundle` and pass --web-runner PATH",
				),
				None,
			)
		}
	};

	let p = &req.project.installer.product;
	let look = look_of(req.project, req.target);
	let (width, height) = window_size(look);
	let manifest: Value = json!({
		"format": 1,
		"product": {
			"id": p.id,
			"name": p.name,
			"publisher": p.publisher,
			"version": req.version,
		},
		"target": req.target.full_id(),
		"renderer": req.target.renderer,
		"theme": req.project.theme_id_for(req.target),
		"look": look,
		"window": { "width": width, "height": height },
		"runner": {
			"file": runner_file,
			"placeholder": runner.is_none(),
			"sha256": runner_hash,
		},
		"payload": {
			"file": "payload.lip",
			"size": lip.len(),
			"sha256": sha256_hex(&lip),
		},
		"project": "project/installer/",
		"run": [runner_name(&req.target.id), "install"],
	});
	let mut text = serde_json::to_string_pretty(&manifest).map_err(|e| other(e.to_string()))?;
	text.push('\n');
	write(&dir.join("manifest.json"), text.as_bytes())?;

	Ok(vec![BuiltArtefact {
		kind: "web".to_owned(),
		path: Some(dir),
		note: Some(runner_note.map_or_else(
			|| "the web runner bundle".to_owned(),
			|n| format!("the web runner bundle, {n}"),
		)),
	}])
}
