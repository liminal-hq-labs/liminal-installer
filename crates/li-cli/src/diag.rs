// Diagnostics and reports produced by `check`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
	Error,
	Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
	pub severity: Severity,
	/// A stable short code such as `missing-layout`.
	pub code: String,
	pub message: String,
	/// The file the problem is in, relative to the project root.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub file: Option<String>,
}

impl Diagnostic {
	pub fn error(code: &str, message: impl Into<String>) -> Self {
		Self::new(Severity::Error, code, message)
	}

	pub fn warning(code: &str, message: impl Into<String>) -> Self {
		Self::new(Severity::Warning, code, message)
	}

	fn new(severity: Severity, code: &str, message: impl Into<String>) -> Self {
		Diagnostic {
			severity,
			code: code.to_owned(),
			message: message.into(),
			file: None,
		}
	}

	pub fn in_file(mut self, file: impl Into<String>) -> Self {
		self.file = Some(file.into());
		self
	}
}

/// The outcome of `check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
	pub ok: bool,
	pub targets: Vec<String>,
	pub errors: usize,
	pub warnings: usize,
	pub diagnostics: Vec<Diagnostic>,
}

impl Report {
	pub fn new(targets: Vec<String>, diagnostics: Vec<Diagnostic>) -> Report {
		let errors = diagnostics
			.iter()
			.filter(|d| d.severity == Severity::Error)
			.count();
		Report {
			ok: errors == 0,
			targets,
			errors,
			warnings: diagnostics.len() - errors,
			diagnostics,
		}
	}

	/// 0 when there are no errors, 1 otherwise. Warnings do not fail a check.
	pub fn exit_code(&self) -> u8 {
		u8::from(!self.ok)
	}

	pub fn render_text(&self) -> String {
		let mut out = String::new();
		for d in &self.diagnostics {
			let level = match d.severity {
				Severity::Error => "error",
				Severity::Warning => "warning",
			};
			out.push_str(&format!("{level}[{}]: {}\n", d.code, d.message));
			if let Some(f) = &d.file {
				out.push_str(&format!("  --> {f}\n"));
			}
		}
		out.push_str(&format!(
			"Checked {} target{}: {} error{}, {} warning{}.\n",
			self.targets.len(),
			plural(self.targets.len()),
			self.errors,
			plural(self.errors),
			self.warnings,
			plural(self.warnings),
		));
		out
	}
}

fn plural(n: usize) -> &'static str {
	if n == 1 { "" } else { "s" }
}
