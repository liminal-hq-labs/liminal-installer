// Lifecycle commands: install, update, modify, repair, uninstall and launch.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use li_model::{Artefact, Feed, Operation, Plan, Receipt, Scope};
use serde_json::{Value, json};

use crate::error::{EngineError, Result};
use crate::exec::{Session, Step, Totals};
use crate::machine::{Machine, Resolver};
use crate::payload::{LipPayload, Payload};
use crate::plan::{build_plan, op_key, select_components};
use crate::progress::RunOptions;
use crate::project::Project;
use crate::scripts::ProjectScripts;

/// What to install and where.
#[derive(Debug, Clone)]
pub struct InstallRequest {
	/// The `os-arch` target id, such as `linux-x86_64`.
	pub target: String,
	pub install_dir: PathBuf,
	pub scope: Scope,
	/// `None` selects every component that applies; required components are always included.
	pub components: Option<Vec<String>>,
	/// The version being installed; falls back to the project's literal version.
	pub version: Option<String>,
	/// The feed URL and channel to remember in the receipt for later updates.
	pub feed: Option<String>,
	pub channel: Option<String>,
}

/// What `repair` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairReport {
	pub checked: usize,
	/// A short description of each operation that was redone.
	pub restored: Vec<String>,
}

/// A newer release a feed offers.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateInfo {
	pub version: String,
	pub artefact: Artefact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchOutcome {
	Launched,
	/// The feed has a newer release; nothing was started. Call again with `skip_update_offer`.
	UpdateAvailable(String),
}

/// Reads `{install}/.liminal/receipt.json`.
pub fn read_receipt(m: &dyn Machine, install_dir: &Path) -> Result<Receipt> {
	let path = install_dir.join(Receipt::PATH);
	let bytes = m
		.read(&path)
		.map_err(|_| EngineError::NotInstalled(install_dir.to_owned()))?;
	serde_json::from_slice(&bytes).map_err(|e| EngineError::Parse {
		path,
		message: e.to_string(),
	})
}

/// The `system` values conditions can read: `os`, `arch`, `free_space` and `installed`.
pub fn system_info(m: &dyn Machine, install_dir: &Path) -> Value {
	json!({
		"os": m.os(),
		"arch": m.arch(),
		"free_space": m.free_space(install_dir),
		"installed": m.kind(&install_dir.join(Receipt::PATH)).is_some(),
	})
}

fn plain_target(full: &str) -> &str {
	full.split(':').next().unwrap_or(full)
}

fn full_target(project: &Project, id: &str) -> Result<String> {
	Ok(project
		.target(id)
		.ok_or_else(|| EngineError::UnknownTarget(id.to_owned()))?
		.full_id())
}

fn script_env(
	m: &dyn Machine,
	target: &str,
	install: &Path,
	version: &str,
) -> BTreeMap<String, String> {
	BTreeMap::from([
		("os".to_owned(), m.os().to_owned()),
		("arch".to_owned(), m.arch().to_owned()),
		("target".to_owned(), target.to_owned()),
		("install".to_owned(), install.to_string_lossy().into_owned()),
		("version".to_owned(), version.to_owned()),
	])
}

fn request_version(project: &Project, req: &InstallRequest) -> String {
	req.version
		.clone()
		.or_else(|| project.literal_version().map(str::to_owned))
		.unwrap_or_else(|| "0.0.0".to_owned())
}

/// The Plan phase on its own: the plan an install would run, with no changes made. The plan
/// holds placeholders, so it is the same on every machine.
pub fn preview(
	project: &Project,
	payload: &dyn Payload,
	req: &InstallRequest,
	m: &dyn Machine,
) -> Result<Plan> {
	let selected = select_components(project, &req.target, req.components.as_deref())?;
	let version = request_version(project, req);
	let env = script_env(m, &req.target, &req.install_dir, &version);
	let scripts = ProjectScripts::load(project, env, m.script_fs())?;
	build_plan(project, payload, &req.target, &selected, Some(&scripts))
}

