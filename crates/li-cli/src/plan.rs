// Dry-run plans: what installing a target would do, before any scripts run.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeSet;

use li_model::{Operation, Plan, Target};

use crate::CliResult;
use crate::project::Project;

/// Builds the plan `preview` prints. The engine implements this by running the flow against a
/// simulated machine; [`StaticPlanner`] is the default until it does.
pub trait Planner {
	fn plan(&self, project: &Project, target: &Target) -> CliResult<Plan>;
}

/// A plan read straight from `components.toml`: directories, copies, shortcuts, file types,
/// URL schemes and PATH entries for every component that applies to the target, then
/// `receipt.write`. It runs no scripts and chooses no optional components.
pub struct StaticPlanner;

impl Planner for StaticPlanner {
	fn plan(&self, project: &Project, target: &Target) -> CliResult<Plan> {
		let mut ops = Vec::new();
		let mut dirs = BTreeSet::new();
		for c in project
			.components
			.components
			.iter()
			.filter(|c| c.applies_to(&target.id))
		{
			for rule in &c.files {
				let to_dir = match rule.to.rfind('/') {
					Some(i) if !rule.to.ends_with('/') => &rule.to[..i],
					_ => rule.to.trim_end_matches('/'),
				};
				if !to_dir.is_empty() && to_dir != "{install}" && dirs.insert(to_dir.to_owned()) {
					ops.push(Operation::Mkdir {
						path: to_dir.to_owned(),
					});
				}
				ops.push(Operation::Copy {
					from: rule.from.replace("{target}", &target.id),
					to: rule.to.clone(),
					sha256: None,
				});
			}
			for s in &c.shortcut {
				ops.push(Operation::Shortcut {
					name: s.name.clone(),
					target: s.target.clone(),
					place: s.place.clone(),
				});
			}
			for ft in &c.file_types {
				ops.push(Operation::FileType {
					extension: ft.extension.clone(),
					name: ft.name.clone(),
					open_with: ft.open_with.clone(),
				});
			}
			for scheme in &c.url_schemes {
				ops.push(Operation::UrlScheme {
					scheme: scheme.clone(),
					open_with: format!("{{install}}/{}", c.id),
				});
			}
			for entry in &c.path {
				ops.push(Operation::PathAdd {
					entry: entry.clone(),
				});
			}
		}
		ops.push(Operation::ReceiptWrite {
			path: format!("{{install}}/{}", li_model::Receipt::PATH),
		});
		Ok(Plan { ops })
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
		Operation::ReceiptWrite { path } => format!("receipt.write  {path}"),
	}
}
