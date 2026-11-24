// DOS outputs: a zip file and a FAT12 1.44 MB floppy image.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Both writers take a list of `(name, bytes)` and nothing else.
//!
//! - [`zip`] writes a stored (uncompressed) zip: `SETUP.LPK` is already compressed, and a stored
//!   zip opens everywhere, DOS unzippers included.
//! - [`floppy_1440`] and [`floppy_1440_labelled`] write a bare FAT12 image of a 1.44 MB disk:
//!   512-byte sectors, one sector to a cluster, two 9-sector FATs and 224 root entries. Names are
//!   8.3 and a `/` in a name puts the file in a subdirectory (`WIN95/SETUP.EXE`), made as needed
//!   with its `.` and `..` entries. Dates are fixed. A disk spanning several images is not
//!   supported, so a payload that does not fit is an error that says by how much.

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
	#[error("`{0}` is not a volume label; use at most 11 upper-case letters, digits or spaces")]
	BadLabel(String),
	#[error(
		"`{0}` uses a name that is already taken in its folder (8.3 names are not case sensitive)"
	)]
	Collision(String),
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

/// The serial number [`floppy_1440`] has always written, kept so existing images do not change.
pub const LEGACY_SERIAL: u32 = 0x4c49_5253;

/// The longest volume label FAT allows.
pub const MAX_LABEL: usize = 11;

/// Checks a volume label: at most 11 characters, upper case, and none of the characters FAT
/// forbids in one.
pub fn check_label(label: &str) -> Result<(), DosImageError> {
	let ok = label.len() <= MAX_LABEL
		&& label.bytes().all(|b| {
			b.is_ascii_uppercase() || b.is_ascii_digit() || b" _-!#$%&'()@^{}~".contains(&b)
		});
	if ok {
		Ok(())
	} else {
		Err(DosImageError::BadLabel(label.to_owned()))
	}
}

/// A directory being built: its children in the order they first appeared.
#[derive(Default)]
struct Dir {
	kids: Vec<(String, [u8; 11], Node)>,
}

enum Node {
	File(usize),
	Dir(Dir),
}

impl Dir {
	/// How many 512-byte clusters this directory (not its children) takes: `.`, `..` and the entries.
	fn clusters(&self) -> usize {
		((self.kids.len() + 2) * 32).div_ceil(SECTOR)
	}

	/// Every cluster this directory's subdirectories and the files below it take.
	fn tree_clusters(&self, files: &[(String, Vec<u8>)]) -> usize {
		self.kids
			.iter()
			.map(|(_, _, n)| match n {
				Node::File(i) => files[*i].1.len().div_ceil(SECTOR),
				Node::Dir(d) => d.clusters() + d.tree_clusters(files),
			})
			.sum()
	}
}

/// Files go in a tree of 8.3 names; a path is `/`-separated and may name subdirectories.
fn build_tree(files: &[(String, Vec<u8>)]) -> Result<Dir, DosImageError> {
	let mut root = Dir::default();
	for (i, (path, _)) in files.iter().enumerate() {
		let bad = || DosImageError::BadName(path.clone());
		let parts: Vec<&str> = path.split('/').collect();
		let (last, folders) = parts.split_last().ok_or_else(bad)?;
		let mut dir = &mut root;
		for part in folders {
			let short = short_name(part).map_err(|_| bad())?;
			let at = match dir.kids.iter().position(|(_, s, _)| *s == short) {
				Some(at) => at,
				None => {
					dir.kids
						.push((part.to_ascii_uppercase(), short, Node::Dir(Dir::default())));
					dir.kids.len() - 1
				}
			};
			dir = match &mut dir.kids[at].2 {
				Node::Dir(d) => d,
				Node::File(_) => return Err(DosImageError::Collision(path.clone())),
			};
		}
		let short = short_name(last).map_err(|_| bad())?;
		if dir.kids.iter().any(|(_, s, _)| *s == short) {
			return Err(DosImageError::Collision(path.clone()));
		}
		dir.kids
			.push((last.to_ascii_uppercase(), short, Node::File(i)));
	}
	Ok(root)
}

