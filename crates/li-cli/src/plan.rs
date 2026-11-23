// Dry-run plans: what installing a target would do, from the engine's Plan phase.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use li_engine::{DirPayload, InstallRequest, SimMachine};
use li_model::{Operation, Plan, Target};

use crate::project::Project;
use crate::{CliError, CliResult};

/// Builds the plan `preview` prints.
pub trait Planner {
	fn plan(&self, project: &Project, target: &Target) -> CliResult<Plan>;
}

/// The engine's Plan phase against a simulated machine: every component that applies to the
/// target, the `on_commit` script hooks, the file hashes and `receipt.write` last. Nothing on
/// the machine is touched.
pub struct EnginePlanner;

impl Planner for EnginePlanner {
	fn plan(&self, project: &Project, target: &Target) -> CliResult<Plan> {
		let payload = DirPayload::new(&project.dir).map_err(|e| CliError::new(e.to_string()))?;
		let slug = project
			.installer
			.product
			.id
			.rsplit('.')
			.next()
			.unwrap_or("app");
		let req = InstallRequest {
			target: target.id.clone(),
			install_dir: PathBuf::from(format!("/opt/{slug}")),
			scope: project.installer.defaults.scope,
			components: None,
			version: None,
			feed: None,
			channel: None,
		};
		// A Windows Tauri target is drawn against a Windows machine with no WebView2 runtime, so
		// the plan shows what the step does when it is missing.
		let machine = if project.installer.tauri.is_some() && target.id.starts_with("windows") {
			SimMachine::new().with_os("windows")
		} else {
			SimMachine::new()
		};
		li_engine::preview(project, &payload, &req, &machine)
			.map_err(|e| CliError::new(e.to_string()))
	}
}

/// One line describing an operation, for human output.
pub fn describe(op: &Operation) -> String {
	match op {
		Operation::Mkdir { path } => format!("mkdir          {path}"),
		Operation::Copy { from, to, .. } => format!("copy           {from} -> {to}"),
		Operation::WriteFile { path, .. } => format!("write_file     {path}"),
		Operation::WriteIni {
			path,
			section,
			key,
			value,
		} => format!("write_ini      {path} [{section}] {key}={value}"),
		Operation::WriteJson { path, pointer, .. } => format!("write_json     {path} {pointer}"),
		Operation::Shortcut {
			name,
			target,
			place,
		} => format!("shortcut       {name} -> {target} in {place}"),
		Operation::FileType {
			extension, name, ..
		} => format!(
			"file_type      .{} ({name})",
			extension.trim_start_matches('.')
		),
		Operation::UrlScheme { scheme, open_with } => {
			format!("url_scheme     {scheme}: -> {open_with}")
		}
		Operation::RegistrySet { key, name, value } => {
			format!("registry.set   {key} {name}={value}")
		}
		Operation::PathAdd { entry } => format!("path.add       {entry}"),
		Operation::ServiceInstall { name, command } => {
			format!("service.install {name}: {command}")
		}
		Operation::OsRegister { product, version } => {
			format!("os.register    {product} {version}")
		}
		Operation::PrereqWebview2 {
			mode,
			min_version,
			installer,
			source,
			path,
			found,
			ran,
			..
		} => {
			let mut s = format!("prereq.webview2 {}", mode.name());
			if let Some(v) = min_version {
				s.push_str(&format!(", at least {v}"));
			}
			if let Some(i) = installer {
				s.push_str(&format!(" via {i}"));
			}
			if let Some(u) = source {
				s.push_str(&format!(" from {u}"));
			}
			if let Some(p) = path {
				s.push_str(&format!(" at {p}"));
			}
			match (found, ran) {
				(Some(v), false) => s.push_str(&format!(" (found {v}, nothing to do)")),
				(_, true) => s.push_str(" (installed)"),
				_ => {}
			}
			s
		}
		Operation::TauriWrap {
			installer,
			product_name,
			hive,
			nsis_mode,
			..
		} => format!("tauri.wrap     {product_name} via {installer} ({hive}, {nsis_mode})"),
		Operation::ReceiptWrite { path } => format!("receipt.write  {path}"),
	}
}
