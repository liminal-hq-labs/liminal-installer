// Host helpers: volumes as files, a directory tree to a pack and back, and 8.3 name generation.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! What the `liminst lpk` command and the tests build on. Everything here is `std`; none of it is
//! needed to read a pack on DOS or Windows.

use std::collections::BTreeSet;
use std::fmt;
use std::format;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::string::{String, ToString};
use std::vec::Vec;

use crate::writer::{FileSpec, Options, PackError, Packed, dos_from_unix, pack};
use crate::{Error, Reader, SinkError, Source, SourceError, flags};

/// Why a host operation failed.
#[derive(Debug)]
pub enum HostError {
	Io(String),
	Lpk(Error),
	Pack(PackError),
	/// A source file name that cannot be stored (not UTF-8, or too long).
	Name(String),
}

impl fmt::Display for HostError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			HostError::Io(m) | HostError::Name(m) => f.write_str(m),
			HostError::Lpk(e) => e.fmt(f),
			HostError::Pack(e) => e.fmt(f),
		}
	}
}

impl std::error::Error for HostError {}

impl From<Error> for HostError {
	fn from(e: Error) -> Self {
		HostError::Lpk(e)
	}
}

impl From<PackError> for HostError {
	fn from(e: PackError) -> Self {
		HostError::Pack(e)
	}
}

fn io_err(path: &Path, e: io::Error) -> HostError {
	HostError::Io(format!("{}: {e}", path.display()))
}

/// The file that holds volume `n` of the set whose first volume is `first`: the first volume as
/// named, then the same name with the extension `L02`, `L03` and so on up to `L99`.
pub fn volume_path(first: &Path, n: u16) -> Option<PathBuf> {
	match n {
		0 => None,
		1 => Some(first.to_path_buf()),
		2..=99 => Some(first.with_extension(format!("L{n:02}"))),
		_ => None,
	}
}

/// Volumes as files next to each other, found by [`volume_path`].
pub struct VolumeFiles {
	first: PathBuf,
	open: Option<File>,
}

impl VolumeFiles {
	pub fn new(first: impl Into<PathBuf>) -> Self {
		VolumeFiles {
			first: first.into(),
			open: None,
		}
	}
}

impl Source for VolumeFiles {
	fn open_volume(&mut self, volume: u16) -> Result<(), SourceError> {
		let path = volume_path(&self.first, volume).ok_or(SourceError::NoVolume)?;
		match File::open(path) {
			Ok(f) => {
				self.open = Some(f);
				Ok(())
			}
			Err(e) if e.kind() == io::ErrorKind::NotFound => Err(SourceError::NoVolume),
			Err(_) => Err(SourceError::Io),
		}
	}

	fn read_at(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), SourceError> {
		let f = self.open.as_mut().ok_or(SourceError::NoVolume)?;
		f.seek(SeekFrom::Start(u64::from(offset)))
			.map_err(|_| SourceError::Io)?;
		f.read_exact(buf).map_err(|e| {
			if e.kind() == io::ErrorKind::UnexpectedEof {
				SourceError::Eof
			} else {
				SourceError::Io
			}
		})
	}
}

/// Writes each volume of `packed` beside `first`, named by [`volume_path`].
pub fn write_volumes(first: &Path, packed: &Packed) -> Result<Vec<PathBuf>, HostError> {
	let mut out = Vec::new();
	for (i, v) in packed.volumes.iter().enumerate() {
		let path = volume_path(first, (i + 1) as u16).ok_or_else(|| {
			HostError::Io("more than 99 volumes cannot be named; use a larger volume size".into())
		})?;
		fs::write(&path, v).map_err(|e| io_err(&path, e))?;
		out.push(path);
	}
	Ok(out)
}

const SHORT_OK: &str = "_-!#$%&'()@^{}~";

fn clean(s: &str) -> String {
	s.chars()
		.filter(|&c| c != ' ' && c != '.')
		.map(|c| {
			let u = c.to_ascii_uppercase();
			if u.is_ascii_alphanumeric() || SHORT_OK.contains(u) {
				u
			} else {
				'_'
			}
		})
		.collect()
}

/// The 8.3 name for `name`, unique among `used`: the name upper-cased when it already fits, else
/// the first six characters, `~N` and the first three of the extension.
pub fn short_component(name: &str, used: &BTreeSet<String>) -> String {
	let (stem, ext) = match name.rfind('.') {
		Some(i) if i > 0 => (&name[..i], &name[i + 1..]),
		_ => (name, ""),
	};
	let (s, e) = (clean(stem), clean(ext));
	let lossless = s.len() <= 8
		&& e.len() <= 3
		&& !s.is_empty()
		&& stem.to_ascii_uppercase() == s
		&& ext.to_ascii_uppercase() == e;
	let join = |s: &str, e: &str| {
		if e.is_empty() {
			s.to_string()
		} else {
			format!("{s}.{e}")
		}
	};
	if lossless && !used.contains(&join(&s, &e)) {
		return join(&s, &e);
	}
	let e: String = e.chars().take(3).collect();
	let s = if s.is_empty() { "_".to_string() } else { s };
	for n in 1.. {
		let tail = format!("~{n}");
		let keep: String = s.chars().take(8 - tail.len()).collect();
		let c = join(&format!("{keep}{tail}"), &e);
		if !used.contains(&c) {
			return c;
		}
	}
	unreachable!()
}