/// The Stage phase: checks every source against the plan's hashes and the free space.
fn stage(
	m: &dyn Machine,
	res: &Resolver,
	payload: Option<&dyn Payload>,
	steps: &[Step],
) -> Result<Totals> {
	let mut totals = Totals { files: 0, bytes: 0 };
	for step in steps {
		match step {
			Step::Apply(Operation::Copy { from, to, sha256 }) => {
				res.resolve(to)?;
				let payload = payload.ok_or_else(|| EngineError::MissingSource(from.clone()))?;
				let data = payload.read(from)?;
				if let Some(expected) = sha256 {
					let actual = li_pack::sha256_hex(&data);
					if !actual.eq_ignore_ascii_case(expected) {
						return Err(EngineError::HashMismatch {
							what: from.clone(),
							expected: expected.clone(),
							actual,
						});
					}
				}
				totals.files += 1;
				totals.bytes += data.len() as u64;
			}
			Step::Apply(op) | Step::Revert(op) => match op {
				Operation::Mkdir { path }
				| Operation::WriteFile { path, .. }
				| Operation::WriteIni { path, .. }
				| Operation::WriteJson { path, .. } => {
					res.resolve(path)?;
				}
				Operation::Copy { to, .. } => {
					res.resolve(to)?;
				}
				_ => {}
			},
			_ => {}
		}
	}
	let have = m.free_space(&res.install);
	if totals.bytes > have {
		return Err(EngineError::InsufficientSpace {
			need: totals.bytes,
			have,
		});
	}
	Ok(totals)
}

/// Runs `steps` as one transaction, then writes the receipt that `receipt_from` builds from the
/// operations the steps effectively performed.
fn run_steps(
	m: &mut dyn Machine,
	res: &Resolver,
	payload: Option<&dyn Payload>,
	product: &str,
	steps: &[Step],
	receipt_from: impl FnOnce(&[Operation], &dyn Machine) -> Option<Receipt>,
	opts: &mut RunOptions<'_>,
) -> Result<Option<Receipt>> {
	let totals = stage(m, res, payload, steps)?;
	let mut session = Session::begin(m, res, payload, product, totals, opts)?;
	let mut effective = Vec::new();
	let run = (|| -> Result<Option<Receipt>> {
		for step in steps {
			effective.extend(session.step(step)?);
		}
		let receipt = receipt_from(&effective, session.machine());
		if let Some(r) = &receipt {
			session.step(&Step::WriteReceipt(Box::new(r.clone())))?;
		}
		Ok(receipt)
	})();
	match run {
		Ok(receipt) => {
			session.finish();
			Ok(receipt)
		}
		Err(e) => Err(session.abort(e)),
	}
}

struct Reconcile<'a> {
	project: &'a Project,
	payload: &'a dyn Payload,
	target: &'a str,
	install_dir: &'a Path,
	scope: Scope,
	selected: Vec<String>,
	version: String,
	feed: Option<String>,
	channel: Option<String>,
	old: Option<Receipt>,
}

/// Brings an install directory to the planned state: installs afresh, or adds, replaces and
/// removes what differs from the old receipt.
fn reconcile(m: &mut dyn Machine, r: Reconcile<'_>, opts: &mut RunOptions<'_>) -> Result<Receipt> {
	let res = Resolver::new(r.install_dir, m.dirs());
	let env = script_env(m, r.target, r.install_dir, &r.version);
	let scripts = ProjectScripts::load(r.project, env, m.script_fs())?;
	let plan = build_plan(r.project, r.payload, r.target, &r.selected, Some(&scripts))?;
	let planned: Vec<Operation> = plan
		.ops
		.into_iter()
		.filter(|o| !matches!(o, Operation::ReceiptWrite { .. }))
		.collect();
	let keys: BTreeSet<String> = planned.iter().map(op_key).collect();

	let old_ops: &[Operation] = r.old.as_ref().map_or(&[], |o| &o.operations);
	let mut steps = Vec::new();
	let mut stale_keys = BTreeSet::new();
	for op in old_ops.iter().rev() {
		let key = op_key(op);
		if !keys.contains(&key) {
			stale_keys.insert(key);
			steps.push(Step::Revert(op.clone()));
		}
	}
	for op in &planned {
		let unchanged = old_ops.iter().any(|o| o == op) && crate::exec::is_intact(m, &res, op);
		if !unchanged {
			steps.push(Step::Apply(op.clone()));
		}
	}

	let product = r.project.installer.product.id.clone();
	let full = full_target(r.project, r.target)?;
	let base = Receipt {
		version: Receipt::VERSION,
		product: product.clone(),
		installed: r.version,
		target: full,
		scope: r.scope,
		feed: r.feed,
		channel: r.channel,
		components: r.selected,
		operations: Vec::new(),
	};
	// Directories are recorded when something created them, and other operations (a shortcut in
	// a new menu directory) may depend on them without the plan naming them. A directory the
	// plan no longer names stays in the receipt unless reverting it really removed it.
	let old_kept: Vec<Operation> = old_ops.to_vec();
	let receipt = run_steps(
		m,
		&res,
		Some(r.payload),
		&product,
		&steps,
		|effective, machine| {
			let mut ops: Vec<Operation> = old_kept
				.into_iter()
				.filter(|o| match o {
					Operation::Mkdir { path } if stale_keys.contains(&op_key(o)) => {
						res.resolve(path).is_ok_and(|p| machine.kind(&p).is_some())
					}
					_ => !stale_keys.contains(&op_key(o)),
				})
				.collect();
			for op in effective {
				let key = op_key(op);
				match ops.iter().position(|o| op_key(o) == key) {
					Some(i) => ops[i] = op.clone(),
					None => ops.push(op.clone()),
				}
			}
			Some(Receipt {
				operations: ops,
				..base
			})
		},
		opts,
	)?;
	receipt.ok_or_else(|| EngineError::Other("no receipt was written".into()))
}

