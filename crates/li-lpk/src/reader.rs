// The reader: opens a pack through a `Source`, reads the directory, and unpacks files into a
// `Sink` one chunk at a time. It allocates nothing and asks the caller for each volume.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::crc::{Crc, crc32};
use crate::error::{Error, InflateError, SourceError};
use crate::format::{
	CHUNK_HEADER_LEN, ChunkHeader, Directory, Entry, FOOTER_LEN, Footer, HEADER_LEN, Header,
	METHOD_DEFLATE, METHOD_NEXT_VOLUME, METHOD_STORED,
};
use crate::inflate::{INBUF, Input, inflate};

/// Where the bytes come from. Offsets are from the start of the pack in the current volume, so a
/// self-extracting stub adds the pack's start. A set of disks is a set of volumes: the reader
/// calls [`Source::open_volume`] when it needs another, and the implementation asks the person.
pub trait Source {
	/// Makes volume `volume` (from 1) the one `read_at` reads. Called again for the same volume
	/// only after another was opened. An implementation that cannot supply it returns
	/// [`SourceError::NoVolume`], or [`SourceError::Cancelled`] when the person declines.
	fn open_volume(&mut self, volume: u16) -> Result<(), SourceError>;

	/// Fills `buf` from `offset` in the open volume, or fails with [`SourceError::Eof`] when the
	/// volume ends first.
	fn read_at(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), SourceError>;
}

/// The sink refused the data. Keep the reason on your side; the reader reports [`Error::Sink`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkError;

/// Where unpacked bytes go.
pub trait Sink {
	/// Takes the next piece of the file, at most 4 KiB.
	fn write(&mut self, data: &[u8]) -> Result<(), SinkError>;
}

impl<F: FnMut(&[u8]) -> Result<(), SinkError>> Sink for F {
	fn write(&mut self, data: &[u8]) -> Result<(), SinkError> {
		self(data)
	}
}

/// The largest piece handed to a [`Sink`], so progress and cancel stay responsive.
pub const SINK_PIECE: usize = 4096;

/// A pack being read: volume 1's header and which volume is open. It holds no reference to the
/// source, so the caller passes the source to each call and may use the same device for other
/// work (writing the unpacked files, say) in between.
pub struct Reader {
	first: Header,
	cur: Header,
	buf: [u8; INBUF],
}

/// One file being unpacked, a chunk at a time. See [`Reader::begin`].
#[derive(Debug, Clone, Copy)]
pub struct Extract {
	at: u32,
	left: u32,
	crc: Crc,
	want: u32,
	volume: u16,
	/// Whether the last chunk header read was a next-volume marker.
	marked: bool,
}

fn map(e: SourceError) -> Error {
	match e {
		SourceError::Eof => Error::Truncated,
		e => Error::Source(e),
	}
}

/// Reads and checks the header and footer of the volume the source has open.
fn read_volume(src: &mut dyn Source) -> Result<Header, Error> {
	let mut hb = [0u8; HEADER_LEN];
	src.read_at(0, &mut hb).map_err(map)?;
	let h = Header::parse(&hb)?;
	let mut fb = [0u8; FOOTER_LEN];
	src.read_at(h.footer_at(), &mut fb).map_err(map)?;
	if Footer::parse(&fb)?.matches(&h) {
		Ok(h)
	} else {
		Err(Error::BadFooter)
	}
}

impl Reader {
	/// Opens volume 1 and checks its header and footer.
	pub fn open(src: &mut dyn Source) -> Result<Reader, Error> {
		src.open_volume(1).map_err(map)?;
		let h = read_volume(src)?;
		if h.volume != 1 {
			return Err(Error::WrongVolume);
		}
		Ok(Reader {
			first: h,
			cur: h,
			buf: [0; INBUF],
		})
	}

	/// Volume 1's header: the file count, the total size, the volume count and the set id.
	pub fn header(&self) -> &Header {
		&self.first
	}

	/// The number of volumes in the set.
	pub fn volume_count(&self) -> u16 {
		self.first.volume_count
	}

	/// The bytes `buf` must hold for [`Reader::read_directory`].
	pub fn directory_len(&self) -> usize {
		self.first.dir_len as usize
	}

	/// Moves to `volume`, asking the source for it and checking that it belongs to this set.
	fn enter(&mut self, src: &mut dyn Source, volume: u16) -> Result<(), Error> {
		if self.cur.volume == volume {
			return Ok(());
		}
		src.open_volume(volume).map_err(map)?;
		let h = read_volume(src)?;
		let f = &self.first;
		if h.volume != volume
			|| h.set_id != f.set_id
			|| h.volume_count != f.volume_count
			|| h.chunk_log2 != f.chunk_log2
		{
			return Err(Error::WrongVolume);
		}
		self.cur = h;
		Ok(())
	}