struct Writer<'a> {
	img: Vec<u8>,
	fat: Vec<u8>,
	next: usize,
	files: &'a [(String, Vec<u8>)],
}

impl Writer<'_> {
	/// Takes `n` clusters in a row and chains them in the FAT; returns the first.
	fn take(&mut self, n: usize) -> usize {
		let first = self.next;
		for c in 0..n {
			let v = if c + 1 == n {
				0xfff
			} else {
				(first + c + 1) as u16
			};
			set_fat(&mut self.fat, first + c, v);
		}
		self.next += n;
		first
	}

	fn entry(name: &[u8; 11], attr: u8, cluster: usize, size: usize) -> [u8; 32] {
		let mut e = [0u8; 32];
		e[..11].copy_from_slice(name);
		e[11] = attr;
		e[22..24].copy_from_slice(&DOS_TIME.to_le_bytes());
		e[24..26].copy_from_slice(&DOS_DATE.to_le_bytes());
		e[26..28].copy_from_slice(&(cluster as u16).to_le_bytes());
		e[28..32].copy_from_slice(&(size as u32).to_le_bytes());
		e
	}

	/// Lays out `dir`'s children, whose own first cluster is `own` (0 for the root), and returns the
	/// directory entries for them.
	fn emit(&mut self, dir: &Dir, own: usize) -> Vec<[u8; 32]> {
		let mut out = Vec::new();
		for (_, short, node) in &dir.kids {
			match node {
				Node::File(i) => {
					let data = &self.files[*i].1;
					let n = data.len().div_ceil(SECTOR);
					let mut first = 0;
					if n > 0 {
						first = self.take(n);
						let at = (DATA_START + first - 2) * SECTOR;
						self.img[at..at + data.len()].copy_from_slice(data);
					}
					out.push(Self::entry(short, 0x20, first, data.len()));
				}
				Node::Dir(d) => {
					let first = self.take(d.clusters());
					out.push(Self::entry(short, 0x10, first, 0));
					// `.` is the directory itself and `..` its parent (cluster 0 when that is the root).
					let mut all = vec![
						Self::entry(b".          ", 0x10, first, 0),
						Self::entry(b"..         ", 0x10, own, 0),
					];
					all.extend(self.emit(d, first));
					let at = (DATA_START + first - 2) * SECTOR;
					for (k, e) in all.iter().enumerate() {
						self.img[at + k * 32..at + (k + 1) * 32].copy_from_slice(e);
					}
				}
			}
		}
		out
	}
}

/// The clusters `files` take, subdirectories included, or why they cannot be laid out.
pub fn clusters_needed(files: &[(String, Vec<u8>)]) -> Result<usize, DosImageError> {
	let root = build_tree(files)?;
	Ok(root.tree_clusters(files))
}

/// A FAT12 image of a 1.44 MB floppy holding `files`.
///
/// `files` are `(path, bytes)` with `/` between folders (`WIN95/SETUP.EXE`); every name is 8.3 and
/// folders are made as needed. `label` is the volume label (see [`check_label`]) and `serial` the
/// volume serial number the boot sector records. Dates are fixed, so the same input always gives
/// the same image.
pub fn floppy_1440_labelled(
	label: &str,
	serial: u32,
	files: &[(String, Vec<u8>)],
) -> Result<Vec<u8>, DosImageError> {
	check_label(label)?;
	let tree = build_tree(files)?;
	if tree.kids.len() + 1 > ROOT_ENTRIES {
		return Err(DosImageError::TooManyFiles);
	}
	let clusters = tree.tree_clusters(files);
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
	b[39..43].copy_from_slice(&serial.to_le_bytes());
	let mut vol = [b' '; 11];
	vol[..label.len()].copy_from_slice(label.as_bytes());
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
	// The FAT and the directories.
	let mut fat = vec![0u8; FAT_SECTORS * SECTOR];
	set_fat(&mut fat, 0, 0xff0);
	set_fat(&mut fat, 1, 0xfff);
	let mut w = Writer {
		img,
		fat,
		next: 2,
		files,
	};
	let mut root = vec![0u8; ROOT_SECTORS * SECTOR];
	root[..11].copy_from_slice(&vol);
	root[11] = 0x08;
	root[22..24].copy_from_slice(&DOS_TIME.to_le_bytes());
	root[24..26].copy_from_slice(&DOS_DATE.to_le_bytes());
	for (i, e) in w.emit(&tree, 0).iter().enumerate() {
		root[(i + 1) * 32..(i + 2) * 32].copy_from_slice(e);
	}
	let Writer { mut img, fat, .. } = w;
	for copy in 0..2 {
		let at = (1 + copy * FAT_SECTORS) * SECTOR;
		img[at..at + fat.len()].copy_from_slice(&fat);
	}
	let at = (1 + 2 * FAT_SECTORS) * SECTOR;
	img[at..at + root.len()].copy_from_slice(&root);
	Ok(img)
}

