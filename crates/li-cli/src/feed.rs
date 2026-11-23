// `liminst feed add`: records a built artefact in a feed file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::path::Path;

use li_model::{Artefact, Feed, FeedProduct, Release};

use crate::project::Project;
use crate::util::{now_rfc3339, sha256_hex, to_json_tabs, write_file, write_file_bytes};
use crate::{CliError, CliResult};

pub struct AddRequest<'a> {
	pub feed: &'a Path,
	pub target: &'a str,
	pub kind: &'a str,
	pub file: &'a Path,
	pub url: &'a str,
	pub version: &'a str,
	pub channel: &'a str,
	/// The Ed25519 secret key file to sign the feed with; `feed.json.sig` is written beside it.
	pub sign_key: Option<&'a Path>,
}

/// The detached signature's path: the feed's own file name plus `.sig`.
pub fn sig_path(feed: &Path) -> std::path::PathBuf {
	let mut name = feed.as_os_str().to_owned();
	name.push(".sig");
	name.into()
}

/// What `add` did with the signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signature {
	/// The feed was signed; the signature is in this file.
	Signed(std::path::PathBuf),
	/// No key was given, so the feed is unsigned.
	Unsigned { removed_stale: bool },
}

/// Adds (or replaces) one artefact in the feed at `req.feed`, creating the feed if needed,
/// points the channel at the version, and stamps the feed. With `sign_key` the exact bytes
/// written are signed and the signature goes to `<feed>.sig`; without it any older signature
/// is removed, because it no longer matches.
pub fn add(project: &Project, req: &AddRequest) -> CliResult<(Feed, Signature)> {
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
	let sig = sig_path(req.feed);
	let signature = match req.sign_key {
		Some(key_path) => {
			let key = li_pack::read_signing_key(key_path)
				.map_err(|e| CliError::new(format!("{}: {e}", key_path.display())))?;
			let (json, hex) =
				li_pack::sign_feed(&feed, &key).map_err(|e| CliError::new(e.to_string()))?;
			write_file_bytes(req.feed, &json)?;
			write_file(&sig, &format!("{hex}\n"))?;
			Signature::Signed(sig)
		}
		None => {
			write_file(req.feed, &to_json_tabs(&feed))?;
			let removed_stale = sig.is_file() && std::fs::remove_file(&sig).is_ok();
			Signature::Unsigned { removed_stale }
		}
	};
	Ok((feed, signature))
}
