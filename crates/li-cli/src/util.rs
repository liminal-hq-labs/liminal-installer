// Small helpers: tab-indented JSON and UTC timestamps.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{CliError, CliResult};

/// Pretty JSON indented with tabs, ending in a newline, as the repository's editor settings ask.
pub fn to_json_tabs<T: Serialize>(value: &T) -> String {
	let mut buf = Vec::new();
	let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
	value.serialize(&mut ser).expect("JSON values serialize");
	let mut s = String::from_utf8(buf).expect("JSON is UTF-8");
	s.push('\n');
	s
}

pub fn sha256_hex(bytes: &[u8]) -> String {
	Sha256::digest(bytes)
		.iter()
		.map(|b| format!("{b:02x}"))
		.collect()
}

/// The current time as RFC 3339 in UTC, to the second.
pub fn now_rfc3339() -> String {
	let secs = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map_or(0, |d| d.as_secs());
	rfc3339(secs)
}

pub(crate) fn rfc3339(secs: u64) -> String {
	let days = (secs / 86_400) as i64;
	let rem = secs % 86_400;
	// Civil-from-days, after Howard Hinnant's date algorithms.
	let z = days + 719_468;
	let era = z.div_euclid(146_097);
	let doe = z.rem_euclid(146_097);
	let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
	let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
	let mp = (5 * doy + 2) / 153;
	let d = doy - (153 * mp + 2) / 5 + 1;
	let m = if mp < 10 { mp + 3 } else { mp - 9 };
	let y = yoe + era * 400 + i64::from(m <= 2);
	format!(
		"{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
		rem / 3600,
		rem % 3600 / 60,
		rem % 60
	)
}

pub fn write_file(path: &Path, contents: &str) -> CliResult<()> {
	if let Some(dir) = path.parent() {
		std::fs::create_dir_all(dir)
			.map_err(|e| CliError::new(format!("cannot create {}: {e}", dir.display())))?;
	}
	std::fs::write(path, contents)
		.map_err(|e| CliError::new(format!("cannot write {}: {e}", path.display())))
}

pub fn write_file_bytes(path: &Path, contents: &[u8]) -> CliResult<()> {
	if let Some(dir) = path.parent() {
		std::fs::create_dir_all(dir)
			.map_err(|e| CliError::new(format!("cannot create {}: {e}", dir.display())))?;
	}
	std::fs::write(path, contents)
		.map_err(|e| CliError::new(format!("cannot write {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn formats_timestamps() {
		assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
		assert_eq!(rfc3339(1_795_305_600), "2026-11-22T00:00:00Z");
		assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
	}

	#[test]
	fn json_uses_tabs() {
		assert_eq!(
			to_json_tabs(&serde_json::json!({"a": [1]})),
			"{\n\t\"a\": [\n\t\t1\n\t]\n}\n"
		);
	}
}
