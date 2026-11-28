// The Plan phase: components and scripts become an ordered list of operations.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeSet;

use li_model::{Operation, Plan, Receipt, Scope};

use crate::error::{EngineError, Result};
use crate::payload::{Payload, expand_sources};
use crate::project::Project;
use crate::scripts::ProjectScripts;
use crate::tauri::{
	self,
	plan::{Skip, WindowsTauri},
};

/// Where the receipt is written, as a placeholder path.
pub fn receipt_path() -> String {
	format!("{{install}}/{}", Receipt::PATH)
}

/// Validates a component selection against a target and returns it in `components.toml` order
/// with every required component included. `None` selects the components that start ticked: the
/// required ones and every optional one whose `default` is not `false`.
pub fn select_components(
	project: &Project,
	target: &str,
	requested: Option<&[String]>,
) -> Result<Vec<String>> {
	if project.target(target).is_none() {
		return Err(EngineError::UnknownTarget(target.to_owned()));
	}
	if let Some(req) = requested {
		for id in req {
			let c = project
				.component(id)
				.ok_or_else(|| EngineError::UnknownComponent(id.clone()))?;
			if !c.applies_to(target) {
				return Err(EngineError::NotApplicable(id.clone(), target.to_owned()));
			}
		}
	}
	Ok(project
		.components
		.components
		.iter()
		.filter(|c| c.applies_to(target))
		.filter(|c| match requested {
			Some(r) => c.required || r.contains(&c.id),
			None => c.on_by_default(),
		})
		.map(|c| c.id.clone())
		.collect())
}

/// Emits `mkdir` for every directory from the placeholder root down to `dir`, once each.
pub(crate) fn mkdirs(dir: &str, seen: &mut BTreeSet<String>, ops: &mut Vec<Operation>) {
	let mut acc = String::new();
	for (i, part) in dir.split('/').enumerate() {
		if i == 0 {
			if part.is_empty() {
				continue;
			}
			acc.push_str(part);
		} else {
			acc.push('/');
			acc.push_str(part);
		}
		if (acc.starts_with('{') && !acc.contains('/') && acc != "{install}") || acc.is_empty() {
			continue;
		}
		if seen.insert(acc.clone()) {
			ops.push(Operation::Mkdir { path: acc.clone() });
		}
	}
}

/// Builds the plan for a target and component selection. The plan holds placeholders, not
/// machine paths, so it is identical on every machine. `receipt.write` is always last.
pub fn build_plan(
	project: &Project,
	payload: &dyn Payload,
	target: &str,
	selected: &[String],
	scripts: Option<&ProjectScripts>,
) -> Result<Plan> {
	build_plan_scoped(
		project,
		payload,
		target,
		selected,
		scripts,
		project.installer.defaults.scope,
	)
}

