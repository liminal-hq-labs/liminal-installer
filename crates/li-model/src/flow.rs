// Flows: directed graphs of steps with the validity rules from the spec.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flow {
	#[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
	pub schema: Option<String>,
	pub version: u32,
	pub id: String,
	/// The first step; defaults to the first in `steps`.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub start: Option<String>,
	pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepType {
	Page,
	Action,
	Branch,
	Commit,
	Finish,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
	pub id: String,
	#[serde(rename = "type")]
	pub kind: StepType,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub layout: Option<String>,
	/// Condition over `target`, `components`, `state` and `system`; the step is skipped when false.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub when: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub on_enter: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub next: Option<String>,
	/// For `branch` steps: first matching condition wins; `next` is the fallback.
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub branches: Vec<Branch>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
	pub when: String,
	pub next: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FlowError {
	#[error("duplicate step id `{0}`")]
	DuplicateStep(String),
	#[error("step `{from}` points at unknown step `{to}`")]
	UnknownStep { from: String, to: String },
	#[error("step `{0}` has no next step and is not a finish step")]
	DeadEnd(String),
	#[error("a flow needs exactly one commit step, found {0}")]
	CommitCount(usize),
	#[error("step `{0}` cannot reach a finish step")]
	NoFinish(String),
	#[error("flow has no steps")]
	Empty,
}

impl Flow {
	pub fn start_id(&self) -> Option<&str> {
		self.start
			.as_deref()
			.or_else(|| self.steps.first().map(|s| s.id.as_str()))
	}

	pub fn step(&self, id: &str) -> Option<&Step> {
		self.steps.iter().find(|s| s.id == id)
	}

	/// Checks the rules every flow must meet: unique ids, no dangling or dead-end steps, exactly
	/// one `commit`, and a `finish` reachable from every step.
	pub fn validate(&self) -> Vec<FlowError> {
		let mut errors = Vec::new();
		if self.steps.is_empty() {
			return vec![FlowError::Empty];
		}
		let mut ids = BTreeSet::new();
		for step in &self.steps {
			if !ids.insert(step.id.as_str()) {
				errors.push(FlowError::DuplicateStep(step.id.clone()));
			}
		}
		let commits = self
			.steps
			.iter()
			.filter(|s| s.kind == StepType::Commit)
			.count();
		if commits != 1 {
			errors.push(FlowError::CommitCount(commits));
		}
		let mut edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
		for step in &self.steps {
			let targets: Vec<&str> = step
				.next
				.iter()
				.map(String::as_str)
				.chain(step.branches.iter().map(|b| b.next.as_str()))
				.collect();
			if targets.is_empty() && step.kind != StepType::Finish {
				errors.push(FlowError::DeadEnd(step.id.clone()));
			}
			for to in &targets {
				if !ids.contains(to) {
					errors.push(FlowError::UnknownStep {
						from: step.id.clone(),
						to: (*to).to_owned(),
					});
				}
			}
			edges.insert(step.id.as_str(), targets);
		}
		// Walk backwards from every finish step to find the steps that can reach one.
		let mut reaches: BTreeSet<&str> = self
			.steps
			.iter()
			.filter(|s| s.kind == StepType::Finish)
			.map(|s| s.id.as_str())
			.collect();
		loop {
			let before = reaches.len();
			for (from, tos) in &edges {
				if tos.iter().any(|t| reaches.contains(t)) {
					reaches.insert(from);
				}
			}
			if reaches.len() == before {
				break;
			}
		}
		for step in &self.steps {
			if !reaches.contains(step.id.as_str()) {
				errors.push(FlowError::NoFinish(step.id.clone()));
			}
		}
		errors
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn step(id: &str, kind: StepType, next: Option<&str>) -> Step {
		Step {
			id: id.into(),
			kind,
			layout: None,
			when: None,
			on_enter: None,
			next: next.map(Into::into),
			branches: vec![],
		}
	}

	fn flow(steps: Vec<Step>) -> Flow {
		Flow {
			schema: None,
			version: 1,
			id: "install".into(),
			start: None,
			steps,
		}
	}

	#[test]
	fn accepts_a_valid_flow() {
		let f = flow(vec![
			step("welcome", StepType::Page, Some("copy")),
			step("copy", StepType::Commit, Some("done")),
			step("done", StepType::Finish, None),
		]);
		assert!(f.validate().is_empty());
	}

	#[test]
	fn rejects_two_commits_and_dead_ends() {
		let f = flow(vec![
			step("a", StepType::Commit, Some("b")),
			step("b", StepType::Commit, None),
		]);
		let errors = f.validate();
		assert!(errors.contains(&FlowError::CommitCount(2)));
		assert!(errors.contains(&FlowError::DeadEnd("b".into())));
	}

	#[test]
	fn rejects_unknown_targets_and_cycles_without_finish() {
		let f = flow(vec![
			step("a", StepType::Page, Some("b")),
			step("b", StepType::Commit, Some("a")),
			step("done", StepType::Finish, None),
			step("c", StepType::Page, Some("nowhere")),
		]);
		let errors = f.validate();
		assert!(errors.contains(&FlowError::NoFinish("a".into())));
		assert!(
			errors
				.iter()
				.any(|e| matches!(e, FlowError::UnknownStep { .. }))
		);
	}
}
