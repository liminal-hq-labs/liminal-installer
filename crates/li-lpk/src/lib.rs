// The Liminal Pack (`.LPK`): files in chunks of at most 32 KiB, each stored or deflated.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The Liminal Pack format, `.LPK`: one container for the DOS, Win16 and Win32 installer
//! runtimes. The byte-level specification is `docs/lpk-format.md`; where this crate and that page
//! differ, the crate is wrong.
//!
//! The reader (everything outside the `writer` feature) is `no_std`, allocates nothing and uses
//! no `unsafe`. A [`Reader`] pulls bytes from a [`Source`] (passed to each call, so the caller can
//! use the same device for other work between chunks) through a 256-byte buffer and decodes one
//! chunk at a time into a window the caller owns (32 KiB at most), so a 16-bit runtime needs no
//! large allocation. Every failure on bad input is an [`Error`]; nothing panics.
//!
//! ```
//! use li_lpk::{Reader, SinkError, SliceSource};
//!
//! # fn demo(pack: &[u8]) -> Result<(), li_lpk::Error> {
//! let volumes = [pack];
//! let mut src = SliceSource::new(&volumes);
//! let mut reader = Reader::open(&mut src)?;
//! let mut dir_bytes = [0u8; 4096];
//! let dir = reader.read_directory(&mut src, &mut dir_bytes)?;
//! let mut window = [0u8; 32768];
//! for entry in dir.entries() {
//!     let entry = entry?;
//!     let mut total = 0usize;
//!     reader.extract(&mut src, &entry, &mut window, &mut |data: &[u8]| {
//!         total += data.len();
//!         Ok::<(), SinkError>(())
//!     })?;
//! }
//! # Ok(())
//! # }
//! ```
#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "writer")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod crc;
mod error;
mod format;
mod inflate;
mod reader;

#[cfg(feature = "writer")]
pub mod writer;

#[cfg(feature = "std")]
pub mod host;

pub use crc::{Crc, crc32};
pub use error::{Error, InflateError, SourceError};
pub use format::{
	CHUNK_HEADER_LEN, ChunkHeader, Directory, ENTRY_FIXED, Entries, Entry, FOOTER_LEN,
	FOOTER_MAGIC, Footer, HEADER_LEN, Header, MAGIC, MAX_CHUNK_LOG2, MAX_ENTRY, MAX_LONG,
	MAX_SHORT, METHOD_DEFLATE, METHOD_NEXT_VOLUME, METHOD_STORED, MIN_CHUNK_LOG2, VERSION_MAJOR,
	VERSION_MINOR, entry_len, flags, safe_path, short_path, targets,
};
pub use inflate::INBUF;
pub use reader::{Reader, SINK_PIECE, Sink, SinkError, SliceSource, Source};