/// Installs a product, or reinstalls over an older receipt (keeping what has not changed).
/// Plan, Stage, Commit and Record run in order; a failure or cancel leaves the machine as it was.
pub fn install(
	m: &mut dyn Machine,
	project: &Project,
	payload: &dyn Payload,
	req: &InstallRequest,
	opts: &mut RunOptions<'_>,
) -> Result<Receipt> {
	let selected = select_components(project, &req.target, req.components.as_deref())?;
	let old = read_receipt(m, &req.install_dir).ok();
	reconcile(
		m,
		Reconcile {
			project,
			payload,
			target: &req.target,
			install_dir: &req.install_dir,
			scope: req.scope,
			selected,
			version: request_version(project, req),
			feed: req.feed.clone(),
			channel: req.channel.clone(),
			old,
		},
		opts,
	)
}

/// Adds or removes components. Everything else about the install is kept.
pub fn modify(
	m: &mut dyn Machine,
	project: &Project,
	payload: &dyn Payload,
	install_dir: &Path,
	components: &[String],
	opts: &mut RunOptions<'_>,
) -> Result<Receipt> {
	let old = read_receipt(m, install_dir)?;
	let target = plain_target(&old.target).to_owned();
	let selected = select_components(project, &target, Some(components))?;
	reconcile(
		m,
		Reconcile {
			project,
			payload,
			target: &target,
			install_dir,
			scope: old.scope,
			selected,
			version: old.installed.clone(),
			feed: old.feed.clone(),
			channel: old.channel.clone(),
			old: Some(old),
		},
		opts,
	)
}

fn version_parts(v: &str) -> Vec<u64> {
	v.split(['.', '-', '+'])
		.map_while(|p| p.parse().ok())
		.collect()
}

/// The release a verified feed offers for an installed product, if it is newer.
pub fn find_update(receipt: &Receipt, feed: &Feed) -> Option<UpdateInfo> {
	let product = feed.products.iter().find(|p| p.id == receipt.product)?;
	let channel = receipt.channel.as_deref().unwrap_or("stable");
	let version = product.channels.get(channel)?;
	if version == &receipt.installed || version_parts(version) < version_parts(&receipt.installed) {
		return None;
	}
	let release = product.releases.iter().find(|r| &r.version == version)?;
	let artefact = release.artefacts.iter().find(|a| {
		a.kind == "payload"
			&& (a.target == receipt.target || a.target == plain_target(&receipt.target))
	})?;
	Some(UpdateInfo {
		version: version.clone(),
		artefact: artefact.clone(),
	})
}

