// A script's source checked without running it: syntax errors, and warnings about what it uses.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`check`] compiles the source in an empty Lua state, so nothing in it runs and no hook is
//! called, then reads its tokens for the things that fail later: a module used without the
//! capability that opens it, a global the sandbox removed, and what the retro runtime's Lua
//! subset lacks. Lines and columns start at 1; a column counts characters.

use serde::{Deserialize, Serialize};

use crate::CAPABILITIES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
	feature = "ts",
	derive(ts_rs::TS),
	ts(export, rename = "ScriptSeverity")
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
	Error,
	Warning,
}

/// One finding, with the span it marks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
	feature = "ts",
	derive(ts_rs::TS),
	ts(export, rename = "ScriptDiagnostic")
)]
pub struct Diagnostic {
	pub severity: Severity,
	/// A stable name for the kind of finding, such as `syntax` or `undeclared-capability`.
	pub code: String,
	pub message: String,
	pub line: u32,
	pub column: u32,
	pub end_line: u32,
	pub end_column: u32,
}

/// The `li.*` modules and the capability each one needs.
const MODULES: &[(&str, &str)] = &[
	("env", "env"),
	("fs", "fs.read"),
	("ui", "ui"),
	("plan", "plan"),
	("net", "net"),
];

/// Globals the sandbox removes.
const REMOVED_GLOBALS: &[&str] = &[
	"io",
	"os",
	"package",
	"debug",
	"coroutine",
	"load",
	"loadfile",
	"dofile",
	"require",
	"pcall",
	"xpcall",
];

/// The hooks that take the plan, not a page.
const PLAN_HOOKS: &[&str] = &["on_commit", "on_uninstall"];
const PAGE_HOOKS: &[&str] = &["detect", "validate"];

#[derive(Debug, Clone, PartialEq)]
enum Kind {
	Ident,
	Punct,
}

#[derive(Debug, Clone)]
struct Token {
	kind: Kind,
	text: String,
	line: u32,
	column: u32,
	end_column: u32,
}

/// Splits code into names and single punctuation marks, leaving out comments, strings and numbers.
fn tokens(source: &str) -> Vec<Token> {
	let chars: Vec<char> = source.chars().collect();
	let mut out = Vec::new();
	let (mut i, mut line, mut col) = (0usize, 1u32, 1u32);
	// The level of a long bracket opening at `at` (`[[` is 0, `[==[` is 2), and its length.
	let long_open = |at: usize| -> Option<(usize, usize)> {
		if chars.get(at) != Some(&'[') {
			return None;
		}
		let mut n = 0;
		while chars.get(at + 1 + n) == Some(&'=') {
			n += 1;
		}
		(chars.get(at + 1 + n) == Some(&'[')).then_some((n, n + 2))
	};
	macro_rules! advance {
		() => {{
			if chars[i] == '\n' {
				line += 1;
				col = 1;
			} else {
				col += 1;
			}
			i += 1;
		}};
	}
	while i < chars.len() {
		let c = chars[i];
		if c == '-' && chars.get(i + 1) == Some(&'-') {
			advance!();
			advance!();
			if let Some((level, len)) = long_open(i) {
				for _ in 0..len {
					advance!();
				}
				while i < chars.len() {
					if chars[i] == ']'
						&& (0..level).all(|k| chars.get(i + 1 + k) == Some(&'='))
						&& chars.get(i + 1 + level) == Some(&']')
					{
						for _ in 0..level + 2 {
							advance!();
						}
						break;
					}
					advance!();
				}
			} else {
				while i < chars.len() && chars[i] != '\n' {
					advance!();
				}
			}
		} else if c == '"' || c == '\'' {
			advance!();
			while i < chars.len() && chars[i] != c && chars[i] != '\n' {
				if chars[i] == '\\' && i + 1 < chars.len() {
					advance!();
				}
				advance!();
			}
			if i < chars.len() && chars[i] == c {
				advance!();
			}
		} else if let Some((level, len)) = long_open(i).filter(|_| c == '[') {
			for _ in 0..len {
				advance!();
			}
			while i < chars.len() {
				if chars[i] == ']'
					&& (0..level).all(|k| chars.get(i + 1 + k) == Some(&'='))
					&& chars.get(i + 1 + level) == Some(&']')
				{
					for _ in 0..level + 2 {
						advance!();
					}
					break;
				}
				advance!();
			}
		} else if c.is_ascii_alphabetic() || c == '_' {
			let (start, start_col, start_line) = (i, col, line);
			while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
				advance!();
			}
			out.push(Token {
				kind: Kind::Ident,
				text: chars[start..i].iter().collect(),
				line: start_line,
				column: start_col,
				end_column: col,
			});
		} else if c.is_ascii_digit() {
			while i < chars.len()
				&& (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_')
			{
				advance!();
			}
		} else if c.is_whitespace() {
			advance!();
		} else {
			out.push(Token {
				kind: Kind::Punct,
				text: c.to_string(),
				line,
				column: col,
				end_column: col + 1,
			});
			advance!();
		}
	}
	out
}