/// How [`collect_dir`] describes the files it finds.
#[derive(Debug, Clone, Default)]
pub struct CollectOptions {
	/// Use the DOS epoch for every time, so the same tree always packs to the same bytes.
	pub fixed_time: bool,
	/// The target bits to put on every file; 0 for all.
	pub targets: u8,
}

fn is_dos_program(name: &str) -> bool {
	let up = name.to_ascii_uppercase();
	[".EXE", ".COM", ".BAT", ".DLL"]
		.iter()
		.any(|e| up.ends_with(e))
}

fn walk(
	dir: &Path,
	rel_short: &str,
	rel_long: &str,
	o: &CollectOptions,
	out: &mut Vec<FileSpec>,
) -> Result<(), HostError> {
	let mut entries: Vec<_> = fs::read_dir(dir)
		.map_err(|e| io_err(dir, e))?
		.collect::<Result<_, _>>()
		.map_err(|e| io_err(dir, e))?;
	entries.sort_by_key(|e| e.file_name());
	let mut names = Vec::new();
	for e in &entries {
		let n = e
			.file_name()
			.into_string()
			.map_err(|n| HostError::Name(format!("{} is not UTF-8: {n:?}", e.path().display())))?;
		names.push(n);
	}
	// Names that already fit keep their place; the rest are mangled around them.
	let mut used: BTreeSet<String> = BTreeSet::new();
	let mut shorts: Vec<Option<String>> = std::vec![None; names.len()];
	let fits = |n: &str| short_component(n, &BTreeSet::new()).eq_ignore_ascii_case(n);
	for (i, n) in names.iter().enumerate() {
		if fits(n) {
			let s = n.to_ascii_uppercase();
			used.insert(s.clone());
			shorts[i] = Some(s);
		}
	}
	for (i, n) in names.iter().enumerate() {
		if shorts[i].is_none() {
			let s = short_component(n, &used);
			used.insert(s.clone());
			shorts[i] = Some(s);
		}
	}
	let mut any = false;
	for ((e, n), s) in entries.iter().zip(&names).zip(shorts) {
		let s = s.unwrap_or_default();
		let short = if rel_short.is_empty() {
			s
		} else {
			format!("{rel_short}/{s}")
		};
		let long = if rel_long.is_empty() {
			n.clone()
		} else {
			format!("{rel_long}/{n}")
		};
		let ty = e.file_type().map_err(|x| io_err(&e.path(), x))?;
		any = true;
		if ty.is_dir() {
			walk(&e.path(), &short, &long, o, out)?;
		} else if ty.is_file() {
			let meta = e.metadata().map_err(|x| io_err(&e.path(), x))?;
			let data = fs::read(e.path()).map_err(|x| io_err(&e.path(), x))?;
			let mut f = FileSpec::new(short.clone(), data);
			f.long_name = if long == short {
				String::new()
			} else {
				long.clone()
			};
			f.targets = o.targets;
			if !o.fixed_time
				&& let Ok(t) = meta.modified()
			{
				let secs = t
					.duration_since(std::time::UNIX_EPOCH)
					.map_or(0, |d| d.as_secs() as i64);
				let (time, date) = dos_from_unix(secs);
				f.dos_time = time;
				f.dos_date = date;
			}
			if is_dos_program(n) || executable(&meta) {
				f.flags |= flags::EXECUTABLE;
			}
			if meta.permissions().readonly() {
				f.attr |= 0x01;
			}
			out.push(f);
		}
	}
	if !any && !rel_short.is_empty() {
		let mut d = FileSpec::directory(rel_short.to_string());
		d.long_name = if rel_long == rel_short {
			String::new()
		} else {
			rel_long.to_string()
		};
		d.targets = o.targets;
		out.push(d);
	}
	Ok(())
}

#[cfg(unix)]
fn executable(m: &fs::Metadata) -> bool {
	use std::os::unix::fs::PermissionsExt;
	m.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &fs::Metadata) -> bool {
	false
}

/// Reads a directory tree into file specs, in short-name order. Names that are not 8.3 get a
/// generated short name and keep their own as the long name.
pub fn collect_dir(root: &Path, o: &CollectOptions) -> Result<Vec<FileSpec>, HostError> {
	let mut out = Vec::new();
	walk(root, "", "", o, &mut out)?;
	out.sort_by(|a, b| a.short_name.cmp(&b.short_name));
	Ok(out)
}

