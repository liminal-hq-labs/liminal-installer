// FreeBSD: the `+MANIFEST` for `pkg create`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

use serde_json::json;

use crate::Result;
use crate::info::PackageInfo;
use crate::xml::sha256_hex;

/// The `+MANIFEST`, as JSON (which `pkg` reads as UCL). Files install under
/// `/usr/local/lib/<slug>`; build with `pkg create -M +MANIFEST -r <staging> -o <out>`.
pub fn manifest(info: &PackageInfo, abi_major: u32) -> Result<String> {
	let slug = info.slug();
	let mut files = BTreeMap::new();
	for e in info.payload()? {
		files.insert(
			format!("/usr/local/lib/{slug}/{}", e.dest),
			sha256_hex(&e.read()?),
		);
	}
	let mut v = json!({
		"name": slug,
		"version": info.package_version(),
		"origin": format!("misc/{slug}"),
		"comment": info.name,
		"maintainer": "noreply@localhost",
		"www": info.homepage.clone().unwrap_or_else(|| "https://localhost/".into()),
		"abi": format!("FreeBSD:{abi_major}:{}", info.arch.freebsd()),
		"arch": format!("freebsd:{abi_major}:{}", info.arch.freebsd()),
		"prefix": "/usr/local",
		"desc": info.description,
		"files": files,
	});
	if let Some(l) = &info.licence {
		v["licenses"] = json!([l]);
		v["licenselogic"] = json!("single");
	}
	let mut s = serde_json::to_string_pretty(&v).map_err(|e| crate::PackageError::ToolFailed {
		tool: "manifest".into(),
		detail: e.to_string(),
	})?;
	s.push('\n');
	Ok(s)
}
