// The receipt written after a successful install, from which every later lifecycle command starts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

use crate::{Operation, Scope};

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
}

impl Receipt {
	pub const VERSION: u32 = 1;
	/// Where the receipt lives, relative to the install directory.
	pub const PATH: &'static str = ".liminal/receipt.json";
}
