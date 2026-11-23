// The messages between the flow session and the webview, and the headless driver's report.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

use li_model::Node;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// What the run does, as the runner's subcommand names it.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
	Install,
	Update,
	Modify,
	Repair,
	Uninstall,
}

impl From<li_runner::Mode> for RunMode {
	fn from(m: li_runner::Mode) -> Self {
		match m {
			li_runner::Mode::Install => RunMode::Install,
			li_runner::Mode::Update => RunMode::Update,
			li_runner::Mode::Modify => RunMode::Modify,
			li_runner::Mode::Repair => RunMode::Repair,
			li_runner::Mode::Uninstall => RunMode::Uninstall,
		}
	}
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunProduct {
	pub id: String,
	pub name: String,
	pub publisher: String,
	pub version: String,
}

/// A component the flow may offer.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunComponent {
	pub id: String,
	pub name: String,
	pub required: bool,
}

/// The size, in CSS pixels, the window opens at.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSize {
	pub width: u32,
	pub height: u32,
}

/// What the webview learns first: who is being installed, how, and how large the window should be.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunInfo {
	pub mode: RunMode,
	pub product: RunProduct,
	/// The full target id, such as `windows-x86_64:web`.
	pub target: String,
	pub renderer: String,
	pub theme: Option<String>,
	/// The look the renderer draws: `afterglow`, `neutral`, `win11`, `win95` or `dos`.
	pub look: String,
	pub window: WindowSize,
	pub install_dir: String,
	pub components: Vec<RunComponent>,
}

/// What kind of screen a page is. A `commit` page does not wait for an answer: progress events
/// move its meters until the work is done.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
	Page,
	Commit,
	Finish,
}

/// One screen of the flow: the page's layout tree (overrides applied, lists expanded, the theme's
/// stage composed around it) and the values it shows.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageView {
	pub step: String,
	pub kind: PageKind,
	/// How many pages the flow has shown, counting this one; the first is 1.
	pub index: u32,
	#[cfg_attr(feature = "ts", ts(optional))]
	pub layout: Option<Node>,
	/// The page's values under the keys its controls bind, such as `install.path`.
	#[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
	pub state: Map<String, Value>,
	/// The `{placeholder}` values: `product.name`, `install.path`, `install.size` and so on.
	pub vars: BTreeMap<String, String>,
	/// What validation rejected the last time the person pressed Next.
	pub problems: Vec<String>,
	/// The paths of the nodes switched off by `enabled_when`, as child indexes from the root joined
	/// by `.` (the root is the empty string).
	pub disabled: Vec<String>,
}

/// Where the commit is: the readings a `meter` or `progress` node draws.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgressView {
	pub phase: String,
	/// The file being copied, relative to the install directory.
	#[cfg_attr(feature = "ts", ts(optional))]
	pub file: Option<String>,
	pub files_done: u32,
	pub files_total: u32,
	/// Overall progress, 0 to 1.
	pub total: f64,
	/// Files copied out of the files to copy, 0 to 1.
	pub file_fraction: f64,
	/// Free space left on the volume, as a fraction of what it started with.
	pub free_fraction: f64,
	pub low_space: bool,
	pub vars: BTreeMap<String, String>,
}

/// What the webview is told, in order.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
	/// First: the run's product, mode and window.
	Info { info: RunInfo },
	/// A screen to show. Waits for a [`PageAnswer`] unless it is a `commit` page.
	Page { page: PageView },
	/// Progress during the commit, or the work of a run with no flow.
	Progress { progress: ProgressView },
	/// The flow finished and the work is done.
	Done,
	/// The person cancelled; every change was rolled back.
	Cancelled,
	/// The run failed; the engine rolled every change back.
	Failed { message: String },
}

/// What the person did on a page: the button they pressed and the values their controls hold.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageAnswer {
	/// `next`, `back` or `cancel`; `commit`, `finish`, `skip` and anything else mean `next`.
	pub action: String,
	#[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
	pub state: Map<String, Value>,
}

/// The answer to a change on the page: which nodes are switched off now, and the values that follow.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageUpdate {
	pub disabled: Vec<String>,
	pub vars: BTreeMap<String, String>,
}