/// Packs the tree under `root` and writes the volumes beside `first`.
pub fn pack_dir(
	root: &Path,
	first: &Path,
	o: &CollectOptions,
	opts: &Options,
) -> Result<(Packed, Vec<PathBuf>), HostError> {
	let files = collect_dir(root, o)?;
	let packed = pack(&files, opts)?;
	let paths = write_volumes(first, &packed)?;
	Ok((packed, paths))
}

/// One line of a listing.
#[derive(Debug, Clone)]
pub struct Listed {
	pub short_name: String,
	pub long_name: String,
	pub size: u32,
	pub crc: u32,
	pub flags: u16,
	pub dos_time: u16,
	pub dos_date: u16,
	pub volume: u16,
}

/// What a pack holds.
#[derive(Debug, Clone)]
pub struct Listing {
	pub volumes: u16,
	pub set_id: u32,
	pub total_size: u32,
	pub chunk_size: usize,
	pub entries: Vec<Listed>,
}

fn vec_of(n: usize) -> Vec<u8> {
	std::vec![0u8; n]
}

/// Lists the pack whose first volume is `first`.
pub fn list(first: &Path) -> Result<Listing, HostError> {
	{
		let mut src = VolumeFiles::new(first);
		let mut r = Reader::open(&mut src)?;
		let h = *r.header();
		let mut dir = vec_of(r.directory_len());
		let d = r.read_directory(&mut src, &mut dir)?;
		let mut entries = Vec::new();
		for e in d.entries() {
			let e = e?;
			entries.push(Listed {
				short_name: String::from_utf8_lossy(e.short_name).into_owned(),
				long_name: String::from_utf8_lossy(e.long_name).into_owned(),
				size: e.size,
				crc: e.crc,
				flags: e.flags,
				dos_time: e.dos_time,
				dos_date: e.dos_date,
				volume: e.first_volume,
			});
		}
		Ok(Listing {
			volumes: h.volume_count,
			set_id: h.set_id,
			total_size: h.total_size,
			chunk_size: h.chunk_size(),
			entries,
		})
	}
}

/// What an unpack or a verify did.
#[derive(Debug, Clone, Copy, Default)]
pub struct Report {
	pub files: u32,
	pub dirs: u32,
	pub bytes: u64,
}

/// Unpacks every file below `out` (or only checks them when `out` is `None`). With
/// `short_names` the 8.3 names are used, otherwise the long name where there is one.
pub fn unpack(first: &Path, out: Option<&Path>, short_names: bool) -> Result<Report, HostError> {
	let mut src = VolumeFiles::new(first);
	let mut r = Reader::open(&mut src)?;
	let header = *r.header();
	let mut dir_bytes = vec_of(r.directory_len());
	let dir = r.read_directory(&mut src, &mut dir_bytes)?;
	let mut window = vec_of(1 << crate::MAX_CHUNK_LOG2);
	let mut report = Report::default();
	for e in dir.entries() {
		let e = e?;
		let name = if short_names || e.long_name.is_empty() {
			e.short_name
		} else {
			e.long_name
		};
		let name = std::str::from_utf8(name).map_err(|_| Error::BadName)?;
		let target = out.map(|o| o.join(name));
		if e.is_dir() {
			if let Some(t) = &target {
				fs::create_dir_all(t).map_err(|x| io_err(t, x))?;
			}
			report.dirs += 1;
			continue;
		}
		let mut file = match &target {
			Some(t) => {
				if let Some(p) = t.parent() {
					fs::create_dir_all(p).map_err(|x| io_err(p, x))?;
				}
				Some(io::BufWriter::new(
					File::create(t).map_err(|x| io_err(t, x))?,
				))
			}
			None => None,
		};
		let mut failed: Option<io::Error> = None;
		let result = r.extract(&mut src, &e, &mut window, &mut |d: &[u8]| {
			if let Some(f) = file.as_mut()
				&& let Err(x) = f.write_all(d)
			{
				failed = Some(x);
				return Err(SinkError);
			}
			Ok(())
		});
		if let (Some(x), Some(t)) = (failed, &target) {
			return Err(io_err(t, x));
		}
		result?;
		if let (Some(mut f), Some(t)) = (file, &target) {
			f.flush().map_err(|x| io_err(t, x))?;
		}
		report.files += 1;
		report.bytes += u64::from(e.size);
	}
	if report.bytes != u64::from(header.total_size)
		|| report.files + report.dirs != header.file_count
	{
		return Err(Error::SizeMismatch.into());
	}
	Ok(report)
}

/// Checks every chunk, file and volume without writing anything.
pub fn verify(first: &Path) -> Result<Report, HostError> {
	unpack(first, None, true)
}
