// `liminst lpk`: pack a folder into a Liminal Pack, list it, unpack it and verify it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use li_lpk::host::{self, CollectOptions, HostError};
use li_lpk::targets;
use li_lpk::writer::Options;

use crate::{CliError, CliResult};

#[derive(Debug, Subcommand)]
pub enum LpkCommand {
	/// Pack a folder into an .LPK, or a set of volumes
	Pack(LpkPackArgs),
	/// List the files in a pack
	List(LpkFileArgs),
	/// Unpack every file into a folder
	Unpack(LpkUnpackArgs),
	/// Check every chunk, file and volume without writing anything
	Verify(LpkFileArgs),
}

#[derive(Debug, Args)]
pub struct LpkPackArgs {
	/// The folder to pack
	pub dir: PathBuf,
	/// The first volume to write, such as SETUP.LPK; later volumes are SETUP.L02, SETUP.L03 and so on
	#[arg(short, long, value_name = "FILE")]
	pub out: PathBuf,
	/// The most bytes in each volume: a number, a number with k or m (1024 and 1048576), or `floppy-1440`
	#[arg(long, value_name = "SIZE", value_parser = parse_size)]
	pub volume_size: Option<u32>,
	/// The most bytes in the first volume, to leave room for a self-extracting stub
	#[arg(long, value_name = "SIZE", value_parser = parse_size)]
	pub first_volume_size: Option<u32>,
	/// The chunk size as a power of two, 9 to 15 (15 is 32 KiB)
	#[arg(long, default_value_t = 15, value_name = "N")]
	pub chunk_log2: u8,
	/// The deflate effort, 1 to 10
	#[arg(long, default_value_t = 9, value_name = "N")]
	pub level: u8,
	/// Who the files are for: all, dos, win16, win32 or posix
	#[arg(long, default_value = "all")]
	pub target: String,
	/// Store the DOS epoch as every time, so the same folder always packs to the same bytes
	#[arg(long)]
	pub fixed_time: bool,
}

#[derive(Debug, Args)]
pub struct LpkFileArgs {
	/// The first volume of the pack
	pub pack: PathBuf,
}

#[derive(Debug, Args)]
pub struct LpkUnpackArgs {
	/// The first volume of the pack
	pub pack: PathBuf,
	/// The folder to unpack into
	#[arg(short, long, value_name = "DIR")]
	pub out: PathBuf,
	/// Name files by their 8.3 names instead of their long names
	#[arg(long)]
	pub short_names: bool,
}

fn parse_size(s: &str) -> Result<u32, String> {
	let t = s.trim().to_ascii_lowercase();
	if t == "floppy-1440" {
		return Ok(1_457_664);
	}
	let (num, mul) = match t.chars().last() {
		Some('k') => (&t[..t.len() - 1], 1024u64),
		Some('m') => (&t[..t.len() - 1], 1024 * 1024),
		_ => (&t[..], 1),
	};
	let n: u64 = num.parse().map_err(|_| format!("`{s}` is not a size"))?;
	u32::try_from(n * mul).map_err(|_| format!("`{s}` is too large"))
}

fn err(e: HostError) -> CliError {
	CliError::new(e.to_string())
}

fn target_bits(name: &str) -> CliResult<u8> {
	match name {
		"all" => Ok(0),
		"dos" => Ok(targets::DOS),
		"win16" => Ok(targets::WIN16),
		"win32" => Ok(targets::WIN32),
		"posix" => Ok(targets::POSIX),
		other => Err(CliError::new(format!(
			"`{other}` is not a target; use all, dos, win16, win32 or posix"
		))),
	}
}

