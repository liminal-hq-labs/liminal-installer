// Test only: the commands and argument names the UI calls match the commands the shell registers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A command the shell registers: its argument names as the UI spells them, and which may be left out.
#[derive(Debug, Default)]
struct Command {
	args: BTreeSet<String>,
	optional: BTreeSet<String>,
}

fn camel(snake: &str) -> String {
	let mut out = String::new();
	let mut up = false;
	for c in snake.chars() {
		if c == '_' {
			up = true;
		} else if up {
			out.extend(c.to_uppercase());
			up = false;
		} else {
			out.push(c);
		}
	}
	out
}

/// Every `#[tauri::command]` function in `source`, by name.
fn commands_in(source: &str) -> BTreeMap<String, Command> {
	let mut out = BTreeMap::new();
	let mut rest = source;
	while let Some(at) = rest.find("#[tauri::command]") {
		rest = &rest[at + "#[tauri::command]".len()..];
		let Some(fn_at) = rest.find("fn ") else { break };
		let sig = &rest[fn_at + 3..];
		let open = sig.find('(').expect("a command has parameters");
		let name = sig[..open].trim().to_owned();
		// The parameter list ends at the first `)` that closes the opening one.
		let mut depth = 0usize;
		let mut end = open;
		for (i, c) in sig[open..].char_indices() {
			match c {
				'(' => depth += 1,
				')' => {
					depth -= 1;
					if depth == 0 {
						end = open + i;
						break;
					}
				}
				_ => {}
			}
		}
		let mut command = Command::default();
		for param in split_top_level(&sig[open + 1..end]) {
			let Some((pname, ty)) = param.split_once(':') else {
				continue;
			};
			let ty = ty.trim();
			// Injected by Tauri, not sent by the UI.
			if ["State<", "AppHandle", "WebviewWindow", "Window"]
				.iter()
				.any(|t| ty.starts_with(t) || ty.contains(&format!("::{t}")))
			{
				continue;
			}
			let key = camel(pname.trim());
			if ty.starts_with("Option<") {
				command.optional.insert(key.clone());
			}
			command.args.insert(key);
		}
		out.insert(name, command);
	}
	out
}

/// Splits on commas that are not inside brackets, parentheses, braces or quotes.
fn split_top_level(text: &str) -> Vec<String> {
	let mut parts = Vec::new();
	let (mut depth, mut quote, mut template) = (0i32, None::<char>, 0i32);
	let mut current = String::new();
	let mut chars = text.chars().peekable();
	while let Some(c) = chars.next() {
		match quote {
			Some(q) => {
				if q == '`' && c == '$' && chars.peek() == Some(&'{') {
					template += 1;
					current.push(c);
					current.push(chars.next().unwrap());
					continue;
				}
				if q == '`' && c == '}' && template > 0 {
					template -= 1;
				} else if c == q && template == 0 {
					quote = None;
				}
			}
			None => match c {
				'\'' | '"' | '`' => quote = Some(c),
				'(' | '[' | '{' | '<' => depth += 1,
				')' | ']' | '}' | '>' => depth -= 1,
				',' if depth == 0 => {
					parts.push(std::mem::take(&mut current));
					continue;
				}
				_ => {}
			},
		}
		current.push(c);
	}
	if !current.trim().is_empty() {
		parts.push(current);
	}
	parts
}

/// The names in `generate_handler![...]`, without their module paths.
fn registered(lib: &str) -> BTreeSet<String> {
	let at = lib.find("generate_handler![").expect("a handler list");
	let list = &lib[at + "generate_handler![".len()..];
	let list = &list[..list.find(']').expect("the list ends")];
	list.split(',')
		.map(|n| n.trim().rsplit("::").next().unwrap_or("").to_owned())
		.filter(|n| !n.is_empty())
		.collect()
}

/// One call the UI makes: the command and the argument names it sends.
#[derive(Debug)]
struct Call {
	file: String,
	command: String,
	args: BTreeSet<String>,
}

