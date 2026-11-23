// DOS outputs: a zip file and a FAT12 1.44 MB floppy image.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Both writers take a list of `(name, bytes)` and nothing else.
//!
//! - [`zip`] writes a stored (uncompressed) zip: `SETUP.LPK` is already compressed, and a stored
//!   zip opens everywhere, DOS unzippers included.
//! - [`floppy_1440`] writes a bare FAT12 image of a 1.44 MB disk: 512-byte sectors, one sector to a
//!   cluster, two 9-sector FATs and 224 root entries. Files go in the root directory under 8.3
//!   names. A disk spanning several images is not supported, so a payload that does not fit is an
//!   error that says by how much.

use flate2::Crc;

/// Why an image could not be written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DosImageError {
	#[error(
		"the files need {need} bytes ({clusters} clusters) and a 1.44 MB disk holds {have}; splitting an installer across several disks is not supported yet, so build the `zip` output or make the payload smaller by {over} bytes"
	)]
	DoesNotFit {
		need: usize,
		clusters: usize,
		have: usize,
		over: usize,
	},
	#[error("`{0}` is not an 8.3 name; a floppy root directory holds only those")]
	BadName(String),
	#[error("too many files for the 224 entries of a floppy root directory")]
	TooManyFiles,
}

const SECTOR: usize = 512;
const TOTAL_SECTORS: usize = 2880;
const FAT_SECTORS: usize = 9;
const ROOT_ENTRIES: usize = 224;
const ROOT_SECTORS: usize = ROOT_ENTRIES * 32 / SECTOR;
const DATA_START: usize = 1 + 2 * FAT_SECTORS + ROOT_SECTORS;
/// The clusters a 1.44 MB disk has for data.
pub const CLUSTERS: usize = TOTAL_SECTORS - DATA_START;
/// The bytes a 1.44 MB disk has for data.
pub const CAPACITY: usize = CLUSTERS * SECTOR;

/// 2026-01-01, 00:00 in DOS date and time, so images are the same every build.
const DOS_DATE: u16 = ((2026 - 1980) << 9) | (1 << 5) | 1;
const DOS_TIME: u16 = 0;

/// Splits `NAME.EXT` into the 11 bytes of a directory entry.
fn short_name(name: &str) -> Result<[u8; 11], DosImageError> {
	let bad = || DosImageError::BadName(name.to_owned());
	let up = name.to_ascii_uppercase();
	let (stem, ext) = up.split_once('.').unwrap_or((&up, ""));
	let ok = |s: &str, max: usize| {
		s.len() <= max
			&& s.bytes()
				.all(|b| b.is_ascii_alphanumeric() || b"_-!#$%&'()@^{}~".contains(&b))
	};
	if stem.is_empty() || !ok(stem, 8) || !ok(ext, 3) {
		return Err(bad());
	}
	let mut out = [b' '; 11];
	out[..stem.len()].copy_from_slice(stem.as_bytes());
	out[8..8 + ext.len()].copy_from_slice(ext.as_bytes());
	Ok(out)
}

fn set_fat(fat: &mut [u8], cluster: usize, value: u16) {
	let at = cluster * 3 / 2;
	if cluster.is_multiple_of(2) {
		fat[at] = value as u8;
		fat[at + 1] = (fat[at + 1] & 0xf0) | ((value >> 8) as u8 & 0x0f);
	} else {
		fat[at] = (fat[at] & 0x0f) | ((value << 4) as u8 & 0xf0);
		fat[at + 1] = (value >> 4) as u8;
	}
}

