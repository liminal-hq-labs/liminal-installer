// macOS: the `.pkg` distribution XML and the installer `.app`'s `Info.plist`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::info::PackageInfo;
use crate::xml::esc;

/// The reverse-DNS identifier of the component package: the product id, lowercased.
pub fn pkg_identifier(info: &PackageInfo) -> String {
	info.id.to_lowercase()
}

/// The distribution XML for `productbuild --distribution`. The component package is
/// `<slug>.pkg`, built with `pkgbuild --identifier <pkg_identifier>`.
pub fn distribution_xml(info: &PackageInfo) -> String {
	let id = esc(&pkg_identifier(info));
	let domain = if info.scope == li_model::Scope::Machine {
		"enable_localSystem=\"true\""
	} else {
		"enable_currentUserHome=\"true\""
	};
	format!(
		r#"<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
	<title>{title}</title>
	<organization>{org}</organization>
	<options customize="never" require-scripts="false" hostArchitectures="{arch}"/>
	<domains {domain}/>
	<pkg-ref id="{id}"/>
	<choices-outline>
		<line choice="default">
			<line choice="{id}"/>
		</line>
	</choices-outline>
	<choice id="default"/>
	<choice id="{id}" visible="false" title="{title}">
		<pkg-ref id="{id}"/>
	</choice>
	<pkg-ref id="{id}" version="{version}" onConclusion="none">{slug}.pkg</pkg-ref>
</installer-gui-script>
"#,
		title = esc(&info.name),
		org = esc(&info.publisher),
		arch = if info.arch == crate::Arch::Aarch64 {
			"arm64"
		} else {
			"x86_64"
		},
		version = esc(&info.version),
		slug = esc(&info.slug()),
	)
}

/// The `Info.plist` for the installer `.app` that wraps the engine.
pub fn info_plist(info: &PackageInfo) -> String {
	format!(
		r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleExecutable</key>
	<string>{exe}</string>
	<key>CFBundleIdentifier</key>
	<string>{id}.installer</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>{name} Installer</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSHumanReadableCopyright</key>
	<string>{publisher}</string>
</dict>
</plist>
"#,
		exe = esc(info.engine.as_deref().unwrap_or("liminal-installer")),
		id = esc(&pkg_identifier(info)),
		name = esc(&info.name),
		version = esc(&info.version),
		publisher = esc(&info.publisher),
	)
}
