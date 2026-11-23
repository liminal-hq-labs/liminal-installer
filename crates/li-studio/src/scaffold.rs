// The files of a new installer project with a neutral theme: what `liminst init` and the Studio's New project write.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::error::StudioError;

/// Pretty JSON indented with tabs, ending in a newline, as the repository's editor settings ask.
fn to_json_tabs<T: serde::Serialize>(value: &T) -> String {
	let mut buf = Vec::new();
	let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
	value.serialize(&mut ser).expect("JSON values serialize");
	let mut s = String::from_utf8(buf).expect("JSON is UTF-8");
	s.push('\n');
	s
}

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

/// Checks the answers of a form before anything is written: the id is reverse-DNS, and the name and
/// publisher can sit inside a TOML string.
pub fn validate(o: &InitOptions) -> Result<(), StudioError> {
	let id_ok = o.id.contains('.')
		&& o.id.split('.').all(|part| {
			!part.is_empty()
				&& part
					.chars()
					.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
		});
	if !id_ok {
		return Err(StudioError::invalid(
			"id",
			"use reverse-DNS form with letters, digits, `-` and `_`, such as hq.liminal.planboard",
		));
	}
	for (what, text) in [("name", &o.name), ("publisher", &o.publisher)] {
		if text.trim().is_empty() {
			return Err(StudioError::invalid(what, "it cannot be empty"));
		}
		if text
			.chars()
			.any(|c| c.is_control() || c == '"' || c == '\\')
		{
			return Err(StudioError::invalid(
				what,
				"it cannot hold quotes, backslashes or line breaks",
			));
		}
	}
	Ok(())
}

/// Writes the scaffold into `root/installer/`, making `root` if needed. Refuses to touch an existing
/// project unless `force` is set. Returns the paths written.
pub fn init(root: &Path, o: &InitOptions) -> Result<Vec<PathBuf>, StudioError> {
	let dir = root.join("installer");
	if dir.join("installer.toml").exists() && !o.force {
		return Err(StudioError::invalid(
			"project",
			format!("{} already exists", dir.join("installer.toml").display()),
		));
	}
	let mut written = Vec::new();
	for (path, contents) in files(o) {
		let full = dir.join(&path);
		if let Some(parent) = full.parent() {
			std::fs::create_dir_all(parent).map_err(|e| StudioError::io(parent, e))?;
		}
		std::fs::write(&full, contents).map_err(|e| StudioError::io(&full, e))?;
		written.push(full);
	}
	Ok(written)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn opts(force: bool) -> InitOptions {
		InitOptions {
			id: "hq.test.demo".into(),
			name: "Demo".into(),
			publisher: "Test".into(),
			force,
		}
	}

	#[test]
	fn validation_catches_a_bad_id_and_text_that_would_break_the_toml() {
		assert!(validate(&opts(false)).is_ok());
		for id in ["demo", "a..b", "a.b c", ".a", ""] {
			let o = InitOptions {
				id: id.into(),
				..opts(false)
			};
			assert!(validate(&o).is_err(), "{id}");
		}
		for bad in ["", "  ", "Say \"hi\"", "a\\b", "two\nlines"] {
			let o = InitOptions {
				name: bad.into(),
				..opts(false)
			};
			assert!(validate(&o).is_err(), "{bad:?}");
		}
	}

	#[test]
	fn it_writes_a_project_the_studio_opens_and_refuses_to_overwrite_it() {
		let tmp = tempfile::tempdir().unwrap();
		let root = tmp.path().join("new").join("demo");
		let written = init(&root, &opts(false)).unwrap();
		assert!(
			written
				.iter()
				.any(|p| p.ends_with("flows/install.flow.json"))
		);
		let project = crate::Project::open(&root).unwrap();
		assert_eq!(project.installer.product.id, "hq.test.demo");
		assert!(init(&root, &opts(false)).is_err());
		init(&root, &opts(true)).unwrap();
	}
}
