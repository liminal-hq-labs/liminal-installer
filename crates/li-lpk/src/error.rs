// Typed errors for the reader. Every failure on bad input is one of these; nothing panics.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

/// What a [`crate::Source`] can fail with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceError {
	/// The read went past the end of the volume.
	Eof,
	/// The device failed.
	Io,
	/// The person declined to supply a volume.
	Cancelled,
	/// The volume does not exist (a single file has only volume 1).
	NoVolume,
}

/// Why a compressed chunk could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflateError {
	/// The chunk's stored bytes ended inside the stream.
	InputEnd,
	/// A block type of 3.
	BadBlockType,
	/// A stored block whose length and its complement disagree.
	BadStoredLength,
	/// A code-length set that is over-subscribed, or a dynamic header that is out of range.
	BadLengths,
	/// A bit sequence that is no code of the table.
	BadCode,
	/// A back-reference to before the start of the chunk.
	BadDistance,
	/// More output than the chunk's decoded length.
	OutputOverflow,
	/// The stream ended before the chunk's decoded length, or stored bytes were left over.
	LengthMismatch,
}

/// Everything the reader can refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
	/// The volume does not start with the LPK magic.
	BadMagic,
	/// A major version this reader does not know.
	UnsupportedVersion,
	/// A must-understand flag this reader does not know.
	UnsupportedFeature,
	/// A header field is out of range.
	BadHeader,
	/// The header's checksum does not match.
	HeaderChecksum,
	/// The footer is missing or disagrees with the header.
	BadFooter,
	/// The volume ends before the data it promises.
	Truncated,
	/// The volume handed over is not the one asked for, or belongs to another set.
	WrongVolume,
	/// The directory's checksum does not match.
	DirectoryChecksum,
	/// A directory entry is malformed.
	BadEntry,
	/// A file name is unsafe or not 8.3 where it must be.
	BadName,
	/// A chunk header is malformed or points outside its volume.
	BadChunk,
	/// A chunk's checksum does not match.
	ChunkChecksum,
	/// A chunk method this reader does not know.
	UnsupportedMethod,
	/// A compressed chunk is damaged.
	Inflate(InflateError),
	/// The chunks do not add up to the file's size.
	SizeMismatch,
	/// The file's checksum does not match what was unpacked.
	FileChecksum,
	/// A buffer the caller supplied is too small.
	BufferTooSmall,
	/// The source failed or declined.
	Source(SourceError),
	/// The sink refused the data.
	Sink,
}

impl Error {
	/// A short description with no allocation.
	pub fn message(&self) -> &'static str {
		match self {
			Error::BadMagic => "not an LPK file",
			Error::UnsupportedVersion => "an LPK version this program does not know",
			Error::UnsupportedFeature => "the pack needs a feature this program lacks",
			Error::BadHeader => "the pack header is damaged",
			Error::HeaderChecksum => "the pack header does not match its checksum",
			Error::BadFooter => "the pack footer is missing or damaged",
			Error::Truncated => "the pack is cut short",
			Error::WrongVolume => "that is not the next disk of this set",
			Error::DirectoryChecksum => "the file list does not match its checksum",
			Error::BadEntry => "a file list entry is damaged",
			Error::BadName => "a file name in the pack is not allowed",
			Error::BadChunk => "a block of the pack is damaged",
			Error::ChunkChecksum => "a block of the pack does not match its checksum",
			Error::UnsupportedMethod => "the pack uses a compression this program lacks",
			Error::Inflate(_) => "compressed data is damaged",
			Error::SizeMismatch => "a file's blocks do not add up to its size",
			Error::FileChecksum => "a file does not match its checksum",
			Error::BufferTooSmall => "a buffer is too small for the pack",
			Error::Source(SourceError::Cancelled) => "cancelled",
			Error::Source(SourceError::NoVolume) => "the disk is not available",
			Error::Source(_) => "the pack could not be read",
			Error::Sink => "the output was refused",
		}
	}
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.message())
	}
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
