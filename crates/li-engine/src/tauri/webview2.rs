// WebView2 detection: the Evergreen runtime's client key, and the version rules around it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Microsoft documents the runtime's presence as a `pv` value under the Evergreen client key, in
//! the 64-bit machine view first and then the current user's hive; a missing, empty or
//! `0.0.0.0` value means the runtime is not installed. Tauri's NSIS template reads the same keys
//! (see `docs/tauri-windows-spike.md`).

use crate::machine::Machine;

/// The Evergreen runtime's client id.
pub const CLIENT_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
/// A machine-wide runtime. Microsoft documents this `WOW6432Node` key for 64-bit Windows, and
/// Tauri's installer reads it first.
pub const HKLM_CLIENT_KEY: &str = r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
/// A per-user runtime.
pub const HKCU_CLIENT_KEY: &str =
	r"HKCU\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
/// The value that holds the version.
pub const VERSION_VALUE: &str = "pv";
/// What the simulated machine's bootstrapper installs.
pub const SIM_INSTALLED_VERSION: &str = "131.0.2903.51";

/// A runtime found on the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
	pub version: String,
	/// `HKLM` for a machine-wide runtime, `HKCU` for a per-user one.
	pub hive: &'static str,
}

fn usable(v: &str) -> bool {
	let v = v.trim();
	!v.is_empty() && v != "0.0.0.0"
}

/// Looks for the runtime in `HKLM` and then `HKCU`.
pub fn detect(m: &dyn Machine) -> Option<Detected> {
	[("HKLM", HKLM_CLIENT_KEY), ("HKCU", HKCU_CLIENT_KEY)]
		.into_iter()
		.find_map(|(hive, key)| {
			m.registry_value(key, VERSION_VALUE)
				.filter(|v| usable(v))
				.map(|v| Detected {
					version: v.trim().to_owned(),
					hive,
				})
		})
}

fn parts(v: &str) -> Vec<u64> {
	v.split('.').map_while(|p| p.trim().parse().ok()).collect()
}

/// Whether `found` is at least `min`, comparing dotted numbers field by field.
pub fn at_least(found: &str, min: &str) -> bool {
	let (mut a, mut b) = (parts(found), parts(min));
	let n = a.len().max(b.len());
	a.resize(n, 0);
	b.resize(n, 0);
	a >= b
}

/// Whether the step has to install: nothing is there, or what is there is older than `min`.
pub fn needs_install(detected: Option<&Detected>, min_version: Option<&str>) -> bool {
	match (detected, min_version) {
		(None, _) => true,
		(Some(d), Some(min)) => !at_least(&d.version, min),
		(Some(_), None) => false,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::sim::SimMachine;

	#[test]
	fn nothing_is_found_on_a_bare_machine() {
		assert_eq!(detect(&SimMachine::new()), None);
	}

	#[test]
	fn a_machine_wide_runtime_wins_over_a_per_user_one() {
		let m = SimMachine::new().with_webview2("130.0.1");
		m.set_registry_value(HKCU_CLIENT_KEY, VERSION_VALUE, "120.0.0");
		assert_eq!(
			detect(&m),
			Some(Detected {
				version: "130.0.1".into(),
				hive: "HKLM"
			})
		);
	}

	#[test]
	fn a_per_user_runtime_is_found_when_there_is_no_machine_one() {
		let m = SimMachine::new();
		m.set_registry_value(HKCU_CLIENT_KEY, VERSION_VALUE, "131.0.2903.51");
		assert_eq!(detect(&m).map(|d| d.hive), Some("HKCU"));
	}

	#[test]
	fn zero_and_empty_versions_mean_not_installed() {
		let m = SimMachine::new();
		m.set_registry_value(HKLM_CLIENT_KEY, VERSION_VALUE, "0.0.0.0");
		m.set_registry_value(HKCU_CLIENT_KEY, VERSION_VALUE, "");
		assert_eq!(detect(&m), None);
	}

	#[test]
	fn versions_compare_by_number_not_by_text() {
		assert!(at_least("131.0.2903.51", "110.0.1587.40"));
		assert!(at_least("110.0.1587.40", "110.0.1587.40"));
		assert!(at_least("110.0.1587", "110.0.1587.0"));
		assert!(!at_least("99.0.0.0", "110.0.0.0"));
		assert!(at_least("1000.0", "999.9"));
	}

	#[test]
	fn an_old_runtime_needs_an_install_only_when_a_minimum_is_set() {
		let old = Detected {
			version: "99.0.0.0".into(),
			hive: "HKLM",
		};
		assert!(needs_install(None, None));
		assert!(!needs_install(Some(&old), None));
		assert!(needs_install(Some(&old), Some("110.0.0.0")));
		assert!(!needs_install(Some(&old), Some("98.0.0.0")));
	}
}
