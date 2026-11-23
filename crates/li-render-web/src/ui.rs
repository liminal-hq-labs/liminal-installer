// The web renderer's `Ui`: each page goes to the webview as an event and the flow waits for its answer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use li_engine::{
	Action, CondContext, Machine, Progress, RealMachine, State, Ui, cond, expand_sources,
};
use li_model::{Component, Layout, Node, Step, StepType};
use li_runner::{RunnerError, RunnerResult, UiContext, UiProvider};
use serde_json::{Map, Value, json};

use crate::look::{look_of, window_size};
use crate::tree::{TreeCtx, prepare_tree};
use crate::wire::{
	PageAnswer, PageKind, PageUpdate, PageView, ProgressView, RunComponent, RunEvent, RunInfo,
	RunProduct, WindowSize,
};

/// Progress events closer together than this are dropped, except the last of a commit.
const PROGRESS_EVERY: Duration = Duration::from_millis(30);

/// What the session handle shares with the page loop: the cancel flag and the page on show.
#[derive(Default)]
pub struct Shared {
	pub cancel: AtomicBool,
	pub current: Mutex<Option<Current>>,
}

/// The page on show, kept so a change can be answered without asking the flow.
pub struct Current {
	pub tree: Option<Node>,
	pub base: Arc<Base>,
}

/// What stays the same for the whole run.
pub struct Base {
	pub info: RunInfo,
	components: Vec<Component>,
	root: PathBuf,
	renderer: String,
	target: Value,
	system: Value,
	/// Per component: how many files it installs and how many bytes.
	sizes: BTreeMap<String, (u64, u64)>,
}

/// `a.b.c` keys as nested objects too, so `state.licence.accepted` reads a bound `licence.accepted`.
fn nest(state: &Map<String, Value>) -> Value {
	let mut out = state.clone();
	for (key, value) in state {
		let mut parts = key.split('.').peekable();
		let Some(first) = parts.next() else { continue };
		if parts.peek().is_none() {
			continue;
		}
		let mut at = out
			.entry(first.to_owned())
			.and_modify(|v| {
				if !v.is_object() {
					*v = json!({});
				}
			})
			.or_insert_with(|| json!({}));
		while let Some(part) = parts.next() {
			let Some(map) = at.as_object_mut() else { break };
			if parts.peek().is_none() {
				map.insert(part.to_owned(), value.clone());
				break;
			}
			at = map
				.entry(part.to_owned())
				.and_modify(|v| {
					if !v.is_object() {
						*v = json!({});
					}
				})
				.or_insert_with(|| json!({}));
		}
	}
	Value::Object(out)
}

fn human(bytes: u64) -> String {
	const KB: f64 = 1024.0;
	let b = bytes as f64;
	if b < KB * KB {
		format!("{:.0} KB", (b / KB).max(if bytes > 0 { 1.0 } else { 0.0 }))
	} else if b < KB * KB * KB {
		let mb = b / (KB * KB);
		if mb < 10.0 {
			format!("{mb:.1} MB")
		} else {
			format!("{mb:.0} MB")
		}
	} else {
		format!("{:.1} GB", b / (KB * KB * KB))
	}
}

fn strings(v: Option<&Value>) -> Option<Vec<String>> {
	Some(
		v?.as_array()?
			.iter()
			.filter_map(|s| s.as_str().map(str::to_owned))
			.collect(),
	)
}

impl Base {
	/// The component ids a state selects: `components.selected`, else `components` (a list or a
	/// comma-separated string), else every component; required ones are always in.
	fn selected(&self, state: &Map<String, Value>) -> Vec<String> {
		let chosen = strings(state.get("components.selected"))
			.or_else(|| strings(state.get("components")))
			.or_else(|| {
				state.get("components").and_then(Value::as_str).map(|s| {
					s.split(',')
						.map(str::trim)
						.filter(|s| !s.is_empty())
						.map(str::to_owned)
						.collect()
				})
			});
		self.components
			.iter()
			.filter(|c| c.required || chosen.as_ref().is_none_or(|ids| ids.contains(&c.id)))
			.map(|c| c.id.clone())
			.collect()
	}

