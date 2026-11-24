// The Studio: one project, one undo history and one event stream for every client.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;
use std::sync::Mutex;

use li_model::{Flow, Layout, Node};
use serde::Serialize;
use serde_json::Value;

use crate::backend::{Preview, Problem, ScriptReport, StudioBackend};
use crate::checks::{check_project, flow_problems};
use crate::error::{ErrorKind, StudioError};
use crate::history::{Entry, EntrySummary, History, Source};
use crate::project::{Doc, DocKey, Project};
use crate::resolve::{merge_patch, node_at_mut, resolve_for_renderer};

/// Raised after every change to the project, whoever made it.
#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ChangeEvent {
	/// `open`, `edit`, `undo` or `redo`.
	#[ts(type = "'open' | 'edit' | 'undo' | 'redo'")]
	pub kind: &'static str,
	pub doc: Option<DocKey>,
	pub label: String,
	pub source: Source,
	pub can_undo: bool,
	pub can_redo: bool,
}

type Listener = Box<dyn Fn(&ChangeEvent) + Send + Sync>;

struct Inner {
	project: Option<Project>,
	history: History,
	preview: Option<Preview>,
}

pub struct Studio {
	inner: Mutex<Inner>,
	pub(crate) backend: Box<dyn StudioBackend>,
	pub(crate) sessions: Mutex<crate::tui::Sessions>,
	listeners: Mutex<Vec<Listener>>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TargetInfo {
	pub full_id: String,
	pub id: String,
	pub renderer: String,
	pub theme: Option<String>,
	pub outputs: Vec<String>,
	pub components: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ComponentInfo {
	pub id: String,
	pub name: String,
	pub required: bool,
	/// Whether the component starts ticked; `false` for an optional component the person ticks.
	pub default: bool,
	pub targets: Vec<String>,
	pub files: usize,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ThemeInfo {
	pub id: String,
	pub name: String,
	pub renderers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct FlowInfo {
	pub id: String,
	pub steps: usize,
	pub errors: usize,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ProjectSummary {
	pub root: String,
	pub id: String,
	pub name: String,
	pub publisher: String,
	pub version: String,
	pub default_theme: Option<String>,
	pub targets: Vec<TargetInfo>,
	pub components: Vec<ComponentInfo>,
	pub themes: Vec<ThemeInfo>,
	pub flows: Vec<FlowInfo>,
	pub layouts: Vec<String>,
	pub scripts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct FlowView {
	pub flow: Flow,
	pub errors: Vec<Problem>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct HistoryView {
	pub entries: Vec<EntrySummary>,
	pub can_undo: bool,
	pub can_redo: bool,
}

impl Studio {
	pub fn new(backend: Box<dyn StudioBackend>) -> Self {
		Studio {
			inner: Mutex::new(Inner {
				project: None,
				history: History::default(),
				preview: None,
			}),
			backend,
			sessions: Mutex::new(crate::tui::Sessions::default()),
			listeners: Mutex::new(Vec::new()),
		}
	}

	/// Registers a callback for every change; the Tauri shell forwards these to the UI.
	pub fn on_change(&self, f: impl Fn(&ChangeEvent) + Send + Sync + 'static) {
		self.listeners.lock().expect("listeners").push(Box::new(f));
	}

	fn emit(&self, event: ChangeEvent) {
		for l in self.listeners.lock().expect("listeners").iter() {
			l(&event);
		}
	}

	pub(crate) fn with_project<T>(
		&self,
		f: impl FnOnce(&Project) -> Result<T, StudioError>,
	) -> Result<T, StudioError> {
		let inner = self.inner.lock().expect("studio");
		f(inner.project.as_ref().ok_or(StudioError::NoProject)?)
	}

	pub fn open(&self, path: &Path) -> Result<ProjectSummary, StudioError> {
		let project = Project::open(path)?;
		{
			let mut inner = self.inner.lock().expect("studio");
			inner.project = Some(project);
			inner.history.clear();
			inner.preview = None;
		}
		self.emit(ChangeEvent {
			kind: "open",
			doc: None,
			label: format!("Opened {}", path.display()),
			source: Source::Ui,
			can_undo: false,
			can_redo: false,
		});
		self.project_summary()
	}

	pub fn is_open(&self) -> bool {
		self.inner.lock().expect("studio").project.is_some()
	}

	pub fn project_summary(&self) -> Result<ProjectSummary, StudioError> {
		self.with_project(|p| {
			let product = &p.installer.product;
			Ok(ProjectSummary {
				root: p.root.display().to_string(),
				id: product.id.clone(),
				name: product.name.clone(),
				publisher: product.publisher.clone(),
				version: match &product.version {
					li_model::project::VersionSource::Literal(v) => v.clone(),
					li_model::project::VersionSource::From { from } => format!("from {from}"),
				},
				default_theme: p.installer.defaults.theme.clone(),
				targets: target_infos(p),
				components: p
					.components
					.components
					.iter()
					.map(|c| ComponentInfo {
						id: c.id.clone(),
						name: c.name.clone(),
						required: c.required,
						default: c.default,
						targets: c.targets.clone(),
						files: c.files.len(),
					})
					.collect(),
				themes: p
					.themes
					.values()
					.map(|t| ThemeInfo {
						id: t.theme.id.clone(),
						name: t.theme.name.clone(),
						renderers: t.theme.renderers.clone(),
					})
					.collect(),
				flows: p
					.flows
					.values()
					.map(|f| FlowInfo {
						id: f.id.clone(),
						steps: f.steps.len(),
						errors: f.validate().len(),
					})
					.collect(),
				layouts: p.layouts.keys().cloned().collect(),
				scripts: p.scripts.keys().cloned().collect(),
			})
		})
	}

	pub fn list_targets(&self) -> Result<Vec<TargetInfo>, StudioError> {
		self.with_project(|p| Ok(target_infos(p)))
	}

	pub fn get_flow(&self, id: &str) -> Result<FlowView, StudioError> {
		self.with_project(|p| {
			let flow = p
				.flows
				.get(id)
				.ok_or_else(|| StudioError::NotFound(format!("flow `{id}`")))?;
			Ok(FlowView {
				flow: flow.clone(),
				errors: flow_problems(flow),
			})
		})
	}

	pub fn get_layout(&self, id: &str, renderer: Option<&str>) -> Result<Layout, StudioError> {
		self.with_project(|p| {
			let layout = p
				.layouts
				.get(id)
				.ok_or_else(|| StudioError::NotFound(format!("layout `{id}`")))?;
			let mut out = layout.clone();
			if let Some(r) = renderer {
				out.root = resolve_for_renderer(&layout.root, r);
			}
			Ok(out)
		})
	}

	pub fn read_script(&self, path: &str) -> Result<String, StudioError> {
		self.with_project(|p| {
			p.scripts
				.get(path)
				.cloned()
				.ok_or_else(|| StudioError::NotFound(format!("script `{path}`")))
		})
	}

	/// Records and writes one change. Returns false, recording nothing, when the document is
	/// already as asked.
	pub(crate) fn apply(
		&self,
		key: DocKey,
		after: Option<Doc>,
		label: &str,
		source: Source,
	) -> Result<bool, StudioError> {
		let event = {
			let mut inner = self.inner.lock().expect("studio");
			let project = inner.project.as_mut().ok_or(StudioError::NoProject)?;
			let before = project.get(&key);
			if before == after {
				return Ok(false);
			}
			project.put(&key, after.clone())?;
			inner.history.record(Entry {
				label: label.to_owned(),
				source,
				key: key.clone(),
				before,
				after,
			});
			inner.preview = None;
			ChangeEvent {
				kind: "edit",
				doc: Some(key),
				label: label.to_owned(),
				source,
				can_undo: inner.history.can_undo(),
				can_redo: inner.history.can_redo(),
			}
		};
		self.emit(event);
		Ok(true)
	}

	pub fn set_flow(
		&self,
		id: &str,
		flow: Flow,
		label: &str,
		source: Source,
	) -> Result<FlowView, StudioError> {
		if flow.id != id {
			return Err(StudioError::invalid(
				"flow",
				"its id does not match the request",
			));
		}
		self.apply(
			DocKey::Flow(id.into()),
			Some(Doc::Flow(flow)),
			label,
			source,
		)?;
		self.get_flow(id)
	}

	pub fn patch_flow(
		&self,
		id: &str,
		patch: &Value,
		label: &str,
		source: Source,
	) -> Result<FlowView, StudioError> {
		let current = self.get_flow(id)?.flow;
		let mut value =
			serde_json::to_value(&current).map_err(|e| StudioError::invalid("flow", e))?;
		merge_patch(&mut value, patch);
		let flow: Flow =
			serde_json::from_value(value).map_err(|e| StudioError::invalid("flow patch", e))?;
		self.set_flow(id, flow, label, source)
	}

	pub fn set_layout(
		&self,
		id: &str,
		layout: Layout,
		label: &str,
		source: Source,
	) -> Result<Layout, StudioError> {
		if layout.id != id {
			return Err(StudioError::invalid(
				"layout",
				"its id does not match the request",
			));
		}
		self.apply(
			DocKey::Layout(id.into()),
			Some(Doc::Layout(layout)),
			label,
			source,
		)?;
		self.get_layout(id, None)
	}

	pub fn patch_layout(
		&self,
		id: &str,
		patch: &Value,
		label: &str,
		source: Source,
	) -> Result<Layout, StudioError> {
		let current = self.get_layout(id, None)?;
		let mut value =
			serde_json::to_value(&current).map_err(|e| StudioError::invalid("layout", e))?;
		merge_patch(&mut value, patch);
		let layout: Layout =
			serde_json::from_value(value).map_err(|e| StudioError::invalid("layout patch", e))?;
		self.set_layout(id, layout, label, source)
	}

	/// Replaces the node at `path` (child indexes from the root), or, with `renderer`, sets or
	/// removes that renderer's override of it.
	pub fn set_layout_node(
		&self,
		id: &str,
		path: &[usize],
		renderer: Option<&str>,
		node: Option<Node>,
		label: &str,
		source: Source,
	) -> Result<Layout, StudioError> {
		let mut layout = self.get_layout(id, None)?;
		let target = node_at_mut(&mut layout.root, path)
			.ok_or_else(|| StudioError::NotFound(format!("node {path:?} in layout `{id}`")))?;
		match (renderer, node) {
			(Some(r), Some(n)) => {
				target.overrides.insert(r.to_owned(), n);
			}
			(Some(r), None) => {
				target.overrides.remove(r);
			}
			(None, Some(n)) => {
				let overrides = std::mem::take(&mut target.overrides);
				*target = n;
				if target.overrides.is_empty() {
					target.overrides = overrides;
				}
			}
			(None, None) => {
				return Err(StudioError::invalid(
					"node",
					"a node is required without a renderer",
				));
			}
		}
		self.set_layout(id, layout, label, source)
	}

	pub fn write_script(
		&self,
		path: &str,
		source_text: &str,
		label: &str,
		source: Source,
	) -> Result<(), StudioError> {
		self.apply(
			DocKey::Script(path.into()),
			Some(Doc::Script(source_text.to_owned())),
			label,
			source,
		)?;
		Ok(())
	}

	pub fn test_script(
		&self,
		path: &str,
		source: Option<&str>,
		target: Option<&str>,
	) -> Result<ScriptReport, StudioError> {
		let inner = self.inner.lock().expect("studio");
		let project = inner.project.as_ref().ok_or(StudioError::NoProject)?;
		let text = match source {
			Some(s) => s.to_owned(),
			None => project
				.scripts
				.get(path)
				.cloned()
				.ok_or_else(|| StudioError::NotFound(format!("script `{path}`")))?,
		};
		self.backend
			.test_script(project, path, &text, target)
			.map_err(|e| StudioError::new(ErrorKind::Script, e))
	}

	/// Checks Lua source without running it: syntax errors, and warnings about capabilities,
	/// removed globals and what the retro subset lacks. Needs no project.
	pub fn check_script_syntax(&self, source: &str) -> Vec<crate::ScriptDiagnostic> {
		li_script::check::check(source)
	}

	pub fn run_preview(&self, target: &str) -> Result<Preview, StudioError> {
		let mut inner = self.inner.lock().expect("studio");
		let project = inner.project.as_ref().ok_or(StudioError::NoProject)?;
		let preview = self.backend.run_preview(project, target)?;
		inner.preview = Some(preview.clone());
		Ok(preview)
	}

	/// The last preview, or `None` if none was run since the project last changed.
	pub fn get_preview(&self) -> Result<Option<Preview>, StudioError> {
		let inner = self.inner.lock().expect("studio");
		if inner.project.is_none() {
			return Err(StudioError::NoProject);
		}
		Ok(inner.preview.clone())
	}

	pub fn check_project(&self) -> Result<Vec<Problem>, StudioError> {
		self.with_project(|p| Ok(self.backend.check(p)?))
	}

	pub fn history(&self) -> HistoryView {
		let inner = self.inner.lock().expect("studio");
		HistoryView {
			entries: inner.history.summaries(),
			can_undo: inner.history.can_undo(),
			can_redo: inner.history.can_redo(),
		}
	}

	pub fn undo(&self, source: Source) -> Result<HistoryView, StudioError> {
		self.step(true, source)
	}

	pub fn redo(&self, source: Source) -> Result<HistoryView, StudioError> {
		self.step(false, source)
	}

	fn step(&self, undo: bool, source: Source) -> Result<HistoryView, StudioError> {
		let event = {
			let mut inner = self.inner.lock().expect("studio");
			if inner.project.is_none() {
				return Err(StudioError::NoProject);
			}
			let entry = if undo {
				inner.history.undo().cloned()
			} else {
				inner.history.redo().cloned()
			}
			.ok_or(StudioError::NothingTo(if undo { "undo" } else { "redo" }))?;
			let doc = if undo {
				entry.before.clone()
			} else {
				entry.after.clone()
			};
			let result = inner
				.project
				.as_mut()
				.expect("checked above")
				.put(&entry.key, doc);
			if let Err(e) = result {
				// Put the cursor back so history still matches the files.
				if undo {
					inner.history.redo();
				} else {
					inner.history.undo();
				}
				return Err(e);
			}
			inner.preview = None;
			ChangeEvent {
				kind: if undo { "undo" } else { "redo" },
				doc: Some(entry.key),
				label: entry.label,
				source,
				can_undo: inner.history.can_undo(),
				can_redo: inner.history.can_redo(),
			}
		};
		self.emit(event);
		Ok(self.history())
	}

	/// Runs every check the Studio knows without the backend, for callers that have no engine.
	pub fn local_check(&self) -> Result<Vec<Problem>, StudioError> {
		self.with_project(|p| Ok(check_project(p)))
	}
}

fn target_infos(p: &Project) -> Vec<TargetInfo> {
	p.installer
		.targets
		.iter()
		.map(|t| TargetInfo {
			full_id: t.full_id(),
			id: t.id.clone(),
			renderer: t.renderer.clone(),
			theme: p.theme_for(t),
			outputs: t.outputs.clone(),
			components: p
				.components
				.components
				.iter()
				.filter(|c| c.applies_to(&t.id))
				.map(|c| c.id.clone())
				.collect(),
		})
		.collect()
}

#[cfg(test)]
pub(crate) mod testutil {
	use std::fs;
	use std::path::{Path, PathBuf};

	use super::Studio;
	use crate::mock::MockBackend;

	pub fn copy_dir(from: &Path, to: &Path) {
		fs::create_dir_all(to).unwrap();
		for e in fs::read_dir(from).unwrap() {
			let e = e.unwrap();
			let dest = to.join(e.file_name());
			if e.file_type().unwrap().is_dir() {
				copy_dir(&e.path(), &dest);
			} else {
				fs::copy(e.path(), dest).unwrap();
			}
		}
	}

	/// A studio open on a scratch copy of an example, so tests can write freely.
	pub fn open_example(name: &str) -> (tempfile::TempDir, Studio, PathBuf) {
		let src = Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../../examples")
			.join(name);
		let dir = tempfile::tempdir().unwrap();
		copy_dir(&src, dir.path());
		let studio = Studio::new(Box::new(MockBackend));
		studio.open(dir.path()).unwrap();
		let root = dir.path().join("installer");
		(dir, studio, root)
	}
}

#[cfg(test)]
mod tests {
	use std::fs;
	use std::sync::{Arc, Mutex};

	use serde_json::json;

	use super::testutil::open_example;
	use super::*;

	#[test]
	fn edits_write_files_and_undo_restores_them() {
		let (_d, s, root) = open_example("planboard");
		let path = root.join("flows/install.flow.json");
		let original = fs::read_to_string(&path).unwrap();
		let mut flow = s.get_flow("install").unwrap().flow;
		flow.steps.retain(|st| st.id != "licence");
		flow.steps[0].next = Some("folder".into());
		s.set_flow("install", flow, "Remove licence", Source::Ui)
			.unwrap();
		assert_ne!(fs::read_to_string(&path).unwrap(), original);
		assert!(!fs::read_to_string(&path).unwrap().contains("\"licence\""));

		s.undo(Source::Ui).unwrap();
		assert_eq!(fs::read_to_string(&path).unwrap(), original);
		let h = s.redo(Source::Ui).unwrap();
		assert!(!h.can_redo && h.can_undo);
		assert!(!fs::read_to_string(&path).unwrap().contains("\"licence\""));
	}

	#[test]
	fn ui_and_mcp_changes_share_one_history_and_events() {
		let (_d, s, _root) = open_example("neutral");
		let events = Arc::new(Mutex::new(Vec::new()));
		let sink = events.clone();
		s.on_change(move |e| {
			sink.lock()
				.unwrap()
				.push((e.kind, e.source, e.label.clone()))
		});

		s.write_script(
			"scripts/a.lua",
			"function detect(page) end\n",
			"UI edit",
			Source::Ui,
		)
		.unwrap();
		s.write_script(
			"scripts/a.lua",
			"function validate(page) return {} end\n",
			"Agent edit",
			Source::Mcp,
		)
		.unwrap();
		let labels: Vec<_> = s
			.history()
			.entries
			.iter()
			.map(|e| (e.label.clone(), e.source))
			.collect();
		assert_eq!(
			labels,
			[
				("UI edit".into(), Source::Ui),
				("Agent edit".into(), Source::Mcp)
			]
		);
		// A person can undo the agent's change.
		s.undo(Source::Ui).unwrap();
		assert_eq!(
			s.read_script("scripts/a.lua").unwrap(),
			"function detect(page) end\n"
		);
		s.undo(Source::Ui).unwrap();
		assert!(s.read_script("scripts/a.lua").is_err());

		let seen = events.lock().unwrap();
		let kinds: Vec<_> = seen.iter().map(|e| (e.0, e.1)).collect();
		assert_eq!(
			kinds,
			[
				("edit", Source::Ui),
				("edit", Source::Mcp),
				("undo", Source::Ui),
				("undo", Source::Ui)
			]
		);
	}

	#[test]
	fn undoing_a_created_script_removes_the_file() {
		let (_d, s, root) = open_example("neutral");
		s.write_script("scripts/new.lua", "-- x\n", "Add", Source::Ui)
			.unwrap();
		assert!(root.join("scripts/new.lua").exists());
		s.undo(Source::Ui).unwrap();
		assert!(!root.join("scripts/new.lua").exists());
	}

	#[test]
	fn no_op_edits_record_nothing() {
		let (_d, s, _r) = open_example("neutral");
		let flow = s.get_flow("install").unwrap().flow;
		s.set_flow("install", flow, "Same", Source::Ui).unwrap();
		assert!(s.history().entries.is_empty());
		assert!(matches!(
			s.undo(Source::Ui),
			Err(StudioError::NothingTo("undo"))
		));
	}

	#[test]
	fn invalid_flows_are_saved_and_reported() {
		let (_d, s, _r) = open_example("neutral");
		let view = s
			.patch_flow("install", &json!({"steps": []}), "Empty", Source::Mcp)
			.unwrap();
		assert_eq!(view.errors.len(), 1);
		assert!(view.errors[0].message.contains("no steps"));
	}

	#[test]
	fn rejects_mismatched_ids_and_unsafe_paths() {
		let (_d, s, _r) = open_example("neutral");
		let flow = s.get_flow("install").unwrap().flow;
		assert!(s.set_flow("update", flow, "x", Source::Ui).is_err());
		assert!(
			s.write_script("scripts/../evil.lua", "x", "x", Source::Mcp)
				.is_err()
		);
	}

	#[test]
	fn layout_overrides_can_be_set_and_cleared_for_one_renderer() {
		let (_d, s, root) = open_example("neutral");
		let retro = s.get_layout("welcome", Some("tui")).unwrap();
		assert_eq!(retro.root.kind, "window");
		let node = Node {
			kind: "dialog".into(),
			bind: None,
			props: Default::default(),
			children: vec![],
			overrides: Default::default(),
		};
		s.set_layout_node(
			"welcome",
			&[],
			Some("web"),
			Some(node),
			"Web override",
			Source::Ui,
		)
		.unwrap();
		assert_eq!(
			s.get_layout("welcome", Some("web")).unwrap().root.kind,
			"dialog"
		);
		assert_eq!(
			s.get_layout("welcome", Some("canvas")).unwrap().root.kind,
			"window"
		);
		assert!(
			fs::read_to_string(root.join("layouts/welcome.layout.json"))
				.unwrap()
				.contains("\"web\"")
		);
		s.set_layout_node("welcome", &[], Some("web"), None, "Clear", Source::Ui)
			.unwrap();
		assert_eq!(
			s.get_layout("welcome", Some("web")).unwrap().root.kind,
			"window"
		);
		assert!(
			s.set_layout_node("welcome", &[9, 9], None, None, "x", Source::Ui)
				.is_err()
		);
	}

	#[test]
	fn a_themes_own_layout_is_listed_and_saved_in_its_theme_folder() {
		let (_d, s, root) = open_example("lt-fizz");
		let ids = s.project_summary().unwrap().layouts;
		assert!(ids.contains(&"fizz-win95/welcome".to_owned()));
		assert!(ids.contains(&"welcome".to_owned()));
		let mut layout = s.get_layout("fizz-win95/welcome", None).unwrap();
		layout.root.props.insert("title".into(), "Edited".into());
		s.set_layout("fizz-win95/welcome", layout, "Edit", Source::Ui)
			.unwrap();
		let file =
			fs::read_to_string(root.join("themes/fizz-win95/layouts/welcome.layout.json")).unwrap();
		assert!(file.contains("Edited") && file.contains("\"id\": \"welcome\""));
	}

	#[test]
	fn preview_is_dropped_when_the_project_changes() {
		let (_d, s, _r) = open_example("lt-fizz");
		s.run_preview("dos-x86:tui-retro").unwrap();
		assert!(s.get_preview().unwrap().is_some());
		s.write_script("scripts/a.lua", "-- x\n", "Edit", Source::Ui)
			.unwrap();
		assert!(s.get_preview().unwrap().is_none());
	}

	#[test]
	fn summary_lists_targets_with_their_components() {
		let (_d, s, _r) = open_example("lt-fizz");
		let sum = s.project_summary().unwrap();
		let dos = sum.targets.iter().find(|t| t.id == "dos-x86").unwrap();
		assert!(dos.components.contains(&"sound".to_string()));
		let win = sum
			.targets
			.iter()
			.find(|t| t.id == "windows-x86_64")
			.unwrap();
		assert!(!win.components.contains(&"sound".to_string()));
		assert_eq!(sum.flows.len(), 5);
	}

	/// A backend that always fails, to reach the errors the mock never raises.
	struct Failing;

	impl crate::StudioBackend for Failing {
		fn check(&self, _: &Project) -> Result<Vec<Problem>, crate::BackendError> {
			Err(crate::BackendError("the engine stopped".into()))
		}
		fn run_preview(&self, _: &Project, _: &str) -> Result<Preview, crate::BackendError> {
			Err(crate::BackendError("the engine stopped".into()))
		}
		fn test_script(
			&self,
			_: &Project,
			_: &str,
			_: &str,
			_: Option<&str>,
		) -> Result<ScriptReport, crate::BackendError> {
			Err(crate::BackendError("the sandbox would not start".into()))
		}
	}

	fn kind<T>(r: Result<T, StudioError>) -> (ErrorKind, String) {
		let e = r.err().expect("the call should fail");
		let p = e.payload();
		(p.kind, p.message)
	}

	#[test]
	fn each_kind_of_failure_reaches_the_caller_with_its_kind() {
		let none = Studio::new(Box::new(crate::MockBackend));
		assert_eq!(kind(none.project_summary()).0, ErrorKind::Project);
		assert_eq!(
			kind(none.open(Path::new("/no/such/project"))).0,
			ErrorKind::NotFound
		);

		let (dir, s, root) = open_example("neutral");
		assert_eq!(kind(s.get_flow("nope")).0, ErrorKind::NotFound);
		assert_eq!(kind(s.undo(Source::Ui)).0, ErrorKind::Project);
		let flow = s.get_flow("install").unwrap().flow;
		assert_eq!(
			kind(s.set_flow("other", flow, "Edit", Source::Ui)).0,
			ErrorKind::InvalidInput
		);
		assert_eq!(
			kind(s.write_script("../escape.lua", "x", "Edit", Source::Ui)).0,
			ErrorKind::InvalidInput
		);
		assert_eq!(kind(s.run_preview("nowhere:tui")).0, ErrorKind::Backend);

		// A file where a folder is needed makes the write fail.
		let scripts = root.join("scripts");
		std::fs::create_dir_all(&scripts).unwrap();
		std::fs::write(scripts.join("blocker"), "x").unwrap();
		assert_eq!(
			kind(s.write_script("scripts/blocker/a.lua", "-- x\n", "Edit", Source::Ui)).0,
			ErrorKind::Io
		);

		let failing = Studio::new(Box::new(Failing));
		failing.open(&root).unwrap();
		let (k, message) = kind(failing.test_script("scripts/a.lua", Some("x"), None));
		assert_eq!(
			(k, message.as_str()),
			(ErrorKind::Script, "the sandbox would not start")
		);
		assert_eq!(kind(failing.check_project()).0, ErrorKind::Backend);
		drop(dir);
	}

	#[test]
	fn calls_fail_cleanly_without_a_project() {
		let s = Studio::new(Box::new(crate::MockBackend));
		assert!(matches!(s.project_summary(), Err(StudioError::NoProject)));
		assert!(matches!(s.undo(Source::Ui), Err(StudioError::NoProject)));
	}
}
