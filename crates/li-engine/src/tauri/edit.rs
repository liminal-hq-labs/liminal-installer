// Small, comment-preserving edits to a hand-written TOML file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `installer.toml` is written by people, with comments and alignment, so the import and the
//! Studio change single keys in place rather than rewriting the file. Only plain `key = value`
//! lines are handled, which is all the sections these edits touch ever hold.

/// A TOML basic string.
pub fn quote(s: &str) -> String {
	let mut out = String::from("\"");
	for c in s.chars() {
		match c {
			'"' => out.push_str("\\\""),
			'\\' => out.push_str("\\\\"),
			'\n' => out.push_str("\\n"),
			'\t' => out.push_str("\\t"),
			c => out.push(c),
		}
	}
	out.push('"');
	out
}

/// A TOML array of basic strings, on one line.
pub fn string_array(items: &[String]) -> String {
	format!(
		"[{}]",
		items
			.iter()
			.map(|i| quote(i))
			.collect::<Vec<_>>()
			.join(", ")
	)
}

fn is_header(line: &str) -> bool {
	line.trim_start().starts_with('[')
}

fn header_name(line: &str) -> Option<&str> {
	let t = line.trim();
	t.strip_prefix('[')
		.and_then(|r| r.strip_suffix(']'))
		.filter(|r| !r.starts_with('['))
		.map(str::trim)
}

fn key_of(line: &str) -> Option<&str> {
	let t = line.trim_start();
	if t.starts_with('#') || t.starts_with('[') {
		return None;
	}
	t.split_once('=').map(|(k, _)| k.trim())
}

/// Sets `key` in `[section]` to `value` (already TOML text), or removes it when `value` is
/// `None`. The section is added at the end of the file when it is missing, and a removed key
/// leaves an empty section in place. Returns the new text.
pub fn set_key(text: &str, section: &str, key: &str, value: Option<&str>) -> String {
	let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
	let start = lines
		.iter()
		.position(|l| header_name(l) == Some(section))
		.map(|i| i + 1);
	let Some(start) = start else {
		let Some(value) = value else {
			return text.to_owned();
		};
		while lines.last().is_some_and(|l| l.trim().is_empty()) {
			lines.pop();
		}
		if !lines.is_empty() {
			lines.push(String::new());
		}
		lines.push(format!("[{section}]"));
		lines.push(format!("{key} = {value}"));
		return lines.join("\n") + "\n";
	};
	let end = lines[start..]
		.iter()
		.position(|l| is_header(l))
		.map_or(lines.len(), |i| start + i);
	let found = (start..end).find(|&i| key_of(&lines[i]) == Some(key));
	match (found, value) {
		(Some(i), Some(v)) => {
			let (head, _) = lines[i].split_once('=').expect("has =");
			let comment = lines[i]
				.split_once('=')
				.and_then(|(_, rest)| trailing_comment(rest));
			lines[i] = match comment {
				Some(c) => format!("{head}= {v}  {c}"),
				None => format!("{head}= {v}"),
			};
		}
		(Some(i), None) => {
			lines.remove(i);
		}
		(None, Some(v)) => {
			let mut at = end;
			while at > start && lines[at - 1].trim().is_empty() {
				at -= 1;
			}
			lines.insert(at, format!("{key} = {v}"));
		}
		(None, None) => return text.to_owned(),
	}
	lines.join("\n") + "\n"
}

/// A `# comment` after a value, outside of any string.
fn trailing_comment(rest: &str) -> Option<&str> {
	let mut in_str = false;
	let mut prev = ' ';
	for (i, c) in rest.char_indices() {
		match c {
			'"' if prev != '\\' => in_str = !in_str,
			'#' if !in_str => return Some(rest[i..].trim_end()),
			_ => {}
		}
		prev = c;
	}
	None
}

