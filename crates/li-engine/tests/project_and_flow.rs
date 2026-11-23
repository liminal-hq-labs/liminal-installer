// Project loading, checks and the flow runner.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use li_engine::{
	Action, FlowEnv, FlowOutcome, FlowRunner, HeadlessUi, ProjectScripts, Severity, SimMachine,
	State, Ui, check,
};
use li_model::{Layout, Step};
use serde_json::json;

fn codes(project: &li_engine::Project) -> Vec<&'static str> {
	check(project).into_iter().map(|d| d.code).collect()
}

#[test]
fn the_fixture_loads_and_checks_clean() {
	let p = project();
	assert_eq!(p.installer.product.id, "hq.liminal.fixture");
	assert_eq!(p.components.components.len(), 3);
	assert!(p.flows.contains_key("install"));
	assert!(p.themes["fixture"].layouts.contains_key("welcome"));
	assert!(p.scripts.contains_key("scripts/setup.lua"));
	let diags = check(&p);
	assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn check_reports_problems() {
	let base = project();

	let mut p = base.clone();
	p.flows.get_mut("install").unwrap().steps[0].layout = Some("nope".into());
	assert!(codes(&p).contains(&"unknown-layout"));

	let mut p = base.clone();
	p.flows.get_mut("install").unwrap().steps[0].when = Some("len(components) > 1".into());
	assert!(codes(&p).contains(&"bad-condition"));

	let mut p = base.clone();
	p.flows.get_mut("install").unwrap().steps[2].branches[0].when = "'ghost' in components".into();
	assert!(codes(&p).contains(&"unknown-component"));

	let mut p = base.clone();
	p.flows.get_mut("install").unwrap().steps[0].next = Some("nowhere".into());
	assert!(codes(&p).contains(&"bad-flow"));

	let mut p = base.clone();
	p.components.components[1].targets = vec!["amiga-m68k".into()];
	assert!(codes(&p).contains(&"unknown-target"));

	let mut p = base.clone();
	p.components.components[0].files[0].from = "dist/{target}/MISSING".into();
	let d = check(&p);
	assert!(
		d.iter()
			.any(|d| d.code == "missing-source" && d.severity == Severity::Error),
		"{d:?}"
	);

	let mut p = base.clone();
	p.flows.get_mut("install").unwrap().steps[1].on_enter = Some("scripts/gone.lua#detect".into());
	assert!(codes(&p).contains(&"missing-script"));

	let mut p = base.clone();
	p.installer.targets[0].theme = Some("elsewhere".into());
	let d = check(&p);
	assert!(
		d.iter()
			.any(|d| d.code == "unknown-theme" && d.severity == Severity::Warning)
	);
}

fn runner_setup() -> (li_engine::Project, ProjectScripts) {
	let p = project();
	let sim = SimMachine::new();
	let env = [("os".to_owned(), "linux".to_owned())].into();
	let scripts = ProjectScripts::load(&p, env, li_engine::Machine::script_fs(&sim)).unwrap();
	(p, scripts)
}

fn env() -> FlowEnv {
	FlowEnv {
		target: json!({"id": "linux-x86_64", "renderer": "canvas"}),
		system: json!({"os": "linux"}),
	}
}

#[test]
fn headless_flow_follows_branches_and_when() {
	let (p, scripts) = runner_setup();
	let flow = &p.flows["install"];
	let layouts = &p.themes["fixture"].layouts;

	// With sound selected the branch visits `sound-info`.
	let mut answers = State::new();
	answers.insert("components".into(), json!(["game", "sound"]));
	let mut ui = HeadlessUi::new(answers);
	let mut runner = FlowRunner::new(flow, env())
		.with_layouts(layouts)
		.with_scripts(&scripts);
	let mut committed = 0;
	let mut commit = |state: &State, _: &mut dyn FnMut(&li_engine::Progress) -> bool| {
		committed += 1;
		assert_eq!(state["detected_os"], "linux", "on_enter detect ran");
		Ok(())
	};
	let outcome = runner.run(&mut ui, &mut commit).unwrap();
	assert_eq!(outcome, FlowOutcome::Finished);
	assert_eq!(committed, 1);
	assert_eq!(
		ui.pages,
		vec!["welcome", "choose", "sound-info", "ready", "done"]
	);

	// Without it, the branch falls through; `when` hides `ready`.
	let mut answers = State::new();
	answers.insert("components".into(), json!(["game"]));
	answers.insert("skip_ready".into(), json!(true));
	let mut ui = HeadlessUi::new(answers);
	let mut runner = FlowRunner::new(flow, env())
		.with_layouts(layouts)
		.with_scripts(&scripts);
	let mut commit = |_: &State, _: &mut dyn FnMut(&li_engine::Progress) -> bool| Ok(());
	runner.run(&mut ui, &mut commit).unwrap();
	assert_eq!(ui.pages, vec!["welcome", "choose", "done"]);
	assert_eq!(
		runner.visited,
		vec!["welcome", "choose", "route", "copy", "done"]
	);
}

struct Scripted {
	actions: Vec<Action>,
	pages: Vec<String>,
	problems: Vec<String>,
	layouts_seen: Vec<bool>,
	set_name: Vec<&'static str>,
}

impl Ui for Scripted {
	fn page(&mut self, step: &Step, layout: Option<&Layout>, state: &mut State) -> Action {
		self.pages.push(step.id.clone());
		self.layouts_seen.push(layout.is_some());
		if step.id == "choose" && !self.set_name.is_empty() {
			state.insert("name".into(), json!(self.set_name.remove(0)));
		}
		if self.actions.is_empty() {
			Action::Next
		} else {
			self.actions.remove(0)
		}
	}

	fn problems(&mut self, _: &Step, problems: &[String]) {
		self.problems.extend(problems.iter().cloned());
	}
}

#[test]
fn back_cancel_and_validation() {
	let (p, scripts) = runner_setup();
	let flow = &p.flows["install"];
	let layouts = &p.themes["fixture"].layouts;
	let mut commit = |_: &State, _: &mut dyn FnMut(&li_engine::Progress) -> bool| Ok(());

	// welcome Next, choose Back, welcome Next, choose Next ... then cancel at ready.
	let mut ui = Scripted {
		actions: vec![
			Action::Next,
			Action::Back,
			Action::Next,
			Action::Next,
			Action::Cancel,
		],
		pages: vec![],
		problems: vec![],
		layouts_seen: vec![],
		set_name: vec![],
	};
	let mut runner = FlowRunner::new(flow, env())
		.with_layouts(layouts)
		.with_scripts(&scripts);
	assert_eq!(
		runner.run(&mut ui, &mut commit).unwrap(),
		FlowOutcome::Cancelled
	);
	assert_eq!(
		ui.pages,
		vec!["welcome", "choose", "welcome", "choose", "ready"]
	);
	assert!(ui.layouts_seen.iter().all(|s| *s));

	// A rejected page is shown again until the script accepts it.
	let mut ui = Scripted {
		actions: vec![],
		pages: vec![],
		problems: vec![],
		layouts_seen: vec![],
		set_name: vec!["bad", "good"],
	};
	let mut runner = FlowRunner::new(flow, env())
		.with_layouts(layouts)
		.with_scripts(&scripts);
	assert_eq!(
		runner.run(&mut ui, &mut commit).unwrap(),
		FlowOutcome::Finished
	);
	assert_eq!(ui.problems, vec!["the name `bad` is not allowed"]);
	assert_eq!(ui.pages.iter().filter(|p| *p == "choose").count(), 2);

	// A page that never validates ends the flow with an error instead of looping.
	let mut answers = State::new();
	answers.insert("name".into(), json!("bad"));
	struct Stubborn(State);
	impl Ui for Stubborn {
		fn page(&mut self, _: &Step, _: Option<&Layout>, s: &mut State) -> Action {
			s.extend(self.0.clone());
			Action::Next
		}
	}
	let mut runner = FlowRunner::new(flow, env()).with_scripts(&scripts);
	assert!(runner.run(&mut Stubborn(answers), &mut commit).is_err());
}

#[test]
fn the_commit_step_reports_progress_and_can_cancel() {
	let (p, scripts) = runner_setup();
	let flow = &p.flows["install"];
	struct Meter(Vec<f64>);
	impl Ui for Meter {
		fn page(&mut self, _: &Step, _: Option<&Layout>, _: &mut State) -> Action {
			Action::Next
		}
		fn progress(&mut self, p: &li_engine::Progress) -> bool {
			self.0.push(p.fraction());
			self.0.len() < 2
		}
	}
	let mut ui = Meter(vec![]);
	let mut runner = FlowRunner::new(flow, env()).with_scripts(&scripts);
	let mut commit = |_: &State, report: &mut dyn FnMut(&li_engine::Progress) -> bool| {
		let p = li_engine::Progress {
			phase: li_engine::Phase::Commit,
			current_file: Some("{install}/a".into()),
			files_done: 1,
			files_total: 4,
			bytes_done: 0,
			bytes_total: 0,
			free_space: 1,
			low_space: false,
		};
		let mut ok = true;
		for _ in 0..3 {
			ok = report(&p);
			if !ok {
				break;
			}
		}
		assert!(!ok, "second event cancels");
		Ok(())
	};
	runner.run(&mut ui, &mut commit).unwrap();
	assert_eq!(ui.0, vec![0.25, 0.25]);
}
