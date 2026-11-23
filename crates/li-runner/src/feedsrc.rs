// Fetching a feed and its payload from a path or a URL, and verifying the feed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};
use std::process::Command;

use li_model::Feed;
use li_pack::VerifyingKey;

use crate::{RunnerError, RunnerResult};

fn is_url(s: &str) -> bool {
	s.starts_with("http://") || s.starts_with("https://")
}

fn local(s: &str) -> PathBuf {
	PathBuf::from(s.strip_prefix("file://").unwrap_or(s))
}

/// Reads a local path or downloads a URL. URLs go through `curl`, which the runner does not
/// bundle; a missing `curl` is reported as such.
pub fn fetch(spec: &str) -> RunnerResult<Vec<u8>> {
	if is_url(spec) {
		let out = Command::new("curl")
			.args(["--fail", "--silent", "--show-error", "--location", spec])
			.output()
			.map_err(|e| RunnerError::new(format!("cannot run `curl` to fetch {spec}: {e}")))?;
		if !out.status.success() {
			return Err(RunnerError::new(format!(
				"cannot fetch {spec}: {}",
				String::from_utf8_lossy(&out.stderr).trim()
			)));
		}
		return Ok(out.stdout);
	}
	let path = local(spec);
	std::fs::read(&path).map_err(|e| RunnerError::new(format!("{}: {e}", path.display())))
}

/// The location of `feed.json` for a feed given as a file, a directory or a URL.
pub fn feed_location(spec: &str) -> String {
	if is_url(spec) {
		return spec.to_owned();
	}
	let p = local(spec);
	if p.is_dir() {
		p.join(li_pack::FEED_FILE).to_string_lossy().into_owned()
	} else {
		p.to_string_lossy().into_owned()
	}
}

/// Fetches the feed and its detached signature, and checks the signature against the pinned
/// key and the feed against the last one seen (no rollback). Returns the feed and the exact
/// bytes that were verified.
pub fn load_feed(
	spec: &str,
	key: &VerifyingKey,
	previous: Option<&Feed>,
) -> RunnerResult<(Feed, Vec<u8>)> {
	let loc = feed_location(spec);
	let json = fetch(&loc)?;
	let sig = fetch(&format!("{loc}.sig")).map_err(|e| {
		RunnerError::new(format!("the feed has no signature, so it is refused ({e})"))
	})?;
	let feed = li_pack::verify_feed(&json, &String::from_utf8_lossy(&sig), key, previous)
		.map_err(|e| RunnerError::new(format!("the feed was refused: {e}")))?;
	Ok((feed, json))
}

/// Where an artefact's `url` points: absolute URLs and paths as they are, relative ones beside
/// the feed.
pub fn artefact_location(feed_loc: &str, url: &str) -> String {
	if is_url(url) || url.starts_with("file://") || Path::new(url).is_absolute() {
		return url.to_owned();
	}
	if let Some(base) = feed_loc.rsplit_once('/').map(|(b, _)| b)
		&& !base.is_empty()
	{
		return format!("{base}/{url}");
	}
	url.to_owned()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn artefacts_resolve_beside_the_feed() {
		assert_eq!(
			artefact_location("/srv/feed/feed.json", "app.lip"),
			"/srv/feed/app.lip"
		);
		assert_eq!(
			artefact_location("https://x.org/f/feed.json", "app.lip"),
			"https://x.org/f/app.lip"
		);
		assert_eq!(
			artefact_location("/srv/feed/feed.json", "https://cdn.org/app.lip"),
			"https://cdn.org/app.lip"
		);
		assert_eq!(
			artefact_location("/srv/feed/feed.json", "/abs/app.lip"),
			"/abs/app.lip"
		);
	}

	#[test]
	fn a_folder_means_its_feed_json() {
		let dir = tempfile::tempdir().unwrap();
		let loc = feed_location(dir.path().to_str().unwrap());
		assert!(loc.ends_with("/feed.json"), "{loc}");
		assert_eq!(
			feed_location("https://x.org/feed.json"),
			"https://x.org/feed.json"
		);
	}

	#[test]
	fn an_unsigned_feed_is_refused() {
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("feed.json"), "{}").unwrap();
		let (_, vk) = li_pack::generate_keypair().unwrap();
		let e = load_feed(dir.path().to_str().unwrap(), &vk, None).unwrap_err();
		assert!(e.to_string().contains("no signature"), "{e}");
	}
}