/// The value text of `key` in `[section]`, if it is a one-line value.
pub fn get_key(text: &str, section: &str, key: &str) -> Option<String> {
	let mut in_section = false;
	for l in text.lines() {
		if is_header(l) {
			in_section = header_name(l) == Some(section);
		} else if in_section && key_of(l) == Some(key) {
			let rest = l.split_once('=')?.1;
			let value = match trailing_comment(rest) {
				Some(c) => rest[..rest.len() - c.len()].trim(),
				None => rest.trim(),
			};
			return Some(value.to_owned());
		}
	}
	None
}

/// Replaces the text between two marker lines (inclusive) with `block`, or appends `block` when
/// the markers are not there. `block` includes its own marker lines.
pub fn replace_block(text: &str, begin: &str, end: &str, block: &str) -> String {
	let lines: Vec<&str> = text.lines().collect();
	let b = lines.iter().position(|l| l.trim() == begin);
	let e = lines.iter().position(|l| l.trim() == end);
	match (b, e) {
		(Some(b), Some(e)) if e >= b => {
			let mut out: Vec<&str> = lines[..b].to_vec();
			out.extend(block.trim_end().lines());
			out.extend(&lines[e + 1..]);
			out.join("\n") + "\n"
		}
		_ => {
			let mut out = text.trim_end().to_owned();
			if !out.is_empty() {
				out.push_str("\n\n");
			}
			out.push_str(block.trim_end());
			out.push('\n');
			out
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const DOC: &str = "# top\n[product]\nid        = \"a.b\"   # the id\nname = \"X\"\n\n[defaults]\nscope = \"user\"\n";

	#[test]
	fn replaces_a_value_and_keeps_alignment_and_comment() {
		let out = set_key(DOC, "product", "id", Some("\"c.d\""));
		assert!(out.contains("id        = \"c.d\"  # the id\n"), "{out}");
		assert!(out.contains("name = \"X\""));
	}

	#[test]
	fn adds_a_key_at_the_end_of_its_section() {
		let out = set_key(DOC, "product", "publisher", Some("\"P\""));
		let p = out.find("publisher").unwrap();
		assert!(p > out.find("name").unwrap() && p < out.find("[defaults]").unwrap());
	}

	#[test]
	fn adds_a_missing_section_and_removes_a_key() {
		let out = set_key(DOC, "tauri", "config", Some("\"../x.json\""));
		assert!(out.ends_with("[tauri]\nconfig = \"../x.json\"\n"), "{out}");
		let out = set_key(&out, "tauri", "config", None);
		assert!(!out.contains("config"));
		assert_eq!(set_key(DOC, "nope", "k", None), DOC);
	}

	#[test]
	fn reads_one_line_values() {
		assert_eq!(get_key(DOC, "product", "id").as_deref(), Some("\"a.b\""));
		assert_eq!(
			get_key(DOC, "defaults", "scope").as_deref(),
			Some("\"user\"")
		);
		assert_eq!(get_key(DOC, "product", "scope"), None);
	}

	#[test]
	fn a_nested_table_is_its_own_section() {
		let doc = "[tauri]\nconfig = \"c\"\n\n[tauri.webview2]\nmode = \"skip\"\n";
		let out = set_key(doc, "tauri", "windows", Some("\"wrap\""));
		assert!(out.contains("config = \"c\"\nwindows = \"wrap\"\n\n[tauri.webview2]"));
		let out = set_key(
			&out,
			"tauri.webview2",
			"mode",
			Some("\"embedBootstrapper\""),
		);
		assert!(out.contains("mode = \"embedBootstrapper\""));
	}

	#[test]
	fn a_block_is_replaced_between_its_markers() {
		let doc = "a\n# >>> x\nold\n# <<< x\nb\n";
		let out = replace_block(doc, "# >>> x", "# <<< x", "# >>> x\nnew\n# <<< x\n");
		assert_eq!(out, "a\n# >>> x\nnew\n# <<< x\nb\n");
		let out = replace_block("a\n", "# >>> x", "# <<< x", "# >>> x\nnew\n# <<< x\n");
		assert_eq!(out, "a\n\n# >>> x\nnew\n# <<< x\n");
	}
}
