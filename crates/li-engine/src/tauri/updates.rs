// Who updates a product, how often to look, and catching up when the app updated itself.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The conflict rule (`docs/tauri-apps.md#updates`): the version on the machine wins. The engine
//! applies a feed's release only when it is newer than what is installed, never installs over a
//! newer version, and when the app updated itself (the Tauri updater runs the product's own
//! installer, which bypasses the receipt) it brings the receipt up to what is there instead of
//! putting the old files back.

use std::path::Path;

use li_model::tauri::UpdateMode;
use li_model::{Operation, Receipt};

use crate::machine::Machine;
use crate::tauri::nsis::find_external;

/// The mode a receipt was written under; an older receipt, or one with no `[updates]`, means the
/// installer updates.
pub fn mode_of(receipt: &Receipt) -> UpdateMode {
	receipt.updates.as_ref().map(|u| u.mode).unwrap_or_default()
}

/// Whether the engine may apply feed updates to this receipt's product.
pub fn installer_owns(receipt: &Receipt) -> bool {
	mode_of(receipt).installer_updates()
}

/// Where the time of the last feed check is kept, relative to the install directory.
pub const LAST_CHECK: &str = ".liminal/last-update-check";

/// Whether a feed check is due at `now` (seconds since the Unix epoch): always when no interval
/// is set or nothing was recorded, else when the interval has passed.
pub fn check_due(
	m: &dyn Machine,
	install_dir: &Path,
	interval_hours: Option<u32>,
	now: u64,
) -> bool {
	let Some(hours) = interval_hours.filter(|h| *h > 0) else {
		return true;
	};
	let last = m
		.read(&install_dir.join(LAST_CHECK))
		.ok()
		.and_then(|b| String::from_utf8_lossy(&b).trim().parse::<u64>().ok());
	match last {
		Some(last) => now >= last.saturating_add(u64::from(hours) * 3600),
		None => true,
	}
}

/// Remembers that a feed check happened at `now`.
pub fn record_check(m: &mut dyn Machine, install_dir: &Path, now: u64) -> std::io::Result<()> {
	m.write(
		&install_dir.join(LAST_CHECK),
		format!("{now}\n").as_bytes(),
		false,
	)
}

/// The version of the product's own Add or Remove Programs entry: the Tauri installer's when it
/// ran, else the native entry the engine wrote.
pub fn installed_version(m: &dyn Machine, receipt: &Receipt) -> Option<String> {
	for op in &receipt.operations {
		match op {
			Operation::TauriWrap {
				external: Some(e), ..
			} => {
				let key = format!("{}\\{}", e.hive, e.uninstall_key);
				return m
					.registry_value(&key, "DisplayVersion")
					.filter(|v| !v.is_empty());
			}
			Operation::RegistrySet { key, name, .. }
				if name == "DisplayVersion" && key.contains(r"\Uninstall\") =>
			{
				return m.registry_value(key, name).filter(|v| !v.is_empty());
			}
			_ => {}
		}
	}
	None
}

/// The receipt brought up to what the machine has, or `None` when it already matches. For a
/// wrapped install the `external` entry is read again; for a native one every copy's hash is
/// dropped, so a repair restores missing files and never puts the old content back.
pub fn refreshed(m: &dyn Machine, receipt: &Receipt) -> Option<Receipt> {
	let now = installed_version(m, receipt)?;
	if now == receipt.installed {
		return None;
	}
	let mut next = receipt.clone();
	next.installed = now.clone();
	for op in &mut next.operations {
		match op {
			Operation::TauriWrap {
				external,
				product_name,
				publisher,
				..
			} => {
				if let Some(e) = external.as_ref()
					&& let Some(fresh) = find_external(m, product_name, publisher, &e.hive)
				{
					*external = Some(fresh);
				}
			}
			Operation::Copy { sha256, .. } => *sha256 = None,
			Operation::RegistrySet { key, name, value }
				if name == "DisplayVersion" && key.contains(r"\Uninstall\") =>
			{
				*value = now.clone();
			}
			_ => {}
		}
	}
	Some(next)
}
