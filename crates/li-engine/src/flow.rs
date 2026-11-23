// The flow runner: walks a flow's steps, asking a `Ui` for each page.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

use li_model::{Flow, Layout, Step, StepType};
use li_script::Page;
use serde_json::Value;

use crate::cond::{self, CondContext};
use crate::error::{EngineError, Result};
use crate::progress::Progress;
use crate::scripts::ProjectScripts;

/// What a flow has collected so far. Page controls write here through their `bind`; the
/// `components` key, a list of component ids, is also what conditions see as `components`.
pub type State = serde_json::Map<String, Value>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
	Next,
	Back,
	Cancel,
}

/// A renderer plugs in here.
pub trait Ui {
	/// Shows a page (or the finish page) and returns what the person did. The UI edits `state`
	/// in place. `layout` is the page's layout for the target, when the project has one.
	fn page(&mut self, step: &Step, layout: Option<&Layout>, state: &mut State) -> Action;

	/// Called when `validate` rejects a page; the page is then shown again.
	fn problems(&mut self, _step: &Step, _problems: &[String]) {}

	/// Called when the commit step begins, so a renderer can show its progress page.
	fn commit_started(&mut self, _step: &Step, _layout: Option<&Layout>) {}

	/// Called for every progress event during commit. Return `false` to cancel.
	fn progress(&mut self, _progress: &Progress) -> bool {
		true
	}
}

/// A `Ui` that answers every page from a table of default answers and presses Next.
#[derive(Debug, Clone, Default)]
pub struct HeadlessUi {
	pub answers: State,
	/// The ids of the pages shown, in order.
	pub pages: Vec<String>,
	pub problems: Vec<String>,
}

impl HeadlessUi {
	pub fn new(answers: State) -> Self {
		HeadlessUi {
			answers,
			..Self::default()
		}
	}
}

impl Ui for HeadlessUi {
	fn page(&mut self, step: &Step, _: Option<&Layout>, state: &mut State) -> Action {
		self.pages.push(step.id.clone());
		for (k, v) in &self.answers {
			state.insert(k.clone(), v.clone());
		}
		Action::Next
	}

	fn problems(&mut self, _: &Step, problems: &[String]) {
		self.problems.extend(problems.iter().cloned());
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowOutcome {
	Finished,
	Cancelled,
}

/// What conditions see besides the state.
#[derive(Debug, Clone, Default)]
pub struct FlowEnv {
	/// `target.id`, `target.renderer` and so on.
	pub target: Value,
	/// `system.os`, `system.free_space`, `system.installed`; see `system_info`.
	pub system: Value,
}

/// The work of a commit step: receives the state and a progress callback.
pub type CommitFn<'c> = dyn FnMut(&State, &mut dyn FnMut(&Progress) -> bool) -> Result<()> + 'c;

pub struct FlowRunner<'a> {
	flow: &'a Flow,
	layouts: Option<&'a BTreeMap<String, Layout>>,
	scripts: Option<&'a ProjectScripts>,
	env: FlowEnv,
	pub state: State,
	/// Ids of the steps run, in order, including skipped-over action and branch steps but not
	/// steps whose `when` was false.
	pub visited: Vec<String>,
}

const MAX_STEPS: usize = 10_000;
const MAX_RETRIES: usize = 100;

impl<'a> FlowRunner<'a> {
	pub fn new(flow: &'a Flow, env: FlowEnv) -> Self {
		FlowRunner {
			flow,
			layouts: None,
			scripts: None,
			env,
			state: State::new(),
			visited: Vec::new(),
		}
	}

	pub fn with_layouts(mut self, layouts: &'a BTreeMap<String, Layout>) -> Self {
		self.layouts = Some(layouts);
		self
	}

	pub fn with_scripts(mut self, scripts: &'a ProjectScripts) -> Self {
		self.scripts = Some(scripts);
		self
	}

	pub fn with_state(mut self, state: State) -> Self {
		self.state = state;
		self
	}

	fn ctx(&self) -> CondContext {
		CondContext {
			target: self.env.target.clone(),
			components: self
				.state
				.get("components")
				.cloned()
				.unwrap_or_else(|| Value::Array(Vec::new())),
			state: Value::Object(self.state.clone()),
			system: self.env.system.clone(),
		}
	}

