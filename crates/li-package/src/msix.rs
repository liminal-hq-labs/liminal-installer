// Windows MSIX: `AppxManifest.xml`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::info::PackageInfo;
use crate::xml::esc;

/// Pads or trims a version to the four numeric parts MSIX requires: `1.2.3-beta` becomes
/// `1.2.3.0`.
pub fn four_part_version(version: &str) -> String {
	let mut parts: Vec<String> = version
		.split(['.', '-', '+'])
		.map(|p| {
			let digits: String = p.chars().take_while(char::is_ascii_digit).collect();
			if digits.is_empty() {
				"0".into()
			} else {
				digits
			}
		})
		.take(3)
		.collect();
	while parts.len() < 3 {
		parts.push("0".into());
	}
	parts.push("0".into());
	parts.join(".")
}

/// `AppxManifest.xml` for a full-trust desktop app. The `Publisher` must match the signing
/// certificate's subject when the package is signed.
pub fn manifest(info: &PackageInfo) -> String {
	let identity: String = info
		.slug()
		.chars()
		.filter(|c| c.is_ascii_alphanumeric() || *c == '-')
		.collect();
	let exe = info
		.executable
		.as_deref()
		.unwrap_or("app.exe")
		.replace('/', "\\");
	format!(
		r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities" IgnorableNamespaces="uap rescap">
	<Identity Name="{identity}" Publisher="CN={publisher}" Version="{version}" ProcessorArchitecture="{arch}"/>
	<Properties>
		<DisplayName>{name}</DisplayName>
		<PublisherDisplayName>{publisher}</PublisherDisplayName>
		<Logo>Assets\StoreLogo.png</Logo>
	</Properties>
	<Dependencies>
		<TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.22621.0"/>
	</Dependencies>
	<Resources>
		<Resource Language="en-us"/>
	</Resources>
	<Applications>
		<Application Id="App" Executable="{exe}" EntryPoint="Windows.FullTrustApplication">
			<uap:VisualElements DisplayName="{name}" Description="{description}" BackgroundColor="transparent" Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png"/>
		</Application>
	</Applications>
	<Capabilities>
		<rescap:Capability Name="runFullTrust"/>
	</Capabilities>
</Package>
"#,
		identity = esc(&identity),
		publisher = esc(&info.publisher),
		version = four_part_version(&info.version),
		arch = info.arch.msix(),
		name = esc(&info.name),
		description = esc(&info.description),
		exe = esc(&exe),
	)
}
