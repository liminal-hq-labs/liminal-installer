// The payload archive and the receipt every package carries.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use flate2::Compression;
use flate2::write::GzEncoder;
use li_model::{Operation, Receipt};

use crate::info::{Entry, PackageInfo};
use crate::xml::sha256_hex;
use crate::{PackageError, Result};

/// The receipt for a native package or `.run` install of `info`: one `copy` per file.
pub fn receipt_for(info: &PackageInfo) -> Result<Receipt> {
	let mut operations = Vec::new();
	for e in &info.files {
		operations.push(Operation::Copy {
			from: e.dest.clone(),
			to: format!("{{install}}/{}", e.dest),
			sha256: Some(sha256_hex(&e.read()?)),
		});
	}
	operations.push(Operation::ReceiptWrite {
		path: format!("{{install}}/{}", Receipt::PATH),
	});
	Ok(Receipt {
		version: Receipt::VERSION,
		product: info.id.clone(),
		installed: info.version.clone(),
		target: info.receipt_target.clone(),
		scope: info.scope,
		feed: None,
		channel: None,
		components: info.components.clone(),
		operations,
		updates: None,
	})
}

pub(crate) fn receipt_entry(info: &PackageInfo) -> Result<Entry> {
	let mut json =
		serde_json::to_vec_pretty(&receipt_for(info)?).map_err(|e| PackageError::ToolFailed {
			tool: "receipt".into(),
			detail: e.to_string(),
		})?;
	json.push(b'\n');
	Ok(Entry::bytes(Receipt::PATH, &json, false))
}

/// A gzip tar of the payload, byte-for-byte reproducible: entries are sorted, owned by root and
/// carry a zero modification time. With `top`, every path sits under that directory.
pub fn payload_tar_gz(info: &PackageInfo, top: Option<&str>) -> Result<Vec<u8>> {
	entries_tar_gz(&info.payload()?, top)
}

/// The same archive for an explicit list of entries, with no receipt added.
pub fn entries_tar_gz(entries: &[Entry], top: Option<&str>) -> Result<Vec<u8>> {
	let mut builder = tar::Builder::new(Vec::new());
	for e in entries {
		let data = e.read()?;
		let mut header = tar::Header::new_gnu();
		header.set_size(data.len() as u64);
		header.set_mode(if e.executable { 0o755 } else { 0o644 });
		header.set_uid(0);
		header.set_gid(0);
		header.set_mtime(0);
		let path = match top {
			Some(t) => format!("{t}/{}", e.dest),
			None => e.dest.clone(),
		};
		builder
			.append_data(&mut header, path, data.as_slice())
			.map_err(|err| PackageError::io("payload", err))?;
	}
	let tar_bytes = builder
		.into_inner()
		.map_err(|err| PackageError::io("payload", err))?;
	let mut gz = GzEncoder::new(Vec::new(), Compression::new(9));
	std::io::Write::write_all(&mut gz, &tar_bytes)
		.map_err(|err| PackageError::io("payload", err))?;
	gz.finish().map_err(|err| PackageError::io("payload", err))
}
