// The receipt written after a successful install, from which every later lifecycle command starts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

use crate::{Operation, Scope, Updates};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
	pub version: u32,
	pub product: String,
	pub installed: String,
	pub target: String,
	pub scope: Scope,
	#[serde(default)]
	pub feed: Option<String>,
	#[serde(default)]
	pub channel: Option<String>,
	pub components: Vec<String>,
	pub operations: Vec<Operation>,
	/// Who updates the product, as the project said at install time. Absent on older receipts,
	/// which the installer updates.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub updates: Option<Updates>,
}

impl Receipt {
	/// What a wrapped installer left behind, when the install ran one.
	pub fn external(&self) -> Option<&crate::tauri::External> {
		self.operations.iter().find_map(|o| match o {
			Operation::TauriWrap { external, .. } => external.as_ref(),
			_ => None,
		})
	}

	pub const VERSION: u32 = 1;
	/// Where the receipt lives, relative to the install directory.
	pub const PATH: &'static str = ".liminal/receipt.json";
}
