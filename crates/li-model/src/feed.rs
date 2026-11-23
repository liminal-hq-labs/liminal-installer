// Feeds: signed JSON lists of products, channels and release artefacts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Feed {
	pub version: u32,
	pub publisher: String,
	/// RFC 3339 time; clients refuse a feed whose timestamp goes backwards.
	#[serde(default)]
	pub timestamp: Option<String>,
	pub products: Vec<FeedProduct>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedProduct {
	pub id: String,
	pub name: String,
	/// Channel name to the version it currently points at.
	pub channels: BTreeMap<String, String>,
	pub releases: Vec<Release>,
	#[serde(default)]
	pub themes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Release {
	pub version: String,
	pub artefacts: Vec<Artefact>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artefact {
	pub target: String,
	pub kind: String,
	pub url: String,
	pub size: u64,
	pub sha256: String,
}
