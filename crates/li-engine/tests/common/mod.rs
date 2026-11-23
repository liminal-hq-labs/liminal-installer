// Shared helpers for the engine's integration tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use li_engine::{DirPayload, EngineError, InstallRequest, Payload, Project, RealMachine};
use li_model::Scope;

pub fn fixture_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic")
}

pub fn project() -> Project {
	Project::load(&fixture_dir()).expect("fixture loads")
}

pub fn payload(project: &Project) -> DirPayload {
	DirPayload::new(&project.dir).expect("payload")
}

pub fn request(install_dir: &Path) -> InstallRequest {
	InstallRequest {
		target: "linux-x86_64".into(),
		install_dir: install_dir.to_owned(),
		scope: Scope::User,
		components: None,
		version: None,
		feed: Some("https://example.org/feed.json".into()),
		channel: Some("stable".into()),
	}
}

pub fn machine(root: &Path) -> RealMachine {
	RealMachine::sandboxed(root).expect("sandbox")
}

/// Every file and directory under `root`, with file contents.
pub fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
	fn walk(dir: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
		for entry in std::fs::read_dir(dir).unwrap() {
			let path = entry.unwrap().path();
			if path.is_dir() {
				out.insert(path.clone(), None);
				walk(&path, out);
			} else {
				out.insert(path.clone(), Some(std::fs::read(&path).unwrap()));
			}
		}
	}
	let mut out = BTreeMap::new();
	walk(root, &mut out);
	out
}

/// A payload that serves tampered bytes for one path after it has been read `after` times.
pub struct FlakyPayload<'a> {
	pub inner: &'a dyn Payload,
	pub path: String,
	pub after: usize,
	pub reads: AtomicUsize,
}

impl<'a> FlakyPayload<'a> {
	pub fn new(inner: &'a dyn Payload, path: &str, after: usize) -> Self {
		FlakyPayload {
			inner,
			path: path.to_owned(),
			after,
			reads: AtomicUsize::new(0),
		}
	}
}

impl Payload for FlakyPayload<'_> {
	fn list(&self) -> Vec<String> {
		self.inner.list()
	}

	fn read(&self, path: &str) -> Result<Vec<u8>, EngineError> {
		let mut data = self.inner.read(path)?;
		if path == self.path && self.reads.fetch_add(1, Ordering::Relaxed) >= self.after {
			data.extend_from_slice(b"tampered");
		}
		Ok(data)
	}

	fn size(&self, path: &str) -> Option<u64> {
		self.inner.size(path)
	}

	fn executable(&self, path: &str) -> bool {
		self.inner.executable(path)
	}
}
