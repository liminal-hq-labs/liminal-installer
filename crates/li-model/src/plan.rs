// Plan operations: what Commit does, each with enough data to undo itself.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

/// One reversible change. Paths hold placeholders until the engine resolves them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
	Mkdir {
		path: String,
	},
	Copy {
		from: String,
		to: String,
		#[serde(default)]
		sha256: Option<String>,
	},
	WriteFile {
		path: String,
		contents: String,
	},
	WriteIni {
		path: String,
		section: String,
		key: String,
		value: String,
	},
	WriteJson {
		path: String,
		pointer: String,
		value: serde_json::Value,
	},
	Shortcut {
		name: String,
		target: String,
		place: String,
	},
	FileType {
		extension: String,
		name: String,
		#[serde(default)]
		open_with: Option<String>,
	},
	UrlScheme {
		scheme: String,
		open_with: String,
	},
	#[serde(rename = "registry.set")]
	RegistrySet {
		key: String,
		name: String,
		value: String,
	},
	#[serde(rename = "path.add")]
	PathAdd {
		entry: String,
	},
	#[serde(rename = "service.install")]
	ServiceInstall {
		name: String,
		command: String,
	},
	#[serde(rename = "os.register")]
	OsRegister {
		product: String,
		version: String,
	},
	#[serde(rename = "receipt.write")]
	ReceiptWrite {
		path: String,
	},
}

/// An ordered list of operations. `receipt.write` is always last.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
	pub ops: Vec<Operation>,
}