	/// The values of the `{placeholders}` for a state.
	pub fn vars(&self, state: &Map<String, Value>) -> BTreeMap<String, String> {
		let dir = state
			.get("install.path")
			.or_else(|| state.get("install_dir"))
			.and_then(Value::as_str)
			.map_or_else(|| self.info.install_dir.clone(), str::to_owned);
		let selected = self.selected(state);
		let (files, bytes) = selected
			.iter()
			.filter_map(|id| self.sizes.get(id))
			.fold((0, 0), |a, s| (a.0 + s.0, a.1 + s.1));
		let free = RealMachine::from_environment().free_space(Path::new(&dir));
		let free_text = |b: u64| {
			if b == u64::MAX {
				"—".to_owned()
			} else {
				human(b)
			}
		};
		let update = state.get("update");
		let text = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or_default().to_owned();
		let p = &self.info.product;
		[
			("product.id", p.id.clone()),
			("product.name", p.name.clone()),
			("product.publisher", p.publisher.clone()),
			("product.version", p.version.clone()),
			("target.id", self.info.target.clone()),
			("target.renderer", self.renderer.clone()),
			("install.path", dir),
			("install.size", human(bytes)),
			("system.free_space", free_text(free)),
			(
				"system.free_space_after",
				free_text(if free == u64::MAX {
					free
				} else {
					free.saturating_sub(bytes)
				}),
			),
			(
				"plan.summary",
				format!(
					"{} components · {files} files · {}",
					selected.len(),
					human(bytes)
				),
			),
			("sound.detected", text(state.get("sound.detected"))),
			("feed.latest", text(update.and_then(|u| u.get("version")))),
			(
				"receipt.installed",
				text(update.and_then(|u| u.get("current"))),
			),
			("feed.notes", text(update.and_then(|u| u.get("notes")))),
			("progress.file", String::new()),
			("progress.percent", "0".into()),
			("validate.problems", String::new()),
		]
		.into_iter()
		.map(|(k, v)| (k.to_owned(), v))
		.collect()
	}

	/// The paths of the nodes whose `enabled_when` does not hold for `state`.
	pub fn disabled(&self, tree: &Node, state: &Map<String, Value>) -> Vec<String> {
		let selected = self.selected(state);
		let ctx = CondContext {
			target: self.target.clone(),
			components: json!(selected),
			state: nest(state),
			system: self.system.clone(),
		};
		let mut out = Vec::new();
		walk(tree, String::new(), &mut |node, path| {
			if let Some(expr) = node.props.get("enabled_when").and_then(Value::as_str) {
				// A condition that does not parse or run leaves the control on.
				if !cond::eval_bool(expr, &ctx).unwrap_or(true) {
					out.push(path.to_owned());
				}
			}
		});
		out
	}
}

/// Visits every node with its path: child indexes from the root joined by `.`.
pub fn walk(node: &Node, path: String, f: &mut dyn FnMut(&Node, &str)) {
	f(node, &path);
	for (i, c) in node.children.iter().enumerate() {
		let child = if path.is_empty() {
			i.to_string()
		} else {
			format!("{path}.{i}")
		};
		walk(c, child, f);
	}
}

/// Whether every button that moves forward is switched off, so Next cannot be pressed.
pub fn next_is_disabled(tree: &Node, disabled: &[String]) -> bool {
	let (mut any, mut on) = (false, false);
	walk(tree, String::new(), &mut |node, path| {
		let forward = node.kind == "button"
			&& matches!(
				node.props.get("action").and_then(Value::as_str),
				Some("next" | "commit" | "finish" | "skip")
			);
		if forward {
			any = true;
			on |= !disabled.iter().any(|d| d == path);
		}
	});
	any && !on
}

/// The page state the webview binds to: the engine's `install_dir` and `components` under the
/// keys layouts use.
fn page_state(base: &Base, state: &State) -> Map<String, Value> {
	let mut out = state.clone();
	if let Some(dir) = state.get("install_dir") {
		out.insert("install.path".into(), dir.clone());
	}
	out.insert("components.selected".into(), json!(base.selected(state)));
	out
}

/// Takes the person's answers back into the engine's state.
fn absorb(answer: &Map<String, Value>, state: &mut State) {
	for (k, v) in answer {
		match k.as_str() {
			"install.path" => {
				state.insert("install_dir".into(), v.clone());
			}
			"components.selected" => {
				state.insert("components".into(), v.clone());
			}
			_ => {}
		}
		state.insert(k.clone(), v.clone());
	}
}