	fn holds(&self, when: &str) -> Result<bool> {
		Ok(cond::eval_bool(when, &self.ctx())?)
	}

	fn layout(&self, step: &Step) -> Option<&'a Layout> {
		self.layouts?.get(step.layout.as_deref()?)
	}

	fn script_for<'s>(&self, step: &'s Step) -> Result<Option<(&'a li_script::Script, &'s str)>> {
		let Some(spec) = &step.on_enter else {
			return Ok(None);
		};
		let (path, hook) = spec
			.split_once('#')
			.ok_or_else(|| EngineError::Flow(format!("on_enter `{spec}` needs `path#hook`")))?;
		let script = self
			.scripts
			.and_then(|s| s.get(path))
			.ok_or_else(|| EngineError::Flow(format!("script `{path}` is not loaded")))?;
		Ok(Some((script, hook)))
	}

	fn enter(&mut self, step: &Step) -> Result<()> {
		if let Some((script, hook)) = self.script_for(step)? {
			if hook != "detect" {
				return Err(EngineError::Flow(format!(
					"on_enter hook `{hook}` is not `detect`"
				)));
			}
			let mut page = Page {
				id: step.id.clone(),
				fields: self.state.clone(),
			};
			script.detect(&mut page)?;
			self.state = page.fields;
		}
		Ok(())
	}

	fn validate(&self, step: &Step) -> Result<Vec<String>> {
		match self.script_for(step)? {
			Some((script, _)) => Ok(script.validate(&Page {
				id: step.id.clone(),
				fields: self.state.clone(),
			})?),
			None => Ok(Vec::new()),
		}
	}

	fn after(&self, step: &Step) -> Result<String> {
		step.next
			.clone()
			.ok_or_else(|| EngineError::Flow(format!("step `{}` has no next step", step.id)))
	}

	/// Walks the flow from its start. Pages go to `ui`; the commit step calls `commit`.
	pub fn run(&mut self, ui: &mut dyn Ui, commit: &mut CommitFn<'_>) -> Result<FlowOutcome> {
		let mut cur = self
			.flow
			.start_id()
			.ok_or_else(|| EngineError::Flow("flow has no steps".into()))?
			.to_owned();
		let mut history: Vec<String> = Vec::new();
		let mut count = 0usize;
		'steps: loop {
			count += 1;
			if count > MAX_STEPS {
				return Err(EngineError::Flow("flow did not finish".into()));
			}
			let step = self
				.flow
				.step(&cur)
				.ok_or_else(|| EngineError::Flow(format!("no step `{cur}`")))?;
			if let Some(w) = &step.when
				&& !self.holds(w)?
			{
				cur = self.after(step)?;
				continue;
			}
			self.visited.push(step.id.clone());
			match step.kind {
				StepType::Page => {
					self.enter(step)?;
					let mut retries = 0;
					loop {
						match ui.page(step, self.layout(step), &mut self.state) {
							Action::Cancel => return Ok(FlowOutcome::Cancelled),
							Action::Back => {
								if let Some(prev) = history.pop() {
									cur = prev;
									continue 'steps;
								}
							}
							Action::Next => {
								let problems = self.validate(step)?;
								if problems.is_empty() {
									break;
								}
								ui.problems(step, &problems);
								retries += 1;
								if retries > MAX_RETRIES {
									return Err(EngineError::Flow(format!(
										"page `{}` kept failing validation: {}",
										step.id,
										problems.join("; ")
									)));
								}
							}
						}
					}
					history.push(step.id.clone());
					cur = self.after(step)?;
				}
				StepType::Action => {
					self.enter(step)?;
					cur = self.after(step)?;
				}
				StepType::Branch => {
					let mut chosen = None;
					for b in &step.branches {
						if self.holds(&b.when)? {
							chosen = Some(b.next.clone());
							break;
						}
					}
					cur = match chosen {
						Some(n) => n,
						None => self.after(step)?,
					};
				}
				StepType::Commit => {
					ui.commit_started(step, self.layout(step));
					let mut report = |p: &Progress| ui.progress(p);
					commit(&self.state, &mut report)?;
					history.clear();
					cur = self.after(step)?;
				}
				StepType::Finish => {
					ui.page(step, self.layout(step), &mut self.state);
					return Ok(FlowOutcome::Finished);
				}
			}
		}
	}
}