pub fn pack(a: &LpkPackArgs) -> CliResult<()> {
	let sizes = match (a.first_volume_size, a.volume_size) {
		(Some(f), Some(v)) => vec![f, v],
		(Some(f), None) => vec![f, u32::MAX],
		(None, Some(v)) => vec![v],
		(None, None) => vec![u32::MAX],
	};
	let opts = Options {
		chunk_log2: a.chunk_log2,
		volume_sizes: sizes,
		level: a.level.clamp(1, 10),
		..Options::default()
	};
	let collect = CollectOptions {
		fixed_time: a.fixed_time,
		targets: target_bits(&a.target)?,
	};
	let (packed, paths) = host::pack_dir(&a.dir, &a.out, &collect, &opts).map_err(err)?;
	let stored: u64 = packed.volumes.iter().map(|v| v.len() as u64).sum();
	println!(
		"Packed {} bytes into {stored} bytes in {} volume{}:",
		packed.raw_bytes,
		paths.len(),
		if paths.len() == 1 { "" } else { "s" }
	);
	for (p, v) in paths.iter().zip(&packed.volumes) {
		println!("  {} ({} bytes)", p.display(), v.len());
	}
	Ok(())
}

fn flag_letters(f: u16) -> String {
	let mut s = String::new();
	s.push(if f & li_lpk::flags::EXECUTABLE != 0 {
		'x'
	} else {
		'-'
	});
	s.push(if f & li_lpk::flags::REPLACE_IF_NEWER != 0 {
		'n'
	} else {
		'-'
	});
	s.push(if f & li_lpk::flags::SHARED != 0 {
		's'
	} else {
		'-'
	});
	s.push(if f & li_lpk::flags::DIRECTORY != 0 {
		'd'
	} else {
		'-'
	});
	s
}

pub fn list(a: &LpkFileArgs) -> CliResult<()> {
	let l = host::list(&a.pack).map_err(err)?;
	println!(
		"{} file{}, {} bytes, {} volume{}, chunks of {} bytes, set {:08x}",
		l.entries.len(),
		if l.entries.len() == 1 { "" } else { "s" },
		l.total_size,
		l.volumes,
		if l.volumes == 1 { "" } else { "s" },
		l.chunk_size,
		l.set_id
	);
	for e in &l.entries {
		let (d, t) = (e.dos_date, e.dos_time);
		let name = if e.long_name.is_empty() {
			e.short_name.clone()
		} else {
			format!("{} ({})", e.short_name, e.long_name)
		};
		println!(
			"{:>10} {:08x} {} {:04}-{:02}-{:02} {:02}:{:02} v{} {name}",
			e.size,
			e.crc,
			flag_letters(e.flags),
			1980 + (d >> 9),
			(d >> 5) & 15,
			d & 31,
			t >> 11,
			(t >> 5) & 63,
			e.volume
		);
	}
	Ok(())
}

pub fn unpack(a: &LpkUnpackArgs) -> CliResult<()> {
	let r = host::unpack(&a.pack, Some(&a.out), a.short_names).map_err(err)?;
	println!(
		"Unpacked {} file{} ({} bytes) into {}",
		r.files,
		if r.files == 1 { "" } else { "s" },
		r.bytes,
		Path::new(&a.out).display()
	);
	Ok(())
}

pub fn verify(a: &LpkFileArgs) -> CliResult<()> {
	let r = host::verify(&a.pack).map_err(err)?;
	println!(
		"OK: {} file{} and {} director{}, {} bytes, every checksum matches",
		r.files,
		if r.files == 1 { "" } else { "s" },
		r.dirs,
		if r.dirs == 1 { "y" } else { "ies" },
		r.bytes
	);
	Ok(())
}

pub fn run(c: &LpkCommand) -> CliResult<()> {
	match c {
		LpkCommand::Pack(a) => pack(a),
		LpkCommand::List(a) => list(a),
		LpkCommand::Unpack(a) => unpack(a),
		LpkCommand::Verify(a) => verify(a),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sizes_parse() {
		assert_eq!(parse_size("1000").unwrap(), 1000);
		assert_eq!(parse_size("2k").unwrap(), 2048);
		assert_eq!(parse_size("1M").unwrap(), 1_048_576);
		assert_eq!(parse_size("floppy-1440").unwrap(), 1_457_664);
		assert!(parse_size("x").is_err());
		assert!(parse_size("9999999m").is_err());
	}
}