fn action_for(answer: &str) -> Action {
	match answer {
		"back" => Action::Back,
		"cancel" => Action::Cancel,
		_ => Action::Next,
	}
}

/// Draws the flow in a webview: it sends each page as a [`RunEvent`] and waits for the person's
/// [`PageAnswer`]. The commit page does not wait; its meters follow the progress events.
pub struct WebUi {
	base: Arc<Base>,
	shared: Arc<Shared>,
	events: Sender<RunEvent>,
	answers: Receiver<PageAnswer>,
	problems: Vec<String>,
	index: u32,
	initial_free: Option<u64>,
	last_sent: Option<Instant>,
}

impl WebUi {
	fn send(&self, event: RunEvent) -> bool {
		self.events.send(event).is_ok()
	}

	fn view(
		&mut self,
		step: &Step,
		layout: Option<&Layout>,
		state: &State,
		kind: PageKind,
	) -> bool {
		self.index += 1;
		let shown = page_state(&self.base, state);
		let tree = layout.map(|l| {
			let components: Vec<&Component> = self.base.components.iter().collect();
			prepare_tree(
				&l.root,
				&TreeCtx {
					renderer: &self.base.renderer,
					components: &components,
					state: &shown,
					root: &self.base.root,
				},
			)
		});
		let mut vars = self.base.vars(&shown);
		vars.insert("validate.problems".into(), self.problems.join(" · "));
		let disabled = tree
			.as_ref()
			.map(|t| self.base.disabled(t, &shown))
			.unwrap_or_default();
		if let Ok(mut cur) = self.shared.current.lock() {
			*cur = Some(Current {
				tree: tree.clone(),
				base: self.base.clone(),
			});
		}
		let page = PageView {
			step: step.id.clone(),
			kind,
			index: self.index,
			layout: tree,
			state: shown,
			vars,
			problems: std::mem::take(&mut self.problems),
			disabled,
		};
		self.send(RunEvent::Page { page })
	}
}

impl Ui for WebUi {
	fn page(&mut self, step: &Step, layout: Option<&Layout>, state: &mut State) -> Action {
		// An answer left over from an earlier page (a cancel pressed during the commit) is stale.
		while self.answers.try_recv().is_ok() {}
		let kind = if step.kind == StepType::Finish {
			PageKind::Finish
		} else {
			PageKind::Page
		};
		if !self.view(step, layout, state, kind) {
			return Action::Cancel;
		}
		match self.answers.recv() {
			Ok(answer) => {
				absorb(&answer.state, state);
				action_for(&answer.action)
			}
			Err(_) => Action::Cancel,
		}
	}

	fn problems(&mut self, _: &Step, problems: &[String]) {
		self.problems = problems.to_vec();
	}

	fn commit_started(&mut self, step: &Step, layout: Option<&Layout>) {
		self.initial_free = None;
		self.last_sent = None;
		let state = State::new();
		self.view(step, layout, &state, PageKind::Commit);
	}

	fn progress(&mut self, p: &Progress) -> bool {
		if self.shared.cancel.load(Ordering::Relaxed) {
			return false;
		}
		let initial = *self.initial_free.get_or_insert(p.free_space);
		let last = p.files_total > 0 && p.files_done >= p.files_total;
		if !last && self.last_sent.is_some_and(|t| t.elapsed() < PROGRESS_EVERY) {
			return true;
		}
		self.last_sent = Some(Instant::now());
		let file = p
			.current_file
			.as_deref()
			.map(|f| f.strip_prefix("{install}/").unwrap_or(f).to_owned());
		let total = p.fraction();
		let free_fraction = if initial == 0 || initial == u64::MAX {
			1.0
		} else {
			(p.free_space as f64 / initial as f64).clamp(0.0, 1.0)
		};
		let vars = BTreeMap::from([
			("progress.file".to_owned(), file.clone().unwrap_or_default()),
			(
				"progress.percent".to_owned(),
				format!("{}", (total * 100.0).round() as u32),
			),
		]);
		self.send(RunEvent::Progress {
			progress: ProgressView {
				phase: format!("{:?}", p.phase).to_lowercase(),
				file,
				files_done: p.files_done.min(u64::from(u32::MAX)) as u32,
				files_total: p.files_total.min(u64::from(u32::MAX)) as u32,
				total,
				file_fraction: if p.files_total > 0 {
					p.files_done as f64 / p.files_total as f64
				} else {
					total
				},
				free_fraction,
				low_space: p.low_space,
				vars,
			},
		})
	}
}

