// `liminst init`: the files of a new installer project with a neutral theme.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::util::{to_json_tabs, write_file};
use crate::{CliError, CliResult};

pub struct InitOptions {
	pub id: String,
	pub name: String,
	pub publisher: String,
	pub force: bool,
}

fn button(label: &str, action: &str, default: bool) -> Value {
	json!({ "kind": "button", "props": { "label": label, "action": action, "default": default } })
}

fn page(id: &str, title: &str, body: Vec<Value>, buttons: Vec<Value>) -> Value {
	let mut children = body;
	children.push(json!({
		"kind": "stack",
		"props": { "direction": "horizontal", "gap": 2 },
		"children": buttons
	}));
	json!({
		"$schema": "https://liminal-hq-labs.github.io/liminal-installer/schemas/layout-1.json",
		"version": 1,
		"id": id,
		"root": {
			"kind": "window",
			"props": { "title": title },
			"children": children
		}
	})
}

fn text(t: &str) -> Value {
	json!({ "kind": "text", "props": { "text": t } })
}

/// Every file of the scaffold as (path under `installer/`, contents).
pub fn files(o: &InitOptions) -> Vec<(String, String)> {
	let slug = o.id.rsplit('.').next().unwrap_or(&o.id).to_owned();
	let installer = format!(
		r#"# The product, its targets and signing.

[product]
id        = "{id}"
name      = "{name}"
publisher = "{publisher}"
version   = {{ from = "git-tag" }}

[defaults]
scope = "user"        # user | machine | ask
theme = "neutral"

[[target]]
id       = "linux-x86_64"
renderer = "tui"
outputs  = ["run", "deb"]
"#,
		id = o.id,
		name = o.name,
		publisher = o.publisher
	);
	let components = format!(
		r#"# What gets installed.

[[component]]
id       = "app"
name     = "{name}"
required = true
files    = [{{ from = "payload/{{target}}/{slug}", to = "{{install}}/bin/" }}]
shortcut = [{{ name = "{name}", target = "{{install}}/bin/{slug}", in = "start_menu" }}]
"#,
		name = o.name,
		slug = slug
	);
	let theme = String::from(
		r#"# A plain theme that works with every renderer.

[theme]
id        = "neutral"
name      = "Neutral"
renderers = ["tui", "tui-retro", "canvas", "web"]
"#,
	);
	let step = |id: &str, kind: &str, layout: &str, next: Option<&str>| {
		let mut s = json!({ "id": id, "type": kind, "layout": layout });
		if let Some(n) = next {
			s["next"] = json!(n);
		}
		s
	};
	let flow = json!({
		"$schema": "https://liminal-hq-labs.github.io/liminal-installer/schemas/flow-1.json",
		"version": 1,
		"id": "install",
		"start": "welcome",
		"steps": [
			step("welcome", "page", "welcome", Some("location")),
			step("location", "page", "location", Some("copy")),
			step("copy", "commit", "copying", Some("done")),
			step("done", "finish", "done", None),
		]
	});
	let title = format!("{} Setup", o.name);
	let layouts = [
		page(
			"welcome",
			&title,
			vec![text(&format!(
				"Welcome. This installs {} on your computer.",
				o.name
			))],
			vec![
				button("Next", "next", true),
				button("Cancel", "cancel", false),
			],
		),
		page(
			"location",
			&title,
			vec![json!({
				"kind": "path",
				"bind": "state.install_dir",
				"props": { "label": "Install to", "width": 40 }
			})],
			vec![
				button("Back", "back", false),
				button("Install", "next", true),
				button("Cancel", "cancel", false),
			],
		),
		page(
			"copying",
			&title,
			vec![
				text("Copying files."),
				json!({ "kind": "progress", "bind": "meter.total", "props": { "label": "Overall" } }),
			],
			vec![],
		),
		page(
			"done",
			&title,
			vec![text(&format!("{} is installed.", o.name))],
			vec![button("Finish", "finish", true)],
		),
	];
	let mut out = vec![
		("installer.toml".to_owned(), installer),
		("components.toml".to_owned(), components),
		("flows/install.flow.json".to_owned(), to_json_tabs(&flow)),
		("themes/neutral/theme.toml".to_owned(), theme),
	];
	for l in &layouts {
		let id = l["id"].as_str().unwrap_or_default();
		out.push((
			format!("themes/neutral/layouts/{id}.layout.json"),
			to_json_tabs(l),
		));
	}
	out
}

/// Writes the scaffold into `root/installer/`. Refuses to touch an existing folder unless
/// `force` is set. Returns the paths written.
pub fn init(root: &Path, o: &InitOptions) -> CliResult<Vec<PathBuf>> {
	let dir = root.join("installer");
	if dir.join("installer.toml").exists() && !o.force {
		return Err(CliError::new(format!(
			"{} already exists; pass --force to overwrite the scaffold files",
			dir.join("installer.toml").display()
		)));
	}
	let mut written = Vec::new();
	for (path, contents) in files(o) {
		let full = dir.join(&path);
		write_file(&full, &contents)?;
		written.push(full);
	}
	Ok(written)
}