fn diagnostic(severity: Severity, code: &str, message: String, t: &Token) -> Diagnostic {
	Diagnostic {
		severity,
		code: code.to_owned(),
		message,
		line: t.line,
		column: t.column,
		end_line: t.line,
		end_column: t.end_column,
	}
}

/// The capabilities named in the leading comment block, as the loader reads them, each with where it is.
fn header_names(source: &str) -> Vec<(String, u32, u32, u32)> {
	let mut out = Vec::new();
	for (n, text) in source.lines().enumerate() {
		let trimmed = text.trim();
		if trimmed.is_empty() {
			continue;
		}
		if !trimmed.starts_with("--") {
			break;
		}
		let Some(at) = text.find("capabilities:") else {
			continue;
		};
		let list_start = at + "capabilities:".len();
		let mut offset = list_start;
		for part in text[list_start..].split_inclusive([',', ' ']) {
			let word = part.trim_matches([',', ' ']);
			if !word.is_empty() {
				let byte = offset + part.find(word).unwrap_or(0);
				let column = text[..byte].chars().count() as u32 + 1;
				out.push((
					word.to_owned(),
					n as u32 + 1,
					column,
					column + word.chars().count() as u32,
				));
			}
			offset += part.len();
		}
	}
	out
}

/// A Lua syntax error as a diagnostic: Lua names the line, and the token it stopped near says where on it.
fn syntax_error(source: &str, message: &str) -> Diagnostic {
	let lines: Vec<&str> = source.lines().collect();
	let body = message.strip_prefix("script:").unwrap_or(message);
	let (line_no, text) = match body.split_once(':') {
		Some((n, rest)) if n.trim().parse::<u32>().is_ok() => {
			(n.trim().parse::<u32>().unwrap_or(1), rest.trim().to_owned())
		}
		_ => (1, body.trim().to_owned()),
	};
	let line_no = line_no.clamp(1, lines.len().max(1) as u32);
	let line_text = lines.get(line_no as usize - 1).copied().unwrap_or("");
	let width = line_text.chars().count() as u32;
	let near = text
		.rsplit_once(" near ")
		.map(|(_, t)| t.trim().to_owned())
		.unwrap_or_default();
	let (column, end_column) = if let Some(tok) = near
		.strip_prefix('\'')
		.and_then(|t| t.strip_suffix('\''))
		.filter(|t| !t.is_empty())
		&& let Some(byte) = line_text.find(tok)
	{
		let start = line_text[..byte].chars().count() as u32 + 1;
		(start, start + tok.chars().count() as u32)
	} else {
		let indent = line_text.chars().take_while(|c| c.is_whitespace()).count() as u32;
		let start = indent + 1;
		(start.min(width.max(1)), width.max(start) + 1)
	};
	Diagnostic {
		severity: Severity::Error,
		code: "syntax".into(),
		message: text,
		line: line_no,
		column,
		end_line: line_no,
		end_column,
	}
}

