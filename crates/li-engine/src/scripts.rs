// Loading a project's Lua scripts and running their plan hooks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::sync::Arc;

use li_model::{Operation, Plan};
use li_script::{Limits, Script, ScriptContext, ScriptFs, UiItem};

use crate::error::Result;
use crate::project::Project;

/// Every script in `installer/scripts/`, loaded once.
pub struct ProjectScripts {
	scripts: BTreeMap<String, Script>,
}

impl ProjectScripts {
	/// Loads each script with the given environment values and read-only file access.
	pub fn load(
		project: &Project,
		env: BTreeMap<String, String>,
		fs: Arc<dyn ScriptFs>,
	) -> Result<Self> {
		let mut scripts = BTreeMap::new();
		for (name, source) in &project.scripts {
			let ctx = ScriptContext {
				env: env.clone(),
				fs: fs.clone(),
				net: None,
			};
			scripts.insert(
				name.clone(),
				Script::load(name, source, ctx, Limits::default())?,
			);
		}
		Ok(ProjectScripts { scripts })
	}

	pub fn empty() -> Self {
		ProjectScripts {
			scripts: BTreeMap::new(),
		}
	}

	/// A script by its project-relative path, such as `scripts/setup.lua`.
	pub fn get(&self, name: &str) -> Option<&Script> {
		self.scripts.get(name)
	}

	/// Runs every script's `on_commit`, in file name order, and gathers the operations.
	pub fn on_commit(&self, plan: &Plan) -> Result<Vec<Operation>> {
		let mut ops = Vec::new();
		for s in self.scripts.values() {
			ops.extend(s.on_commit(plan)?);
		}
		Ok(ops)
	}

	pub fn on_uninstall(&self, plan: &Plan) -> Result<Vec<Operation>> {
		let mut ops = Vec::new();
		for s in self.scripts.values() {
			ops.extend(s.on_uninstall(plan)?);
		}
		Ok(ops)
	}

	/// Notes and dialogs the scripts asked for since the last call.
	pub fn take_ui(&self) -> Vec<UiItem> {
		self.scripts.values().flat_map(Script::take_ui).collect()
	}
}
