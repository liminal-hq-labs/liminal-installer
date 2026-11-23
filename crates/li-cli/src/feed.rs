// `liminst feed add`: records a built artefact in a feed file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::path::Path;

use li_model::{Artefact, Feed, FeedProduct, Release};

use crate::project::Project;
use crate::util::{now_rfc3339, sha256_hex, to_json_tabs, write_file};
use crate::{CliError, CliResult};

pub struct AddRequest<'a> {
	pub feed: &'a Path,
	pub target: &'a str,
	pub kind: &'a str,
	pub file: &'a Path,
	pub url: &'a str,
	pub version: &'a str,
	pub channel: &'a str,
}

/// Adds (or replaces) one artefact in the feed at `req.feed`, creating the feed if needed,
/// points the channel at the version, and stamps the feed. The feed is not signed: signing
/// is not available yet, so no `.sig` file is written.
pub fn add(project: &Project, req: &AddRequest) -> CliResult<Feed> {
	let bytes = std::fs::read(req.file)
		.map_err(|e| CliError::new(format!("cannot read {}: {e}", req.file.display())))?;
	let mut feed: Feed = if req.feed.is_file() {
		let text = std::fs::read_to_string(req.feed)
			.map_err(|e| CliError::new(format!("cannot read {}: {e}", req.feed.display())))?;
		serde_json::from_str(&text)
			.map_err(|e| CliError::new(format!("{} is not a feed: {e}", req.feed.display())))?
	} else {
		Feed {
			version: 1,
			publisher: project.installer.product.publisher.clone(),
			timestamp: None,
			products: Vec::new(),
		}
	};
	let p = &project.installer.product;
	let themes: Vec<String> = project.themes.keys().cloned().collect();
	let idx = match feed.products.iter().position(|x| x.id == p.id) {
		Some(i) => i,
		None => {
			feed.products.push(FeedProduct {
				id: p.id.clone(),
				name: p.name.clone(),
				channels: BTreeMap::new(),
				releases: Vec::new(),
				themes: themes.clone(),
			});
			feed.products.len() - 1
		}
	};
	let product = &mut feed.products[idx];
	product.themes = themes;
	product
		.channels
		.insert(req.channel.to_owned(), req.version.to_owned());
	let ridx = match product
		.releases
		.iter()
		.position(|r| r.version == req.version)
	{
		Some(i) => i,
		None => {
			product.releases.push(Release {
				version: req.version.to_owned(),
				artefacts: Vec::new(),
			});
			product.releases.len() - 1
		}
	};
	let release = &mut product.releases[ridx];
	release
		.artefacts
		.retain(|a| !(a.target == req.target && a.kind == req.kind));
	release.artefacts.push(Artefact {
		target: req.target.to_owned(),
		kind: req.kind.to_owned(),
		url: req.url.to_owned(),
		size: bytes.len() as u64,
		sha256: sha256_hex(&bytes),
	});
	feed.timestamp = Some(now_rfc3339());
	write_file(req.feed, &to_json_tabs(&feed))?;
	Ok(feed)
}
