// Arch Linux: the `PKGBUILD` and its source tarball.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::Result;
use crate::info::PackageInfo;
use crate::payload::payload_tar_gz;
use crate::xml::{sh_quote, sha256_hex};

/// The source tarball and its SHA-256, for `source=` and `sha256sums=`.
pub fn source_tarball(info: &PackageInfo) -> Result<(Vec<u8>, String)> {
	let bytes = payload_tar_gz(info, Some(&info.source_dir()))?;
	let sum = sha256_hex(&bytes);
	Ok((bytes, sum))
}

/// The `PKGBUILD`. Name the tarball `<slug>-<version>.tar.gz` beside it and run `makepkg`.
pub fn pkgbuild(info: &PackageInfo, source_sha256: &str) -> String {
	let slug = info.slug();
	let dir = info.linux_install_dir();
	let mut out = String::new();
	out.push_str(&format!("# Maintainer: {}\n", info.publisher));
	out.push_str(&format!("pkgname={}\n", sh_quote(&slug)));
	out.push_str(&format!("pkgver={}\n", sh_quote(&info.package_version())));
	out.push_str("pkgrel=1\n");
	out.push_str(&format!("pkgdesc={}\n", sh_quote(&info.description)));
	out.push_str(&format!("arch=({})\n", sh_quote(info.arch.rpm())));
	if let Some(h) = &info.homepage {
		out.push_str(&format!("url={}\n", sh_quote(h)));
	}
	out.push_str(&format!(
		"license=({})\n",
		sh_quote(info.licence.as_deref().unwrap_or("custom"))
	));
	out.push_str("options=('!strip')\n");
	out.push_str("source=(\"$pkgname-$pkgver.tar.gz\")\n");
	out.push_str(&format!("sha256sums=({})\n\n", sh_quote(source_sha256)));
	out.push_str("package() {\n");
	out.push_str(&format!("\tinstall -d \"$pkgdir{dir}\"\n"));
	out.push_str(&format!(
		"\tcp -a \"$srcdir/$pkgname-$pkgver/.\" \"$pkgdir{dir}/\"\n"
	));
	if let Some(exe) = &info.executable {
		out.push_str("\tinstall -d \"$pkgdir/usr/bin\"\n");
		out.push_str(&format!(
			"\tln -s {} \"$pkgdir/usr/bin/$pkgname\"\n",
			sh_quote(&format!("{dir}/{exe}"))
		));
	}
	out.push_str("}\n");
	out
}
