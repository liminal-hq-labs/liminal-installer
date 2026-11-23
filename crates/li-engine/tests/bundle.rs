// Bundling a project into a `.lip` and loading it back.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use li_engine::bundle::{bundle_project, bundled_version, extract_project, pinned_key};
use li_engine::{LipPayload, Payload, Project, pack_payload};
use li_pack::{Lip, generate_keypair};

#[test]
fn a_bundled_project_loads_back_and_the_payload_is_unchanged() {
	let p = project();
	let (_, vk) = generate_keypair().unwrap();
	let plain = pack_payload(&p, "linux-x86_64").unwrap();
	let bytes = bundle_project(&p, &plain, "1.0.0", Some(&vk)).unwrap();
	let lip = Lip::parse(&bytes).unwrap();
	assert_eq!(pinned_key(&lip).unwrap(), Some(vk));
	assert_eq!(bundled_version(&lip).as_deref(), Some("1.0.0"));

	let payload = LipPayload(lip.clone());
	let plain_payload = LipPayload::from_bytes(&plain).unwrap();
	for f in plain_payload.list() {
		assert_eq!(payload.read(&f).unwrap(), plain_payload.read(&f).unwrap());
	}
	assert!(
		payload
			.list()
			.contains(&"_project/installer/installer.toml".to_owned())
	);

	let dir = tempfile::tempdir().unwrap();
	extract_project(&lip, dir.path()).unwrap();
	let back = Project::load(dir.path()).unwrap();
	assert_eq!(back.installer, p.installer);
	assert_eq!(back.components, p.components);
	assert_eq!(
		back.flows.keys().collect::<Vec<_>>(),
		p.flows.keys().collect::<Vec<_>>()
	);
	assert_eq!(
		back.themes["fixture"].layouts.len(),
		p.themes["fixture"].layouts.len()
	);
	assert_eq!(back.scripts, p.scripts);
}

#[test]
fn a_plain_payload_has_no_project_to_extract() {
	let p = project();
	let plain = pack_payload(&p, "linux-x86_64").unwrap();
	let lip = Lip::parse(&plain).unwrap();
	assert!(pinned_key(&lip).unwrap().is_none());
	let dir = tempfile::tempdir().unwrap();
	assert!(extract_project(&lip, dir.path()).is_err());
}