/// Hands the lifecycle commands a [`WebUi`] wired to a session.
pub struct WebProvider {
	pub(crate) events: Sender<RunEvent>,
	pub(crate) answers: Option<Receiver<PageAnswer>>,
	pub(crate) shared: Arc<Shared>,
}

impl WebProvider {
	fn make(&mut self, cx: &UiContext) -> RunnerResult<Box<dyn Ui>> {
		let answers = self
			.answers
			.take()
			.ok_or_else(|| RunnerError::new("the web runner shows one flow per run"))?;
		let project = cx.project;
		let components: Vec<Component> = project
			.components
			.components
			.iter()
			.filter(|c| c.applies_to(&cx.target.id))
			.cloned()
			.collect();
		let mut sizes = BTreeMap::new();
		if let Some(payload) = cx.payload {
			for c in &components {
				let mut total = (0u64, 0u64);
				for rule in &c.files {
					for (from, _) in expand_sources(payload, &rule.from) {
						total.0 += 1;
						total.1 += payload.size(&from).unwrap_or(0);
					}
				}
				sizes.insert(c.id.clone(), total);
			}
		}
		let theme = project.theme_id_for(cx.target).map(str::to_owned);
		let look = look_of(project, cx.target);
		let (width, height) = window_size(look);
		let p = &project.installer.product;
		let info = RunInfo {
			mode: cx.mode.into(),
			product: RunProduct {
				id: p.id.clone(),
				name: p.name.clone(),
				publisher: p.publisher.clone(),
				version: cx.version.to_owned(),
			},
			target: cx.target.full_id(),
			renderer: cx.target.renderer.clone(),
			theme,
			look: look.to_owned(),
			window: WindowSize { width, height },
			install_dir: cx.install_dir.to_string_lossy().into_owned(),
			components: components
				.iter()
				.map(|c| RunComponent {
					id: c.id.clone(),
					name: c.name.clone(),
					required: c.required,
				})
				.collect(),
		};
		let base = Arc::new(Base {
			info: info.clone(),
			components,
			root: project.dir.clone(),
			renderer: cx.target.renderer.clone(),
			target: json!({
				"id": cx.target.id,
				"renderer": cx.target.renderer,
				"os": cx.machine.os(),
				"arch": cx.machine.arch(),
			}),
			system: li_engine::system_info(cx.machine, cx.install_dir),
			sizes,
		});
		if let Ok(mut cur) = self.shared.current.lock() {
			*cur = Some(Current {
				tree: None,
				base: base.clone(),
			});
		}
		let _ = self.events.send(RunEvent::Info { info });
		Ok(Box::new(WebUi {
			base,
			shared: self.shared.clone(),
			events: self.events.clone(),
			answers,
			problems: Vec::new(),
			index: 0,
			initial_free: None,
			last_sent: None,
		}))
	}
}

impl UiProvider for WebProvider {
	fn page_ui(&mut self, cx: &UiContext) -> RunnerResult<Box<dyn Ui>> {
		self.make(cx)
	}

	fn status_ui(&mut self, cx: &UiContext) -> RunnerResult<Box<dyn Ui>> {
		self.make(cx)
	}
}

impl Shared {
	/// Answers a change on the page: which nodes are switched off now and the values that follow.
	pub fn update(&self, state: &Map<String, Value>) -> PageUpdate {
		let cur = self.current.lock().ok();
		let Some(cur) = cur.as_deref().and_then(Option::as_ref) else {
			return PageUpdate {
				disabled: Vec::new(),
				vars: BTreeMap::new(),
			};
		};
		PageUpdate {
			disabled: cur
				.tree
				.as_ref()
				.map(|t| cur.base.disabled(t, state))
				.unwrap_or_default(),
			vars: cur.base.vars(state),
		}
	}
}