/// Checks `source` and returns what it finds, in the order it appears in the file. A syntax error
/// comes alone: the tokens of a script that does not parse tell nothing reliable.
pub fn check(source: &str) -> Vec<Diagnostic> {
	if let Ok(lua) = mlua::Lua::new_with(mlua::StdLib::NONE, mlua::LuaOptions::default())
		&& let Err(e) = lua.load(source).set_name("=script").into_function()
	{
		let message = match &e {
			mlua::Error::SyntaxError { message, .. } => message.clone(),
			other => other.to_string(),
		};
		return vec![syntax_error(source, &message)];
	}
	let mut out = Vec::new();
	let mut declared: Vec<String> = Vec::new();
	for (name, line, column, end_column) in header_names(source) {
		if CAPABILITIES.contains(&name.as_str()) {
			declared.push(name);
		} else {
			out.push(Diagnostic {
				severity: Severity::Error,
				code: "unknown-capability".into(),
				message: format!(
					"unknown capability `{name}`; the capabilities are {}",
					CAPABILITIES.join(", ")
				),
				line,
				column,
				end_line: line,
				end_column,
			});
		}
	}
	let toks = tokens(source);
	// A script that returns its own `capabilities` list declares them there.
	let returns_list = toks
		.windows(2)
		.any(|w| w[0].kind == Kind::Ident && w[0].text == "capabilities" && w[1].text == "=");
	let mut warned: Vec<String> = Vec::new();
	let at = |i: usize| toks.get(i);
	let text = |i: usize| toks.get(i).map(|t| t.text.as_str());
	for (i, t) in toks.iter().enumerate() {
		if t.kind != Kind::Ident {
			continue;
		}
		let after_dot = i > 0 && matches!(text(i - 1), Some("." | ":"));
		let prev = if i > 0 { text(i - 1) } else { None };
		match t.text.as_str() {
			"li" if !after_dot && text(i + 1) == Some(".") => {
				let Some(module) = at(i + 2).filter(|m| m.kind == Kind::Ident) else {
					continue;
				};
				let span = Token {
					kind: Kind::Ident,
					text: String::new(),
					line: t.line,
					column: t.column,
					end_column: module.end_column,
				};
				if module.text == "retro" {
					let part = at(i + 4).filter(|_| text(i + 3) == Some("."));
					let name =
						part.map_or("li.retro".to_owned(), |p| format!("li.retro.{}", p.text));
					out.push(diagnostic(
						Severity::Warning,
						"unavailable-module",
						format!("`{name}` is a design only; no runtime has it yet"),
						&span,
					));
					continue;
				}
				let Some((_, cap)) = MODULES.iter().find(|(m, _)| *m == module.text) else {
					out.push(diagnostic(
						Severity::Warning,
						"unknown-module",
						format!(
							"`li.{}` is not a module; the modules are li.env, li.fs, li.ui, li.plan and li.net",
							module.text
						),
						&span,
					));
					continue;
				};
				if !returns_list
					&& !declared
						.iter()
						.any(|d| d == cap || d == "fs.write" && *cap == "fs.read")
				{
					let short = format!("li.{}", module.text);
					if !warned.iter().any(|w| w == &short) {
						warned.push(short.clone());
						out.push(diagnostic(
							Severity::Warning,
							"undeclared-capability",
							format!(
								"`{short}` needs the `{cap}` capability; add `-- capabilities: {cap}` at the top of the file"
							),
							&span,
						));
					}
				}
				if module.text == "net" {
					out.push(diagnostic(
						Severity::Warning,
						"retro-subset",
						"`li.net` is not in the retro Lua subset, so DOS and Win16 targets refuse this script".into(),
						&span,
					));
				}
			}
			"utf8" if !after_dot && text(i + 1) == Some(".") => out.push(diagnostic(
				Severity::Warning,
				"retro-subset",
				"`utf8` is not in the retro Lua subset".into(),
				t,
			)),
			"string" if !after_dot && text(i + 1) == Some(".") => {
				if let Some(f) =
					at(i + 2).filter(|f| matches!(f.text.as_str(), "pack" | "unpack" | "packsize"))
				{
					out.push(Diagnostic {
						end_column: f.end_column,
						..diagnostic(
							Severity::Warning,
							"retro-subset",
							format!("`string.{}` is not in the retro Lua subset", f.text),
							t,
						)
					});
				}
			}
			"goto" if !after_dot => out.push(diagnostic(
				Severity::Warning,
				"retro-subset",
				"`goto` is not in the retro Lua subset".into(),
				t,
			)),
			"close" if prev == Some("<") && text(i + 1) == Some(">") => out.push(diagnostic(
				Severity::Warning,
				"retro-subset",
				"`<close>` variables are not in the retro Lua subset".into(),
				t,
			)),
			name if REMOVED_GLOBALS.contains(&name)
				&& !after_dot
				&& !matches!(prev, Some("function"))
				&& !matches!(prev, Some("local"))
				&& text(i + 1) != Some("=") =>
			{
				let hint = if name == "require" {
					"the `li.*` modules are globals"
				} else {
					"the sandbox removes it"
				};
				out.push(diagnostic(
					Severity::Warning,
					"sandbox-global",
					format!("`{name}` is not available to scripts: {hint}"),
					t,
				));
			}
			"function" => {
				let (Some(name), Some("("), Some(param), Some(")")) =
					(at(i + 1), text(i + 2), at(i + 3), text(i + 4))
				else {
					continue;
				};
				if PLAN_HOOKS.contains(&name.text.as_str()) && param.text == "page" {
					out.push(diagnostic(
						Severity::Warning,
						"hook-parameter",
						format!(
							"`{}` receives the plan; name its parameter `plan`",
							name.text
						),
						param,
					));
				} else if PAGE_HOOKS.contains(&name.text.as_str()) && param.text == "plan" {
					out.push(diagnostic(
						Severity::Warning,
						"hook-parameter",
						format!(
							"`{}` receives the page; name its parameter `page`",
							name.text
						),
						param,
					));
				}
			}
			_ => {}
		}
	}
	out.sort_by_key(|d| (d.line, d.column));
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	fn codes(src: &str) -> Vec<(String, u32, u32)> {
		check(src)
			.into_iter()
			.map(|d| (d.code, d.line, d.column))
			.collect()
	}

	#[test]
	fn a_syntax_error_names_its_line_and_the_token_it_stopped_at() {
		let found = check("local x = 1\nlocal y = * 2\n");
		assert_eq!(found.len(), 1, "{found:?}");
		let d = &found[0];
		assert_eq!((d.code.as_str(), d.severity), ("syntax", Severity::Error));
		assert_eq!((d.line, d.column, d.end_column), (2, 11, 12));
		assert!(d.message.contains("unexpected symbol"), "{}", d.message);
	}

	#[test]
	fn an_unclosed_block_is_reported_at_the_end() {
		let found = check("function detect(page)\n\tlocal a = 1\n");
		assert_eq!(found.len(), 1, "{found:?}");
		assert_eq!(found[0].code, "syntax");
		assert!(found[0].message.contains("'end' expected"), "{found:?}");
		assert_eq!(found[0].line, 2);
	}

	#[test]
	fn checking_runs_no_code() {
		// A loop that never ends would hang if the chunk ran.
		assert!(check("while true do end\nfunction detect(page) end").is_empty());
		assert!(check("error('boom')").is_empty());
	}

	#[test]
	fn a_clean_script_has_no_findings() {
		let src = "-- capabilities: env, ui\nfunction detect(page)\n\tli.ui.note(li.env.get('HOME'))\nend\n";
		assert!(check(src).is_empty(), "{:?}", check(src));
	}

	#[test]
	fn an_undeclared_module_is_warned_once_at_its_first_use() {
		let src = "-- capabilities: ui\nfunction detect(page)\n\tlocal a = li.env.get('A')\n\tlocal b = li.env.get('B')\nend\n";
		let found = check(src);
		assert_eq!(found.len(), 1, "{found:?}");
		let d = &found[0];
		assert_eq!(
			(d.code.as_str(), d.severity, d.line, d.column, d.end_column),
			("undeclared-capability", Severity::Warning, 3, 12, 18)
		);
		assert!(d.message.contains("`env`"), "{}", d.message);
	}

	#[test]
	fn fs_write_also_opens_fs() {
		assert!(check("-- capabilities: fs.write\nlocal x = li.fs\n").is_empty());
		assert_eq!(codes("local x = li.fs\n").len(), 1);
	}

	#[test]
	fn an_unknown_header_capability_is_an_error_where_it_is_written() {
		let found = check("-- capabilities: env, teleport\n");
		assert_eq!(found.len(), 1);
		assert_eq!(
			(
				found[0].code.as_str(),
				found[0].severity,
				found[0].column,
				found[0].end_column
			),
			("unknown-capability", Severity::Error, 23, 31)
		);
	}

	#[test]
	fn strings_and_comments_are_not_code() {
		let src = "-- li.env.get and os.exit\nlocal s = 'li.net.get(os)'\nlocal t = [[ li.ui ]]\n--[[ li.plan\nutf8.char ]]\n";
		assert!(check(src).is_empty(), "{:?}", check(src));
	}

	#[test]
	fn what_the_retro_subset_lacks_is_warned() {
		let src = "-- capabilities: net\nlocal a = utf8.char(65)\nlocal b = string.pack('i4', 1)\nlocal c = li.net.get('x')\ngoto done\n::done::\n";
		let found = check(src);
		let retro: Vec<_> = found.iter().filter(|d| d.code == "retro-subset").collect();
		assert_eq!(retro.len(), 4, "{found:?}");
		assert!(retro.iter().all(|d| d.severity == Severity::Warning));
	}

	#[test]
	fn removed_globals_and_retro_modules_are_warned() {
		let found =
			check("local f = require('li.env')\nlocal s = li.retro.sound\nlocal o = li.nope\n");
		let c: Vec<&str> = found.iter().map(|d| d.code.as_str()).collect();
		assert_eq!(
			c,
			["sandbox-global", "unavailable-module", "unknown-module"],
			"{found:?}"
		);
	}

	#[test]
	fn a_hook_with_the_wrong_parameter_is_warned() {
		let found = check(
			"function on_commit(page) end\nfunction detect(plan) end\nfunction validate(page) end\n",
		);
		assert_eq!(found.len(), 2, "{found:?}");
		assert!(found[0].message.contains("`plan`"));
		assert_eq!(found[0].code, "hook-parameter");
	}

	#[test]
	fn a_returned_capabilities_list_counts_as_declared() {
		assert!(
			check(
				"return { capabilities = {'env'}, detect = function(page) return li.env.get('A') end }"
			)
			.is_empty()
		);
	}
}
