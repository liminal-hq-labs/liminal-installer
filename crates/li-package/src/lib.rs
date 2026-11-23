// Writers for Linux, Windows, macOS and FreeBSD installer outputs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Output generators: everything here is pure text or file generation from a [`PackageInfo`].
//!
//! | Module | Output |
//! |---|---|
//! | [`run`] | self-extracting `.run` for Linux and FreeBSD (shell stub plus a payload archive) |
//! | [`deb`] | Debian control tree, and `dpkg-deb` when present |
//! | [`rpm`] | RPM `.spec` and its source tarball |
//! | [`arch`] | Arch `PKGBUILD` and its source tarball |
//! | [`wix`] | WiX v4 `.wxs` source |
//! | [`msix`] | MSIX `AppxManifest.xml` |
//! | [`macos`] | `.pkg` distribution XML and the installer `.app`'s `Info.plist` |
//! | [`dos`] | a stored zip and a FAT12 1.44 MB floppy image for the DOS runtime |
//! | [`freebsd`] | `+MANIFEST` for `pkg create` |
//!
//! Every native package ships the receipt of `SPEC.md` §9 in its payload, so the themed
//! installer and the Hub can update, repair or remove a package that was installed natively.

pub mod arch;
pub mod deb;
pub mod dos;
pub mod freebsd;
mod info;
pub mod macos;
pub mod msix;
mod payload;
pub mod rpm;
pub mod run;
pub mod wix;
mod xml;

pub use info::{Arch, Content, Entry, PackageInfo, resolve_version};
pub use payload::{entries_tar_gz, payload_tar_gz, receipt_for};

/// Failures while collecting inputs or writing outputs.
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
	#[error("{path}: {source}")]
	Io {
		path: String,
		#[source]
		source: std::io::Error,
	},
	#[error("component `{component}` lists `{from}`, which matches no file")]
	MissingFile { component: String, from: String },
	#[error(
		"component `{component}` uses `{placeholder}` in `{path}`; packages only support `{{install}}`"
	)]
	Placeholder {
		component: String,
		placeholder: String,
		path: String,
	},
	#[error("the product version is read from `{0}` at build time; pass a version explicitly")]
	VersionNeeded(String),
	#[error("`{0}` is not on the PATH")]
	ToolMissing(String),
	#[error("`{tool}` failed: {detail}")]
	ToolFailed { tool: String, detail: String },
}

impl PackageError {
	pub fn io(path: impl AsRef<std::path::Path>, source: std::io::Error) -> Self {
		PackageError::Io {
			path: path.as_ref().display().to_string(),
			source,
		}
	}
}

pub type Result<T> = std::result::Result<T, PackageError>;
