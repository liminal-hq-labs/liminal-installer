// Text helpers: wrapping, `{placeholders}` and sizes, matching li-render-tui.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// Wraps `text` at `width` columns on word boundaries, keeping explicit line breaks.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
	let width = width.max(1);
	let mut lines = Vec::new();
	for para in text.split('\n') {
		let mut line = String::new();
		for word in para.split_whitespace() {
			let wl = word.chars().count();
			let ll = line.chars().count();
			if ll > 0 && ll + 1 + wl > width {
				lines.push(core::mem::take(&mut line));
			}
			if !line.is_empty() {
				line.push(' ');
			}
			line.push_str(word);
		}
		lines.push(line);
	}
	lines
}

/// Replaces each `{name}` with `lookup(name)`. A name with no value stays as written.
pub fn interpolate(text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
	let mut out = String::with_capacity(text.len());
	let mut rest = text;
	while let Some(open) = rest.find('{') {
		out.push_str(&rest[..open]);
		let after = &rest[open + 1..];
		match after.find('}') {
			Some(close)
				if !after[..close].is_empty()
					&& after[..close]
						.chars()
						.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) =>
			{
				match lookup(&after[..close]) {
					Some(v) => out.push_str(&v),
					None => out.push_str(&rest[open..open + close + 2]),
				}
				rest = &after[close + 1..];
			}
			_ => {
				out.push('{');
				rest = after;
			}
		}
	}
	out.push_str(rest);
	out
}

/// A size for people: `812 bytes`, `96 KB`, `1.4 MB`.
pub fn human(bytes: u64) -> String {
	if bytes < 1024 {
		format!("{bytes} bytes")
	} else if bytes < 1024 * 1024 {
		format!("{} KB", bytes.div_ceil(1024))
	} else {
		let tenths = (bytes * 10 + 1024 * 1024 / 2) / (1024 * 1024);
		format!("{}.{} MB", tenths / 10, tenths % 10)
	}
}

#[cfg(test)]
mod tests {
	use alloc::string::ToString;
	use alloc::vec;

	use super::*;

	#[test]
	fn wraps_and_keeps_breaks() {
		assert_eq!(wrap("aa bb cc", 5), vec!["aa bb", "cc"]);
		assert_eq!(wrap("a\n\nb", 9), vec!["a", "", "b"]);
		assert_eq!(wrap("", 9), vec![""]);
	}

	#[test]
	fn fills_known_names_only() {
		let look = |n: &str| (n == "a.b").then(|| "X".to_string());
		assert_eq!(interpolate("{a.b} {c} { } {", &look), "X {c} { } {");
	}

	#[test]
	fn sizes() {
		assert_eq!(human(10), "10 bytes");
		assert_eq!(human(2048), "2 KB");
		assert_eq!(human(1_500_000), "1.4 MB");
	}
}