/// A FAT12 image of a 1.44 MB floppy holding `files`, with the serial number this crate has always
/// written. A `label` longer than 11 characters is cut and its letters are made upper case, so
/// callers that derive one from a name keep working; [`floppy_1440_labelled`] is strict.
pub fn floppy_1440(label: &str, files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, DosImageError> {
	let label: String = label.to_ascii_uppercase().chars().take(MAX_LABEL).collect();
	floppy_1440_labelled(&label, LEGACY_SERIAL, files)
}

/// The bytes still free on an image written by this module, from its FAT.
pub fn free_bytes(img: &[u8]) -> usize {
	let entry = |c: usize| -> usize {
		let at = SECTOR + c * 3 / 2;
		let w = usize::from(img[at]) | usize::from(img[at + 1]) << 8;
		if c.is_multiple_of(2) {
			w & 0xfff
		} else {
			w >> 4
		}
	};
	(2..2 + CLUSTERS).filter(|&c| entry(c) == 0).count() * SECTOR
}

/// The volume label and serial number in an image's boot sector.
pub fn boot_label_and_serial(img: &[u8]) -> (String, u32) {
	let label = String::from_utf8_lossy(&img[43..54]).trim_end().to_owned();
	let serial = u32::from_le_bytes([img[39], img[40], img[41], img[42]]);
	(label, serial)
}

/// Reads the files back out of an image written by [`floppy_1440`], for tests and tools. Paths use
/// `/` between folders, in directory order, each folder's files after its entry in its parent.
pub fn read_floppy(img: &[u8]) -> Vec<(String, Vec<u8>)> {
	let entry = |c: usize| -> usize {
		let at = SECTOR + c * 3 / 2;
		let w = usize::from(img[at]) | usize::from(img[at + 1]) << 8;
		if c.is_multiple_of(2) {
			w & 0xfff
		} else {
			w >> 4
		}
	};
	// The bytes of a chain of clusters (a bad image cannot loop it for long).
	let chain = |first: usize, limit: usize| -> Vec<u8> {
		let mut data = Vec::new();
		let mut c = first;
		let mut hops = 0;
		while (2..0xff8).contains(&c) && hops <= CLUSTERS && data.len() < limit {
			let at = (DATA_START + c - 2) * SECTOR;
			if at + SECTOR > img.len() {
				break;
			}
			data.extend_from_slice(&img[at..at + SECTOR]);
			c = entry(c);
			hops += 1;
		}
		data
	};
	fn walk(
		raw: &[u8],
		prefix: &str,
		depth: usize,
		chain: &dyn Fn(usize, usize) -> Vec<u8>,
		out: &mut Vec<(String, Vec<u8>)>,
	) {
		for e in raw.as_chunks::<32>().0 {
			if e[0] == 0 {
				break;
			}
			if e[0] == 0xe5 || e[11] & 0x08 != 0 || e[0] == b'.' {
				continue;
			}
			let stem = String::from_utf8_lossy(&e[..8]).trim_end().to_owned();
			let ext = String::from_utf8_lossy(&e[8..11]).trim_end().to_owned();
			let name = if ext.is_empty() {
				stem
			} else {
				format!("{stem}.{ext}")
			};
			let path = format!("{prefix}{name}");
			let first = usize::from(u16::from_le_bytes([e[26], e[27]]));
			if e[11] & 0x10 != 0 {
				if depth < 16 {
					let sub = chain(first, usize::MAX);
					walk(&sub, &format!("{path}/"), depth + 1, chain, out);
				}
				continue;
			}
			let size = u32::from_le_bytes([e[28], e[29], e[30], e[31]]) as usize;
			let mut data = if size > 0 {
				chain(first, size)
			} else {
				Vec::new()
			};
			data.truncate(size);
			out.push((path, data));
		}
	}
	let root = (1 + 2 * FAT_SECTORS) * SECTOR;
	let mut out = Vec::new();
	walk(
		&img[root..root + ROOT_SECTORS * SECTOR],
		"",
		0,
		&chain,
		&mut out,
	);
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

	fn sha(img: &[u8]) -> String {
		use sha2::{Digest, Sha256};
		format!("{:x}", Sha256::digest(img))
	}

	/// Images from before subdirectories existed, pinned by hash: root-only input must not change.
	#[test]
	fn existing_images_are_byte_identical() {
		let one = vec![("A.B".to_owned(), vec![7u8; 1024])];
		for (label, files, want) in [
			(
				"FIZZ",
				files(),
				"cda573be92ae1cb347254887d432618fb06e69d28ace44c455c38d6484eec562",
			),
			(
				"x",
				one.clone(),
				"77abdef2e04a284427d7f2ce880f043698d981996215c90b474fe6e160dc2672",
			),
			(
				"a-very-long-label",
				one,
				"404e0c5ca66bae79ed9aa4ce3a11ce9949a3e434769e8c1c591e267a47d3be67",
			),
			(
				"E",
				vec![],
				"80d786d115774aeacb736fb3d0373dc15ae58c90fcf5055f4f036323a52e257b",
			),
		] {
			assert_eq!(sha(&floppy_1440(label, &files).unwrap()), want, "{label}");
		}
	}

	fn tree() -> Vec<(String, Vec<u8>)> {
		vec![
			("README.TXT".into(), b"top".to_vec()),
			("WIN95/SETUP.EXE".into(), vec![1u8; 3000]),
			("WIN95/SETUP.LPK".into(), vec![2u8; 700]),
			("win31/setup.exe".into(), vec![3u8; 10]),
			(
				"WIN95/DATA/DEEP.BIN".into(),
				(0..2049u32).map(|i| i as u8).collect(),
			),
			("WIN95/EMPTY.TXT".into(), vec![]),
			("COMMON.LPK".into(), vec![4u8; 513]),
		]
	}

	#[test]
	fn subdirectories_round_trip() {
		let img = floppy_1440_labelled("QUILL", 0x1234_abcd, &tree()).unwrap();
		assert_eq!(img.len(), 1_474_560);
		let mut got = read_floppy(&img);
		let mut want: Vec<_> = tree()
			.into_iter()
			.map(|(n, d)| (n.to_ascii_uppercase(), d))
			.collect();
		got.sort();
		want.sort();
		assert_eq!(got, want);
	}

	#[test]
	fn images_are_deterministic() {
		let a = floppy_1440_labelled("QUILL", 7, &tree()).unwrap();
		let b = floppy_1440_labelled("QUILL", 7, &tree()).unwrap();
		assert_eq!(a, b);
	}

	#[test]
	fn a_directory_has_dot_and_dotdot() {
		let img = floppy_1440_labelled("X", 1, &tree()).unwrap();
		let root = (1 + 2 * FAT_SECTORS) * SECTOR;
		// Root entries: label, README.TXT, then the first folder, WIN95.
		let e = &img[root + 2 * 32..root + 3 * 32];
		assert_eq!(&e[..11], b"WIN95      ");
		assert_eq!(e[11], 0x10);
		let first = usize::from(u16::from_le_bytes([e[26], e[27]]));
		let at = (DATA_START + first - 2) * SECTOR;
		let dot = &img[at..at + 32];
		let dotdot = &img[at + 32..at + 64];
		assert_eq!(&dot[..11], b".          ");
		assert_eq!(usize::from(u16::from_le_bytes([dot[26], dot[27]])), first);
		assert_eq!(&dotdot[..11], b"..         ");
		assert_eq!(u16::from_le_bytes([dotdot[26], dotdot[27]]), 0);
		// The nested folder's `..` is WIN95, not the root.
		let deep = (0..16)
			.map(|i| &img[at + i * 32..at + (i + 1) * 32])
			.find(|e| &e[..11] == b"DATA       ")
			.unwrap();
		let dfirst = usize::from(u16::from_le_bytes([deep[26], deep[27]]));
		let dat = (DATA_START + dfirst - 2) * SECTOR;
		assert_eq!(
			usize::from(u16::from_le_bytes([img[dat + 32 + 26], img[dat + 32 + 27]])),
			first
		);
	}

	#[test]
	fn a_folder_bigger_than_a_cluster_chains() {
		let mut files: Vec<(String, Vec<u8>)> = (0..40)
			.map(|i| (format!("D/F{i:03}.TXT"), vec![i as u8; 5]))
			.collect();
		files.push(("TOP.TXT".into(), vec![9]));
		let img = floppy_1440("X", &files).unwrap();
		let mut got = read_floppy(&img);
		got.sort();
		files.sort();
		assert_eq!(got, files);
	}

	#[test]
	fn name_collisions_are_refused() {
		let two = |a: &str, b: &str| vec![(a.to_owned(), vec![1]), (b.to_owned(), vec![2])];
		for (a, b) in [
			("A.TXT", "a.txt"),
			("A.TXT", "A.TXT"),
			("D/A.TXT", "d/a.txt"),
			// A file and a folder with one name.
			("D", "D/A.TXT"),
			("D/A.TXT", "D"),
		] {
			assert!(
				matches!(
					floppy_1440("X", &two(a, b)),
					Err(DosImageError::Collision(_))
				),
				"{a} {b}"
			);
		}
		// The same name in different folders is fine.
		assert!(floppy_1440("X", &two("A/X.TXT", "B/X.TXT")).is_ok());
	}

	#[test]
	fn bad_paths_are_refused() {
		for p in [
			"/A.TXT",
			"A//B.TXT",
			"A/",
			"TOOLONGNAME/A.TXT",
			"A/B.LONGEXT",
			"",
		] {
			assert!(
				matches!(
					floppy_1440("X", &[(p.to_owned(), vec![1])]),
					Err(DosImageError::BadName(_))
				),
				"{p:?}"
			);
		}
	}

	#[test]
	fn the_label_and_serial_are_in_the_boot_sector_and_the_root() {
		let img = floppy_1440_labelled("QUILL 01", 0xdead_beef, &tree()).unwrap();
		assert_eq!(
			boot_label_and_serial(&img),
			("QUILL 01".to_owned(), 0xdead_beef)
		);
		assert_eq!(&img[39..43], &0xdead_beefu32.to_le_bytes());
		let root = (1 + 2 * FAT_SECTORS) * SECTOR;
		assert_eq!(&img[root..root + 11], b"QUILL 01   ");
		assert_eq!(img[root + 11], 0x08);
		// The legacy entry point keeps its serial.
		let old = floppy_1440("fizz", &files()).unwrap();
		assert_eq!(
			boot_label_and_serial(&old),
			("FIZZ".to_owned(), LEGACY_SERIAL)
		);
	}

	#[test]
	fn labels_are_checked() {
		assert!(check_label("ABCDEFGHIJK").is_ok());
		assert!(check_label("").is_ok());
		for bad in ["ABCDEFGHIJKL", "lower", "A.B", "A*B", "A/B", "é"] {
			assert!(
				matches!(
					floppy_1440_labelled(bad, 0, &[]),
					Err(DosImageError::BadLabel(_))
				),
				"{bad}"
			);
		}
	}

	#[test]
	fn the_free_space_report_matches_the_files() {
		let empty = floppy_1440("X", &[]).unwrap();
		assert_eq!(free_bytes(&empty), CAPACITY);
		let files = tree();
		let used = clusters_needed(&files).unwrap();
		// Files: 6 + 1 + 1 + 5 + 1 (+0) clusters; folders: WIN95 (7 entries: . .. 4 files + DATA = 7 -> 1), WIN31 1, DATA 1.
		let data: usize = files.iter().map(|(_, d)| d.len().div_ceil(SECTOR)).sum();
		assert_eq!(used, data + 3);
		let img = floppy_1440("X", &files).unwrap();
		assert_eq!(free_bytes(&img), CAPACITY - used * SECTOR);
	}

	#[test]
	fn folders_count_against_the_capacity() {
		// A file that fills the disk fits at the root, but not once a folder needs its cluster.
		let full = vec![("D/BIG.BIN".to_owned(), vec![1u8; CAPACITY])];
		let e = floppy_1440("X", &full).unwrap_err();
		assert!(
			matches!(e, DosImageError::DoesNotFit { over: 512, .. }),
			"{e}"
		);
		let ok = vec![("D/BIG.BIN".to_owned(), vec![1u8; CAPACITY - SECTOR])];
		let img = floppy_1440("X", &ok).unwrap();
		assert_eq!(free_bytes(&img), 0);
		assert_eq!(
			read_floppy(&img),
			ok.iter()
				.map(|(n, d)| (n.clone(), d.clone()))
				.collect::<Vec<_>>()
		);
	}

	#[test]
	fn the_root_entry_limit_counts_folders() {
		let files: Vec<_> = (0..224)
			.map(|i| (format!("D{i:03}/F.TXT"), vec![1u8]))
			.collect();
		assert_eq!(floppy_1440("X", &files), Err(DosImageError::TooManyFiles));
		assert!(floppy_1440("X", &files[..223]).is_ok());
	}

	/// mtools and dosfstools read what we wrote, byte for byte; skipped when they are not installed.
	#[test]
	fn mtools_and_fsck_agree_with_the_tree() {
		let dir = tempfile::tempdir().unwrap();
		let img = dir.path().join("t.img");
		std::fs::write(
			&img,
			floppy_1440_labelled("QUILL", 0xcafe_f00d, &tree()).unwrap(),
		)
		.unwrap();
		let Some(o) = run(
			"mdir",
			&["-/".as_ref(), "-i".as_ref(), img.as_os_str(), "::".as_ref()],
		) else {
			eprintln!("skipping the mtools cross-check: mdir is not installed");
			return;
		};
		let listing = String::from_utf8_lossy(&o.stdout).to_string();
		assert!(o.status.success(), "{listing}");
		for want in [
			"WIN95", "WIN31", "DATA", "DEEP", "SETUP", "COMMON", "README",
		] {
			assert!(listing.contains(want), "{want} missing:\n{listing}");
		}
		assert!(listing.contains("QUILL"), "label:\n{listing}");
		assert!(listing.contains("CAFE-F00D"), "serial:\n{listing}");
		let used = clusters_needed(&tree()).unwrap() * SECTOR;
		let free = format!("{}", CAPACITY - used);
		assert!(
			listing
				.replace([',', ' '], "")
				.contains(&format!("{free}bytesfree")),
			"free {free}:\n{listing}"
		);
		for (path, data) in tree() {
			let o = run(
				"mtype",
				&["-i".as_ref(), img.as_os_str(), format!("::{path}").as_ref()],
			)
			.unwrap();
			assert!(o.status.success(), "{path}");
			assert_eq!(o.stdout, data, "{path}");
		}
		if let Some(o) = run("fsck.fat", &["-n".as_ref(), img.as_os_str()]) {
			assert!(
				o.status.success(),
				"{}{}",
				String::from_utf8_lossy(&o.stdout),
				String::from_utf8_lossy(&o.stderr)
			);
		} else {
			eprintln!("skipping fsck.fat: not installed");
		}
	}
}
