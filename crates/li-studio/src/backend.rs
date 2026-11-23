// The thin trait between the Studio and the installer engine.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_model::Plan;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::project::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
	Error,
	Warning,
}

/// One finding from a check, a flow validation or a dry run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Problem {
	pub severity: Severity,
	/// What it concerns, such as `flow:install`, `layout:welcome`, `target:dos-x86` or
	/// `script:scripts/sound-setup.lua`.
	pub scope: String,
	pub message: String,
	/// The flow step involved, when there is one.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub step: Option<String>,
}

impl Problem {
	pub fn error(scope: impl Into<String>, message: impl Into<String>) -> Self {
		Problem {
			severity: Severity::Error,
			scope: scope.into(),
			message: message.into(),
			step: None,
		}
	}

	pub fn warning(scope: impl Into<String>, message: impl Into<String>) -> Self {
		Problem {
			severity: Severity::Warning,
			..Problem::error(scope, message)
		}
	}

	pub fn at_step(mut self, step: impl Into<String>) -> Self {
		self.step = Some(step.into());
		self
	}
}

/// An image of one step. `encoding` is `utf8` for SVG text and `base64` for PNG.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screenshot {
	pub mime: String,
	pub encoding: String,
	pub data: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewStep {
	pub id: String,
	/// The step type: `page`, `action`, `branch`, `commit` or `finish`.
	pub kind: String,
	pub layout: Option<String>,
	/// True when the step's `when` condition is false for this target.
	pub skipped: bool,
	pub screenshot: Option<Screenshot>,
}

/// The result of a dry run of one target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preview {
	/// The `os-arch:renderer` target that was run.
	pub target: String,
	pub plan: Plan,
	pub problems: Vec<Problem>,
	pub steps: Vec<PreviewStep>,
}

/// A hook from the spec's hook table, and whether the script defines it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookInfo {
	pub name: String,
	pub when: String,
	pub present: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptReport {
	pub ok: bool,
	pub hooks: Vec<HookInfo>,
	/// The `li.*` modules the script asks for.
	pub capabilities: Vec<String>,
	pub diagnostics: Vec<Problem>,
	pub output: Vec<String>,
}

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[error("{0}")]
pub struct BackendError(pub String);

/// What the Studio needs from the engine. The real engine implements this against `li-engine`
/// and `li-script`; [`crate::MockBackend`] stands in until then. Methods take the whole open
/// project so an implementation may read `project.root` from disk instead of the loaded copy.
pub trait StudioBackend: Send + Sync {
	/// The same checks as `liminst check`.
	fn check(&self, project: &Project) -> Result<Vec<Problem>, BackendError>;

	/// Dry-runs the install flow for a target given as `os-arch:renderer` and returns the plan,
	/// the problems found and an image of each step.
	fn run_preview(&self, project: &Project, target: &str) -> Result<Preview, BackendError>;

	/// Loads `source` as the script at `path` and runs its tests. `source` may differ from the
	/// file on disk, so unsaved edits can be tested.
	fn test_script(
		&self,
		project: &Project,
		path: &str,
		source: &str,
	) -> Result<ScriptReport, BackendError>;
}
