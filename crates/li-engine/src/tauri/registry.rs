// The registry values Tauri's NSIS installer writes, which the native path reproduces.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! From the Windows spike (`docs/tauri-windows-spike.md#registry-map`): the Add or Remove Programs
//! key, the publisher and product key, and the classes for file types and deep links. One list
//! feeds the native plan (as `registry.set` operations) and the simulated NSIS installer, so the
//! two cannot disagree about what "one app" looks like.

use li_model::TauriSnapshot;
use li_model::tauri::NsisInstallMode;

/// One value to write. `name` is empty for a key's default value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegWrite {
	pub key: String,
	pub name: String,
	pub value: String,
}

fn w(key: String, name: &str, value: impl Into<String>) -> RegWrite {
	RegWrite {
		key,
		name: name.to_owned(),
		value: value.into(),
	}
}

/// `REG_DWORD` values are written as `dword:xxxxxxxx`, as in a `.reg` file.
pub fn dword(n: u32) -> String {
	format!("dword:{n:08x}")
}

/// The publisher Tauri shows: `bundle.publisher`, else the second part of the identifier.
pub fn publisher_of(snap: &TauriSnapshot) -> String {
	snap.publisher.clone().unwrap_or_else(|| {
		snap.identifier
			.split('.')
			.nth(1)
			.unwrap_or(&snap.identifier)
			.to_owned()
	})
}

/// The Add or Remove Programs key under a hive.
pub fn uninstall_key(hive: &str, product_name: &str) -> String {
	format!(r"{hive}\Software\Microsoft\Windows\CurrentVersion\Uninstall\{product_name}")
}

/// The publisher and product key under a hive (its default value is the install directory).
pub fn product_key(hive: &str, publisher: &str, product_name: &str) -> String {
	format!(r"{hive}\Software\{publisher}\{product_name}")
}

/// Every value the installer writes for a per-user (`HKCU`) or per-machine (`HKLM`) install.
/// `install` is the install directory, `uninstall_string` the command Add or Remove Programs
/// runs, written as given (quoted path first), and `estimated_kb` the size.
pub fn registry_writes(
	snap: &TauriSnapshot,
	hive: &str,
	install: &str,
	uninstall_string: &str,
	estimated_kb: u32,
) -> Vec<RegWrite> {
	let exe = format!(r"{install}\{}.exe", snap.binary_stem());
	let product = &snap.product_name;
	let mut out = Vec::new();
	let key = uninstall_key(hive, product);
	out.push(w(key.clone(), "DisplayName", product.clone()));
	out.push(w(key.clone(), "DisplayVersion", snap.app_version.clone()));
	out.push(w(key.clone(), "Publisher", publisher_of(snap)));
	out.push(w(key.clone(), "DisplayIcon", format!("\"{exe}\"")));
	out.push(w(key.clone(), "InstallLocation", format!("\"{install}\"")));
	out.push(w(key.clone(), "UninstallString", uninstall_string));
	out.push(w(
		key.clone(),
		"MainBinaryName",
		format!("{}.exe", snap.binary_stem()),
	));
	out.push(w(key.clone(), "EstimatedSize", dword(estimated_kb)));
	out.push(w(key.clone(), "NoModify", dword(1)));
	out.push(w(key.clone(), "NoRepair", dword(1)));
	if let Some(home) = &snap.homepage {
		for name in ["URLInfoAbout", "URLUpdateInfo", "HelpLink"] {
			out.push(w(key.clone(), name, home.clone()));
		}
	}
	if snap.windows.install_mode == NsisInstallMode::Both {
		out.push(w(
			key.clone(),
			if hive == "HKLM" {
				"AllUsers"
			} else {
				"CurrentUser"
			},
			"1",
		));
	}
	out.push(w(
		product_key(hive, &publisher_of(snap), product),
		"",
		install.to_owned(),
	));

	let classes = format!(r"{hive}\Software\Classes");
	for assoc in &snap.file_associations {
		let name = &assoc.name;
		for ext in &assoc.ext {
			let ext = ext.trim_start_matches('.');
			out.push(w(format!(r"{classes}\.{ext}"), "", name.clone()));
			out.push(w(
				format!(r"{classes}\.{ext}"),
				&format!("{name}_backup"),
				"",
			));
		}
		let class = format!(r"{classes}\{name}");
		out.push(w(
			class.clone(),
			"",
			assoc.description.clone().unwrap_or_else(|| name.clone()),
		));
		out.push(w(
			format!(r"{class}\DefaultIcon"),
			"",
			format!("\"{exe}\",0"),
		));
		out.push(w(format!(r"{class}\shell"), "", "open"));
		out.push(w(
			format!(r"{class}\shell\open"),
			"",
			format!("Open with {product}"),
		));
		out.push(w(
			format!(r"{class}\shell\open\command"),
			"",
			format!("\"{exe}\" \"%1\""),
		));
	}
	for scheme in &snap.deep_link_schemes {
		let class = format!(r"{classes}\{scheme}");
		out.push(w(
			class.clone(),
			"",
			format!("URL:{} protocol", snap.identifier),
		));
		out.push(w(class.clone(), "URL Protocol", ""));
		out.push(w(
			format!(r"{class}\DefaultIcon"),
			"",
			format!("\"{exe}\",0"),
		));
		out.push(w(
			format!(r"{class}\shell\open\command"),
			"",
			format!("\"{exe}\" \"%1\""),
		));
	}
	out
}