/// Calls written as `invoke('name', { ... })` or `call('name', { ... })` with a literal name.
fn calls_in(file: &str, source: &str) -> Vec<Call> {
	let mut out = Vec::new();
	for marker in ["invoke", "call"] {
		let mut from = 0;
		while let Some(at) = source[from..].find(marker) {
			let start = from + at;
			from = start + marker.len();
			let before = source[..start].chars().next_back();
			if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
				continue;
			}
			let mut rest = &source[from..];
			// An optional type argument, such as `invoke<string>(`.
			if rest.starts_with('<') {
				let Some(close) = rest.find('>') else {
					continue;
				};
				rest = &rest[close + 1..];
			}
			let Some(rest) = rest.strip_prefix("('") else {
				continue;
			};
			let Some(name_end) = rest.find('\'') else {
				continue;
			};
			let command = rest[..name_end].to_owned();
			let after = &rest[name_end + 1..];
			let mut args = BTreeSet::new();
			if let Some(body) = after.strip_prefix(',').map(str::trim_start)
				&& body.starts_with('{')
			{
				// The object ends at its matching brace.
				let mut depth = 0i32;
				let mut end = 0;
				for (i, c) in body.char_indices() {
					match c {
						'{' => depth += 1,
						'}' => {
							depth -= 1;
							if depth == 0 {
								end = i;
								break;
							}
						}
						_ => {}
					}
				}
				for entry in split_top_level(&body[1..end]) {
					let key = entry.split(':').next().unwrap_or("").trim().to_owned();
					if !key.is_empty() {
						args.insert(key);
					}
				}
			}
			out.push(Call {
				file: file.to_owned(),
				command,
				args,
			});
		}
	}
	out
}

fn ui_sources(dir: &Path, out: &mut Vec<PathBuf>) {
	for entry in std::fs::read_dir(dir).unwrap().flatten() {
		let path = entry.path();
		if path.is_dir() {
			ui_sources(&path, out);
		} else if path.extension().is_some_and(|e| e == "ts" || e == "tsx")
			&& !path.to_string_lossy().contains(".test.")
		{
			out.push(path);
		}
	}
}

#[test]
fn every_command_the_ui_calls_is_registered_with_the_arguments_it_sends() {
	let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
	let read = |name: &str| std::fs::read_to_string(src.join(name)).unwrap();
	let lib = read("lib.rs");
	let mut commands = BTreeMap::new();
	for file in ["lib.rs", "settings.rs", "windows.rs"] {
		commands.extend(commands_in(&read(file)));
	}
	let handlers = registered(&lib);
	for name in commands.keys() {
		assert!(
			handlers.contains(name),
			"`{name}` is a command but is not in generate_handler!"
		);
	}

	let mut files = Vec::new();
	ui_sources(
		&Path::new(env!("CARGO_MANIFEST_DIR")).join("../src"),
		&mut files,
	);
	let calls: Vec<Call> = files
		.iter()
		.flat_map(|f| {
			calls_in(
				&f.display().to_string(),
				&std::fs::read_to_string(f).unwrap(),
			)
		})
		.collect();
	assert!(
		calls.len() > 20,
		"the UI's calls were not found: {}",
		calls.len()
	);
	for call in &calls {
		let command = commands.get(&call.command).unwrap_or_else(|| {
			panic!(
				"{} calls `{}`, which is not a command",
				call.file, call.command
			)
		});
		assert!(
			handlers.contains(&call.command),
			"`{}` is not registered",
			call.command
		);
		for arg in &call.args {
			assert!(
				command.args.contains(arg),
				"{} sends `{arg}` to `{}`, which takes {:?}",
				call.file,
				call.command,
				command.args
			);
		}
		for arg in &command.args {
			assert!(
				call.args.contains(arg) || command.optional.contains(arg),
				"{} leaves out `{arg}` for `{}`",
				call.file,
				call.command
			);
		}
	}
}

#[test]
fn the_parsers_read_a_command_and_a_call() {
	let found = commands_in(
		"#[tauri::command]\nfn set_thing(state: State<AppState>, thing_id: String, note: Option<String>) -> Res<()> {}",
	);
	let c = &found["set_thing"];
	assert_eq!(
		c.args.iter().cloned().collect::<Vec<_>>(),
		["note", "thingId"]
	);
	assert!(c.optional.contains("note"));
	let calls = calls_in(
		"x.ts",
		"invoke('set_thing', { thingId, note: a ?? null, q: `${x}&b` })",
	);
	assert_eq!(
		calls[0].args.iter().cloned().collect::<Vec<_>>(),
		["note", "q", "thingId"]
	);
}
