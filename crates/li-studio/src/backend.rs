// The thin trait between the Studio and the installer engine.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

use li_model::{Layout, Node, Plan};
use li_render_tui::Look;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::project::Project;

#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
	Error,
	Warning,
}

/// One finding from a check, a flow validation or a dry run.
#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Problem {
	pub severity: Severity,
	/// What it concerns, such as `flow:install`, `layout:welcome`, `target:dos-x86` or
	/// `script:scripts/sound-setup.lua`.
	pub scope: String,
	pub message: String,
	/// The flow step involved, when there is one.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[ts(optional)]
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
#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screenshot {
	pub mime: String,
	#[ts(type = "'utf8' | 'base64'")]
	pub encoding: String,
	pub data: String,
}

#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewStep {
	pub id: String,
	/// The step type: `page`, `action`, `branch`, `commit` or `finish`.
	pub kind: String,
	pub layout: Option<String>,
	/// True when the step's `when` condition is false for this target.
	pub skipped: bool,
	/// The tui text version of the screen, as an image. Only the `tui` renderers draw it; the
	/// Studio draws every other renderer from `layout_tree`.
	pub screenshot: Option<Screenshot>,
	/// The layout as the target's renderer draws it: its `@renderer` overrides applied, string
	/// references such as `items = "components"` expanded and document `source` files read in.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[ts(optional)]
	pub layout_tree: Option<Node>,
	/// The page state when the step is shown, including seeded meter readings.
	#[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
	#[ts(optional, type = "Record<string, unknown>")]
	pub state: serde_json::Map<String, serde_json::Value>,
}

/// The product a preview was run for, for the placeholders a screen shows.
#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PreviewProduct {
	pub id: String,
	pub name: String,
	pub version: String,
	pub publisher: String,
}

/// The result of a dry run of one target.
#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preview {
	/// The `os-arch:renderer` target that was run.
	pub target: String,
	/// The target's renderer, which picks the look the Studio draws.
	#[serde(default)]
	pub renderer: String,
	/// The target's theme.
	#[serde(default)]
	pub theme: Option<String>,
	#[serde(default)]
	pub product: PreviewProduct,
	/// Sample values for the `{placeholders}` a screen shows, such as `product.version`,
	/// `target.id` and `install.size`.
	#[serde(default)]
	pub vars: BTreeMap<String, String>,
	pub plan: Plan,
	pub problems: Vec<Problem>,
	pub steps: Vec<PreviewStep>,
}

/// A hook from the spec's hook table, and whether the script defines it.
#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookInfo {
	pub name: String,
	pub when: String,
	pub present: bool,
}

#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptReport {
	pub ok: bool,
	pub hooks: Vec<HookInfo>,
	/// The `li.*` modules the script's declared capabilities open.
	pub capabilities: Vec<String>,
	pub diagnostics: Vec<Problem>,
	pub output: Vec<String>,
	/// The `os-arch:renderer` target the test ran against, if one was named.
	#[serde(default)]
	pub target: Option<String>,
	/// One result per hook the script defines, in the order they ran.
	#[serde(default)]
	pub tests: Vec<HookRun>,
}

/// The outcome of calling one hook against the sample.
#[derive(ts_rs::TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookRun {
	pub hook: String,
	pub ok: bool,
	/// One line: what the hook did, or the error that stopped it.
	pub summary: String,
	/// What it listed: the fields it set, the problems it returned, the operations it added, the notes it showed.
	pub lines: Vec<String>,
}

/// A layout resolved for the terminal renderer: what a live terminal session draws.
#[derive(Debug, Clone)]
pub struct TuiPage {
	/// The layout with the `tui` renderer's overrides applied and references expanded.
	pub layout: Layout,
	pub look: Look,
}

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[error("{0}")]
pub struct BackendError(pub String);

/// What the Studio needs from the engine. [`crate::EngineBackend`] implements this against `li-engine`,
/// `li-script` and `li-render-tui`; [`crate::MockBackend`] stands in for tests and the browser demo. Methods take the whole open
/// project so an implementation may read `project.root` from disk instead of the loaded copy.
pub trait StudioBackend: Send + Sync {
	/// The same checks as `liminst check`.
	fn check(&self, project: &Project) -> Result<Vec<Problem>, BackendError>;

	/// Dry-runs the install flow for a target given as `os-arch:renderer` and returns the plan,
	/// the problems found and an image of each step.
	fn run_preview(&self, project: &Project, target: &str) -> Result<Preview, BackendError>;

	/// Loads `source` as the script at `path` and runs its tests. `source` may differ from the
	/// file on disk, so unsaved edits can be tested. `target` (`os-arch:renderer`) picks the
	/// machine, components and install folder the sample is built from; `None` uses a fixed sample.
	fn test_script(
		&self,
		project: &Project,
		path: &str,
		source: &str,
		target: Option<&str>,
	) -> Result<ScriptReport, BackendError>;

	/// Resolves a page for a live terminal session. `page` is a step of the install flow, or a
	/// layout id (`welcome`, or `theme/welcome` for a theme's own). Only `tui` targets have one.
	/// `state` is the page state the screen starts from.
	fn tui_page(
		&self,
		_project: &Project,
		_target: &str,
		_page: &str,
		_state: &serde_json::Map<String, serde_json::Value>,
	) -> Result<TuiPage, BackendError> {
		Err(BackendError(
			"this backend has no terminal renderer".to_owned(),
		))
	}
}