	/// Reads the directory into `buf` (at least [`Reader::directory_len`] bytes), checks its
	/// checksum and returns it. The directory lives in volume 1; read it once and keep it, so a
	/// disk set is not swapped to look a file up.
	pub fn read_directory<'b>(
		&mut self,
		src: &mut dyn Source,
		buf: &'b mut [u8],
	) -> Result<Directory<'b>, Error> {
		let n = self.first.dir_len as usize;
		let buf = buf.get_mut(..n).ok_or(Error::BufferTooSmall)?;
		self.enter(src, 1)?;
		src.read_at(self.first.dir_offset, buf).map_err(map)?;
		if crc32(buf) != self.first.dir_crc {
			return Err(Error::DirectoryChecksum);
		}
		Ok(Directory::new(buf, &self.first))
	}

	/// Starts unpacking `e`: asks the source for the volume it begins in. Call
	/// [`Reader::step`] until it returns `None`.
	pub fn begin(&mut self, src: &mut dyn Source, e: &Entry) -> Result<Extract, Error> {
		if e.size != 0 {
			self.enter(src, e.first_volume)?;
		}
		Ok(Extract {
			at: e.first_offset,
			left: e.size,
			crc: Crc::default(),
			want: e.crc,
			volume: e.first_volume,
			marked: false,
		})
	}

	/// Decodes the next chunk of the file into the start of `window` and returns how many bytes
	/// it holds, or `None` when the file is complete and its checksum matched. `window` must be
	/// at least the pack's chunk size (see [`Header::chunk_size`]; 32 KiB is always enough). The
	/// chunk has been checked before it is returned, so damaged data is never handed out.
	pub fn step(
		&mut self,
		src: &mut dyn Source,
		x: &mut Extract,
		window: &mut [u8],
	) -> Result<Option<usize>, Error> {
		if window.len() < self.first.chunk_size() {
			return Err(Error::BufferTooSmall);
		}
		loop {
			if x.left == 0 {
				return if x.crc.value() == x.want {
					Ok(None)
				} else {
					Err(Error::FileChecksum)
				};
			}
			let h = self.cur;
			let mut hb = [0u8; CHUNK_HEADER_LEN];
			let end = x.at.checked_add(CHUNK_HEADER_LEN as u32);
			if x.at < h.data_start() || end.is_none_or(|e| e > h.footer_at()) {
				return Err(Error::BadChunk);
			}
			src.read_at(x.at, &mut hb).map_err(map)?;
			let c = ChunkHeader::parse(&hb);
			let mut crc = Crc::default();
			crc.update(ChunkHeader::prefix(&hb));
			if c.method == METHOD_NEXT_VOLUME {
				if crc.value() != c.crc
					|| c.raw_len != 0
					|| c.stored_len != 0
					|| c.arg != h.volume.wrapping_add(1)
					|| x.marked
				{
					return Err(Error::BadChunk);
				}
				x.marked = true;
				self.enter(src, c.arg)?;
				x.volume = c.arg;
				x.at = self.cur.data_start();
				continue;
			}
			x.marked = false;
			let (raw, stored) = (usize::from(c.raw_len), usize::from(c.stored_len));
			let body = end.unwrap_or(0);
			if raw as u32 > x.left {
				return Err(Error::SizeMismatch);
			}
			if raw == 0
				|| raw > h.chunk_size()
				|| stored > h.chunk_size()
				|| c.arg != 0
				|| body
					.checked_add(stored as u32)
					.is_none_or(|e| e > h.footer_at())
			{
				return Err(Error::BadChunk);
			}
			let out = &mut window[..raw];
			let mut inp = Input::new(src, &mut self.buf, body, stored as u32, crc);
			match c.method {
				METHOD_STORED => {
					if stored != raw {
						return Err(Error::BadChunk);
					}
					inp.copy_to(out)?;
				}
				METHOD_DEFLATE => {
					if inflate(&mut inp, out)? != raw || !inp.drained() {
						return Err(Error::Inflate(InflateError::LengthMismatch));
					}
				}
				_ => return Err(Error::UnsupportedMethod),
			}
			if inp.crc.value() != c.crc {
				return Err(Error::ChunkChecksum);
			}
			x.crc.update(out);
			x.left -= raw as u32;
			x.at = body + stored as u32;
			return Ok(Some(raw));
		}
	}

	/// Unpacks one file into `sink`, in pieces of at most [`SINK_PIECE`] bytes.
	pub fn extract(
		&mut self,
		src: &mut dyn Source,
		e: &Entry,
		window: &mut [u8],
		sink: &mut dyn Sink,
	) -> Result<(), Error> {
		let mut x = self.begin(src, e)?;
		while let Some(n) = self.step(src, &mut x, window)? {
			for piece in window[..n].chunks(SINK_PIECE) {
				sink.write(piece).map_err(|_| Error::Sink)?;
			}
		}
		Ok(())
	}
}

/// A single-volume source over a slice, or a set of volumes over several. No allocation.
pub struct SliceSource<'a> {
	volumes: &'a [&'a [u8]],
	open: usize,
}

impl<'a> SliceSource<'a> {
	/// One volume per slice, volume 1 first.
	pub fn new(volumes: &'a [&'a [u8]]) -> Self {
		SliceSource { volumes, open: 0 }
	}
}

impl Source for SliceSource<'_> {
	fn open_volume(&mut self, volume: u16) -> Result<(), SourceError> {
		let i = usize::from(volume).wrapping_sub(1);
		if i >= self.volumes.len() {
			return Err(SourceError::NoVolume);
		}
		self.open = i;
		Ok(())
	}

	fn read_at(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), SourceError> {
		let v = self.volumes.get(self.open).ok_or(SourceError::NoVolume)?;
		let start = offset as usize;
		let src = start
			.checked_add(buf.len())
			.and_then(|end| v.get(start..end))
			.ok_or(SourceError::Eof)?;
		buf.copy_from_slice(src);
		Ok(())
	}
}