/// [`build_plan`] for a given install scope, which decides the registry hive a Windows Tauri
/// product writes to.
pub fn build_plan_scoped(
	project: &Project,
	payload: &dyn Payload,
	target: &str,
	selected: &[String],
	scripts: Option<&ProjectScripts>,
	scope: Scope,
) -> Result<Plan> {
	let mut ops = Vec::new();
	let mut seen = BTreeSet::new();
	mkdirs("{install}", &mut seen, &mut ops);
	let tauri = WindowsTauri::of(project, target, scope);
	if let Some(t) = &tauri {
		tauri::plan::pre_ops(t, payload, &mut seen, &mut ops)?;
	}
	for id in selected {
		let c = project
			.component(id)
			.ok_or_else(|| EngineError::UnknownComponent(id.clone()))?;
		let skip = tauri.as_ref().map_or(Skip::Nothing, |t| t.skips(c));
		if skip == Skip::All {
			continue;
		}
		for rule in &c.files {
			let pattern = rule.from.replace("{target}", target);
			let found = expand_sources(payload, &pattern);
			if found.is_empty() {
				return Err(EngineError::MissingSource(pattern));
			}
			for (src, rel) in found {
				let to = if rule.to.ends_with('/') {
					format!("{}{}", rule.to, rel)
				} else if pattern.contains(['*', '?']) || pattern.ends_with('/') {
					format!("{}/{}", rule.to, rel)
				} else {
					rule.to.clone()
				};
				if let Some((parent, _)) = to.rsplit_once('/') {
					mkdirs(parent, &mut seen, &mut ops);
				}
				let sha = li_pack::sha256_hex(&payload.read(&src)?);
				ops.push(Operation::Copy {
					from: src,
					to,
					sha256: Some(sha),
				});
			}
		}
		let mut first_target = None;
		for s in &c.shortcut {
			first_target.get_or_insert_with(|| s.target.clone());
			ops.push(Operation::Shortcut {
				name: s.name.clone(),
				target: s.target.clone(),
				place: s.place.clone(),
				args: s.args.clone().filter(|a| !a.is_empty()),
			});
		}
		for ft in c.file_types.iter().filter(|_| skip == Skip::Nothing) {
			ops.push(Operation::FileType {
				extension: ft.extension.clone(),
				name: ft.name.clone(),
				open_with: ft.open_with.clone().or_else(|| first_target.clone()),
			});
		}
		for scheme in c.url_schemes.iter().filter(|_| skip == Skip::Nothing) {
			ops.push(Operation::UrlScheme {
				scheme: scheme.clone(),
				open_with: first_target.clone().unwrap_or_default(),
			});
		}
		for entry in &c.path {
			ops.push(Operation::PathAdd {
				entry: entry.clone(),
			});
		}
	}
	if let Some(t) = &tauri {
		tauri::plan::post_ops(t, payload, &mut ops);
	}
	if let Some(scripts) = scripts {
		let so_far = Plan { ops: ops.clone() };
		ops.extend(scripts.on_commit(&so_far)?);
	}
	// Last before the receipt, so an uninstall (which undoes the plan backwards) reaches it first.
	for pre in &project.installer.uninstall.pre {
		ops.push(Operation::UninstallPre {
			run: pre.run.clone(),
			args: pre.args.clone(),
			wait_ms: pre.wait_ms,
			ignore_failure: pre.ignore_failure,
		});
	}
	ops.push(Operation::ReceiptWrite {
		path: receipt_path(),
	});
	Ok(Plan { ops })
}

/// Whether two operations ask for the same thing: the outcome an operation records after it ran
/// (what a WebView2 step found, the uninstall entry a wrapped installer left) is not part of it.
pub fn same_plan(a: &Operation, b: &Operation) -> bool {
	fn plain(op: &Operation) -> Operation {
		let mut op = op.clone();
		match &mut op {
			Operation::PrereqWebview2 { found, ran, .. } => {
				*found = None;
				*ran = false;
			}
			Operation::TauriWrap { external, .. } => *external = None,
			_ => {}
		}
		op
	}
	plain(a) == plain(b)
}

/// A stable identity for an operation, so an update can tell what replaces what.
pub fn op_key(op: &Operation) -> String {
	match op {
		Operation::Mkdir { path } => format!("mkdir:{path}"),
		Operation::Copy { to, .. } => format!("file:{to}"),
		Operation::WriteFile { path, .. } => format!("file:{path}"),
		Operation::WriteIni {
			path, section, key, ..
		} => format!("ini:{path}:{section}:{key}"),
		Operation::WriteJson { path, pointer, .. } => format!("json:{path}:{pointer}"),
		Operation::Shortcut { name, place, .. } => format!("shortcut:{place}:{name}"),
		Operation::UninstallPre { run, args, .. } => format!("uninstall_pre:{run}:{args}"),
		Operation::FileType { extension, .. } => format!("file_type:{extension}"),
		Operation::UrlScheme { scheme, .. } => format!("url_scheme:{scheme}"),
		Operation::RegistrySet { key, name, .. } => format!("registry:{key}:{name}"),
		Operation::PathAdd { entry } => format!("path:{entry}"),
		Operation::ServiceInstall { name, .. } => format!("service:{name}"),
		Operation::OsRegister { product, .. } => format!("os_register:{product}"),
		Operation::PrereqWebview2 { .. } => "prereq:webview2".to_owned(),
		Operation::TauriWrap { product_name, .. } => format!("tauri_wrap:{product_name}"),
		Operation::ReceiptWrite { .. } => "receipt".to_owned(),
	}
}