/// A FAT12 image of a 1.44 MB floppy holding `files` in its root directory.
pub fn floppy_1440(label: &str, files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, DosImageError> {
	if files.len() + 1 > ROOT_ENTRIES {
		return Err(DosImageError::TooManyFiles);
	}
	let names = files
		.iter()
		.map(|(n, _)| short_name(n))
		.collect::<Result<Vec<_>, _>>()?;
	let clusters: usize = files.iter().map(|(_, d)| d.len().div_ceil(SECTOR)).sum();
	if clusters > CLUSTERS {
		let need: usize = files.iter().map(|(_, d)| d.len()).sum();
		return Err(DosImageError::DoesNotFit {
			need,
			clusters,
			have: CAPACITY,
			over: (clusters - CLUSTERS) * SECTOR,
		});
	}
	let mut img = vec![0u8; TOTAL_SECTORS * SECTOR];
	// The boot sector: a BIOS parameter block, and a stub that tells the person this is not a boot disk.
	let b = &mut img[..SECTOR];
	b[..3].copy_from_slice(&[0xeb, 0x3c, 0x90]);
	b[3..11].copy_from_slice(b"LIMINAL ");
	b[11..13].copy_from_slice(&(SECTOR as u16).to_le_bytes());
	b[13] = 1;
	b[14..16].copy_from_slice(&1u16.to_le_bytes());
	b[16] = 2;
	b[17..19].copy_from_slice(&(ROOT_ENTRIES as u16).to_le_bytes());
	b[19..21].copy_from_slice(&(TOTAL_SECTORS as u16).to_le_bytes());
	b[21] = 0xf0;
	b[22..24].copy_from_slice(&(FAT_SECTORS as u16).to_le_bytes());
	b[24..26].copy_from_slice(&18u16.to_le_bytes());
	b[26..28].copy_from_slice(&2u16.to_le_bytes());
	b[38] = 0x29;
	b[39..43].copy_from_slice(&0x4c49_5253u32.to_le_bytes());
	let mut vol = [b' '; 11];
	for (i, c) in label.to_ascii_uppercase().bytes().take(11).enumerate() {
		vol[i] = c;
	}
	b[43..54].copy_from_slice(&vol);
	b[54..62].copy_from_slice(b"FAT12   ");
	// 0x3e: `mov si, msg; lodsb; or al, al; jz hang; mov ah, 0Eh; int 10h; jmp again; hang: jmp hang`
	let code: [u8; 20] = [
		0xbe, 0x57, 0x7c, 0xac, 0x08, 0xc0, 0x74, 0x06, 0xb4, 0x0e, 0xcd, 0x10, 0xeb, 0xf5, 0xeb,
		0xfe, 0x90, 0x90, 0x90, 0x90,
	];
	b[0x3e..0x3e + 20].copy_from_slice(&code);
	let msg = b"This disk is not bootable. Run SETUP from DOS.\r\n\0";
	b[0x57..0x57 + msg.len()].copy_from_slice(msg);
	b[510] = 0x55;
	b[511] = 0xaa;
	// The FAT and the directory.
	let mut fat = vec![0u8; FAT_SECTORS * SECTOR];
	set_fat(&mut fat, 0, 0xff0);
	set_fat(&mut fat, 1, 0xfff);
	let mut root = vec![0u8; ROOT_SECTORS * SECTOR];
	root[..11].copy_from_slice(&vol);
	root[11] = 0x08;
	root[22..24].copy_from_slice(&DOS_TIME.to_le_bytes());
	root[24..26].copy_from_slice(&DOS_DATE.to_le_bytes());
	let mut next = 2usize;
	for (i, ((_, data), name)) in files.iter().zip(&names).enumerate() {
		let e = &mut root[(i + 1) * 32..(i + 2) * 32];
		e[..11].copy_from_slice(name);
		e[11] = 0x20;
		e[22..24].copy_from_slice(&DOS_TIME.to_le_bytes());
		e[24..26].copy_from_slice(&DOS_DATE.to_le_bytes());
		let n = data.len().div_ceil(SECTOR);
		if n > 0 {
			e[26..28].copy_from_slice(&(next as u16).to_le_bytes());
			for c in 0..n {
				set_fat(
					&mut fat,
					next + c,
					if c + 1 == n {
						0xfff
					} else {
						(next + c + 1) as u16
					},
				);
			}
			let at = (DATA_START + next - 2) * SECTOR;
			img[at..at + data.len()].copy_from_slice(data);
			next += n;
		}
		e[28..32].copy_from_slice(&(data.len() as u32).to_le_bytes());
	}
	for copy in 0..2 {
		let at = (1 + copy * FAT_SECTORS) * SECTOR;
		img[at..at + fat.len()].copy_from_slice(&fat);
	}
	let at = (1 + 2 * FAT_SECTORS) * SECTOR;
	img[at..at + root.len()].copy_from_slice(&root);
	Ok(img)
}

/// Reads the files back out of an image written by [`floppy_1440`], for tests and tools.
pub fn read_floppy(img: &[u8]) -> Vec<(String, Vec<u8>)> {
	let fat_at = SECTOR;
	let entry = |c: usize| -> usize {
		let at = fat_at + c * 3 / 2;
		let w = usize::from(img[at]) | usize::from(img[at + 1]) << 8;
		if c.is_multiple_of(2) {
			w & 0xfff
		} else {
			w >> 4
		}
	};
	let root = (1 + 2 * FAT_SECTORS) * SECTOR;
	let mut out = Vec::new();
	for i in 0..ROOT_ENTRIES {
		let e = &img[root + i * 32..root + (i + 1) * 32];
		if e[0] == 0 {
			break;
		}
		if e[11] & 0x08 != 0 {
			continue;
		}
		let stem = String::from_utf8_lossy(&e[..8]).trim_end().to_owned();
		let ext = String::from_utf8_lossy(&e[8..11]).trim_end().to_owned();
		let name = if ext.is_empty() {
			stem
		} else {
			format!("{stem}.{ext}")
		};
		let size = u32::from_le_bytes([e[28], e[29], e[30], e[31]]) as usize;
		let mut c = usize::from(u16::from_le_bytes([e[26], e[27]]));
		let mut data = Vec::new();
		while size > 0 && (2..0xff8).contains(&c) {
			let at = (DATA_START + c - 2) * SECTOR;
			data.extend_from_slice(&img[at..at + SECTOR]);
			c = entry(c);
		}
		data.truncate(size);
		out.push((name, data));
	}
	out
}

/// A stored zip file of `files`. Names may hold `/` for folders.
pub fn zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
	let mut out = Vec::new();
	let mut central = Vec::new();
	for (name, data) in files {
		let mut crc = Crc::new();
		crc.update(data);
		let sum = crc.sum();
		let offset = out.len() as u32;
		let head = |v: &mut Vec<u8>, central: bool| {
			if central {
				v.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
				v.extend_from_slice(&20u16.to_le_bytes());
			} else {
				v.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
			}
			v.extend_from_slice(&20u16.to_le_bytes());
			v.extend_from_slice(&0u16.to_le_bytes());
			v.extend_from_slice(&0u16.to_le_bytes());
			v.extend_from_slice(&DOS_TIME.to_le_bytes());
			v.extend_from_slice(&DOS_DATE.to_le_bytes());
			v.extend_from_slice(&sum.to_le_bytes());
			v.extend_from_slice(&(data.len() as u32).to_le_bytes());
			v.extend_from_slice(&(data.len() as u32).to_le_bytes());
			v.extend_from_slice(&(name.len() as u16).to_le_bytes());
			v.extend_from_slice(&0u16.to_le_bytes());
		};
		head(&mut out, false);
		out.extend_from_slice(name.as_bytes());
		out.extend_from_slice(data);
		head(&mut central, true);
		central.extend_from_slice(&0u16.to_le_bytes());
		central.extend_from_slice(&0u16.to_le_bytes());
		central.extend_from_slice(&0u16.to_le_bytes());
		central.extend_from_slice(&0u32.to_le_bytes());
		central.extend_from_slice(&offset.to_le_bytes());
		central.extend_from_slice(name.as_bytes());
	}
	let cd_at = out.len() as u32;
	out.extend_from_slice(&central);
	out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
	out.extend_from_slice(&0u16.to_le_bytes());
	out.extend_from_slice(&0u16.to_le_bytes());
	out.extend_from_slice(&(files.len() as u16).to_le_bytes());
	out.extend_from_slice(&(files.len() as u16).to_le_bytes());
	out.extend_from_slice(&(central.len() as u32).to_le_bytes());
	out.extend_from_slice(&cd_at.to_le_bytes());
	out.extend_from_slice(&0u16.to_le_bytes());
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	fn files() -> Vec<(String, Vec<u8>)> {
		vec![
			(
				"SETUP.EXE".into(),
				(0..160_000u32).map(|i| (i * 7 % 251) as u8).collect(),
			),
			("setup.lpk".into(), vec![9u8; 513]),
			("README".into(), b"hello".to_vec()),
			("EMPTY.TXT".into(), vec![]),
		]
	}

	#[test]
	fn a_floppy_reads_back() {
		let img = floppy_1440("FIZZ", &files()).unwrap();
		assert_eq!(img.len(), 1_474_560);
		assert_eq!(&img[510..512], &[0x55, 0xaa]);
		let back = read_floppy(&img);
		let want: Vec<(String, Vec<u8>)> = files()
			.into_iter()
			.map(|(n, d)| (n.to_ascii_uppercase(), d))
			.collect();
		assert_eq!(back, want);
	}

	#[test]
	fn the_fat_has_the_media_byte_and_two_equal_copies() {
		let img = floppy_1440("X", &files()).unwrap();
		assert_eq!(&img[512..515], &[0xf0, 0xff, 0xff]);
		assert_eq!(img[512..512 + 9 * 512], img[512 + 9 * 512..512 + 18 * 512]);
	}

	#[test]
	fn a_full_disk_fits_and_one_byte_more_does_not() {
		let full = vec![("BIG.BIN".to_owned(), vec![1u8; CAPACITY])];
		assert!(floppy_1440("X", &full).is_ok());
		let over = vec![("BIG.BIN".to_owned(), vec![1u8; CAPACITY + 1])];
		let e = floppy_1440("X", &over).unwrap_err();
		assert!(
			matches!(e, DosImageError::DoesNotFit { over: 512, .. }),
			"{e}"
		);
		assert!(
			e.to_string()
				.contains("splitting an installer across several disks is not supported")
		);
	}

	#[test]
	fn long_names_are_refused() {
		assert!(matches!(
			floppy_1440("X", &[("LONGERNAME.EXE".into(), vec![1])]),
			Err(DosImageError::BadName(_))
		));
		assert!(matches!(
			floppy_1440("X", &[("A.TOOL".into(), vec![1])]),
			Err(DosImageError::BadName(_))
		));
	}

	#[test]
	fn a_zip_has_the_structure_readers_expect() {
		let z = zip(&files());
		assert_eq!(&z[..4], b"PK\x03\x04");
		let eocd = z.len() - 22;
		assert_eq!(&z[eocd..eocd + 4], b"PK\x05\x06");
		assert_eq!(u16::from_le_bytes([z[eocd + 10], z[eocd + 11]]), 4);
		let cd =
			u32::from_le_bytes([z[eocd + 16], z[eocd + 17], z[eocd + 18], z[eocd + 19]]) as usize;
		assert_eq!(&z[cd..cd + 4], b"PK\x01\x02");
	}

	/// Runs `tool args`, skipping the test (returning None) when the tool is not installed.
	fn run(tool: &str, args: &[&std::ffi::OsStr]) -> Option<std::process::Output> {
		std::process::Command::new(tool).args(args).output().ok()
	}

	#[test]
	fn dosfstools_and_unzip_accept_the_outputs() {
		let dir = tempfile::tempdir().unwrap();
		let img = dir.path().join("a.img");
		std::fs::write(&img, floppy_1440("FIZZ", &files()).unwrap()).unwrap();
		if let Some(o) = run("fsck.fat", &["-n".as_ref(), img.as_os_str()]) {
			assert!(
				o.status.success(),
				"{}{}",
				String::from_utf8_lossy(&o.stdout),
				String::from_utf8_lossy(&o.stderr)
			);
		}
		if let Some(o) = run("mdir", &["-i".as_ref(), img.as_os_str(), "::".as_ref()]) {
			let out = String::from_utf8_lossy(&o.stdout).to_string();
			assert!(out.contains("SETUP") && out.contains("README"), "{out}");
		}
		let zip_path = dir.path().join("a.zip");
		std::fs::write(&zip_path, zip(&files())).unwrap();
		if let Some(o) = run("unzip", &["-tq".as_ref(), zip_path.as_os_str()]) {
			assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
		}
	}
}