/// Updates to the release a verified feed points at, keeping the installed components, scope and
/// channel. `payload_bytes` is the `.lip` the feed's artefact names; its size and SHA-256 are
/// checked against the feed before anything runs. Callers verify the feed itself with
/// `li_pack::verify_feed`.
pub fn update(
	m: &mut dyn Machine,
	project: &Project,
	install_dir: &Path,
	feed: &Feed,
	payload_bytes: &[u8],
	opts: &mut RunOptions<'_>,
) -> Result<Receipt> {
	let old = read_receipt(m, install_dir)?;
	let info =
		find_update(&old, feed).ok_or_else(|| EngineError::UpToDate(old.installed.clone()))?;
	li_pack::verify_artefact(payload_bytes, &info.artefact)?;
	let payload = LipPayload::from_bytes(payload_bytes)?;
	let target = plain_target(&old.target).to_owned();
	let selected = select_components(project, &target, Some(&old.components))?;
	reconcile(
		m,
		Reconcile {
			project,
			payload: &payload,
			target: &target,
			install_dir,
			scope: old.scope,
			selected,
			version: info.version,
			feed: old.feed.clone(),
			channel: old.channel.clone(),
			old: Some(old),
		},
		opts,
	)
}

/// Checks every recorded operation against the machine and redoes the ones that are missing or
/// changed, using `payload` for file contents. The receipt is left as it was.
pub fn repair(
	m: &mut dyn Machine,
	payload: &dyn Payload,
	install_dir: &Path,
	opts: &mut RunOptions<'_>,
) -> Result<RepairReport> {
	let receipt = read_receipt(m, install_dir)?;
	let res = Resolver::new(install_dir, m.dirs());
	let mut steps = Vec::new();
	let mut restored = Vec::new();
	let mut checked = 0;
	for op in &receipt.operations {
		if matches!(op, Operation::ReceiptWrite { .. }) {
			continue;
		}
		checked += 1;
		if !crate::exec::is_intact(m, &res, op) {
			restored.push(op_key(op));
			steps.push(Step::Apply(op.clone()));
		}
	}
	if !steps.is_empty() {
		run_steps(
			m,
			&res,
			Some(payload),
			&receipt.product,
			&steps,
			|_, _| None,
			opts,
		)?;
	}
	Ok(RepairReport { checked, restored })
}

/// Undoes the recorded operations in reverse and removes the receipt, leaving nothing behind
/// that the install created. Files and directories the user added are left alone. If
/// `project` has `on_uninstall` hooks, their operations run first.
pub fn uninstall(
	m: &mut dyn Machine,
	project: Option<&Project>,
	install_dir: &Path,
	opts: &mut RunOptions<'_>,
) -> Result<()> {
	let receipt = read_receipt(m, install_dir)?;
	let res = Resolver::new(install_dir, m.dirs());
	let mut steps = Vec::new();
	if let Some(project) = project {
		let env = script_env(
			m,
			plain_target(&receipt.target),
			install_dir,
			&receipt.installed,
		);
		let scripts = ProjectScripts::load(project, env, m.script_fs())?;
		let plan = Plan {
			ops: receipt.operations.clone(),
		};
		for op in scripts.on_uninstall(&plan)? {
			steps.push(Step::Apply(op));
		}
	}
	steps.push(Step::RemoveReceipt);
	for op in receipt.operations.iter().rev() {
		if !matches!(op, Operation::ReceiptWrite { .. }) {
			steps.push(Step::Revert(op.clone()));
		}
	}
	run_steps(m, &res, None, &receipt.product, &steps, |_, _| None, opts)?;
	Ok(())
}

/// Starts the installed product: the target of the first recorded shortcut. With a verified
/// `feed`, a newer release is offered first unless `skip_update_offer` is set.
pub fn launch(
	m: &mut dyn Machine,
	install_dir: &Path,
	feed: Option<&Feed>,
	skip_update_offer: bool,
) -> Result<LaunchOutcome> {
	let receipt = read_receipt(m, install_dir)?;
	if !skip_update_offer && let Some(info) = feed.and_then(|f| find_update(&receipt, f)) {
		return Ok(LaunchOutcome::UpdateAvailable(info.version));
	}
	let res = Resolver::new(install_dir, m.dirs());
	let target = receipt
		.operations
		.iter()
		.find_map(|o| match o {
			Operation::Shortcut { target, .. } => Some(target.clone()),
			_ => None,
		})
		.ok_or_else(|| EngineError::Other("the receipt records nothing to launch".into()))?;
	let path = res.resolve(&target)?;
	m.launch(&path, &[]).map_err(|e| EngineError::io(path, e))?;
	Ok(LaunchOutcome::Launched)
}
