// Runs the engine's checks and a dry run over every example project.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use li_engine::{DirPayload, InstallRequest, Project, Severity, SimMachine};
use li_examples_test::{EXAMPLES, examples_root};

fn load(name: &str) -> Project {
	Project::load(&examples_root().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn the_engine_finds_no_errors_in_any_example() {
	for name in EXAMPLES {
		let errors: Vec<_> = li_engine::check(&load(name))
			.into_iter()
			.filter(|d| d.severity == Severity::Error)
			.collect();
		assert!(errors.is_empty(), "{name}: {errors:?}");
	}
}

#[test]
fn every_example_previews_for_its_linux_target() {
	for name in EXAMPLES {
		let project = load(name);
		let target = project
			.installer
			.targets
			.iter()
			.find(|t| t.id == "linux-x86_64")
			.unwrap_or_else(|| panic!("{name}: no linux-x86_64 target"));
		let payload = DirPayload::new(&project.dir).unwrap();
		let req = InstallRequest {
			target: target.id.clone(),
			install_dir: PathBuf::from("/opt/example"),
			scope: project.installer.defaults.scope,
			components: None,
			version: None,
			feed: None,
			channel: None,
		};
		let plan = li_engine::preview(&project, &payload, &req, &SimMachine::new())
			.unwrap_or_else(|e| panic!("{name}: {e}"));
		assert!(!plan.ops.is_empty(), "{name}: empty plan");
		assert!(
			matches!(
				plan.ops.last(),
				Some(li_model::Operation::ReceiptWrite { .. })
			),
			"{name}: the plan does not end with the receipt"
		);
	}
}
