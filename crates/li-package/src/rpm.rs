// RPM: the `.spec` file and its source tarball.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::Result;
use crate::info::PackageInfo;
use crate::payload::payload_tar_gz;

/// The source tarball `Source0` names: the payload under `<slug>-<version>/`.
pub fn source_tarball(info: &PackageInfo) -> Result<Vec<u8>> {
	payload_tar_gz(info, Some(&info.source_dir()))
}

/// The `.spec` file. Build it with `rpmbuild -bb` after placing the tarball in `SOURCES`.
pub fn spec(info: &PackageInfo) -> String {
	let slug = info.slug();
	let dir = info.linux_install_dir();
	let mut out = String::new();
	out.push_str("%global debug_package %{nil}\n%global __os_install_post %{nil}\n\n");
	out.push_str(&format!("Name:           {slug}\n"));
	out.push_str(&format!("Version:        {}\n", info.package_version()));
	out.push_str("Release:        1\n");
	out.push_str(&format!("Summary:        {}\n", info.name));
	out.push_str(&format!(
		"License:        {}\n",
		info.licence.as_deref().unwrap_or("Proprietary")
	));
	if let Some(h) = &info.homepage {
		out.push_str(&format!("URL:            {h}\n"));
	}
	out.push_str(&format!("Vendor:         {}\n", info.publisher));
	out.push_str("Source0:        %{name}-%{version}.tar.gz\n");
	out.push_str(&format!("BuildArch:      {}\n", info.arch.rpm()));
	out.push_str("AutoReqProv:    no\n\n");
	out.push_str(&format!("%description\n{}\n\n", info.description));
	out.push_str("%prep\n%setup -q\n\n%build\n\n%install\n");
	out.push_str(&format!("install -d %{{buildroot}}{dir}\n"));
	out.push_str(&format!("cp -a . %{{buildroot}}{dir}/\n"));
	if let Some(exe) = &info.executable {
		out.push_str("install -d %{buildroot}/usr/bin\n");
		out.push_str(&format!(
			"ln -s {dir}/{exe} %{{buildroot}}/usr/bin/{slug}\n"
		));
	}
	out.push_str(&format!("\n%files\n{dir}\n"));
	if info.executable.is_some() {
		out.push_str(&format!("/usr/bin/{slug}\n"));
	}
	out.push_str(&format!(
		"\n%changelog\n* Mon Jan 05 2026 {} <noreply@localhost> - {}-1\n- Packaged by liminst\n",
		info.publisher,
		info.package_version()
	));
	out
}
