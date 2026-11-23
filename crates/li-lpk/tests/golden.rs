// Golden files: packs committed to the repository. The reader must keep unpacking them forever;
// the writer must keep producing them byte for byte. `LI_UPDATE_GOLDEN=1` rewrites them, and the
// diff then shows exactly what changed in the format.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use std::path::PathBuf;

use common::*;
use li_lpk::writer::{FileSpec, Options, Packed, pack};
use li_lpk::{flags, targets};

fn golden(name: &str) -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("tests/golden")
		.join(name)
}

fn update() -> bool {
	std::env::var_os("LI_UPDATE_GOLDEN").is_some()
}

/// Four files: text that deflates, a program that is stored, a nested name with a long name, and
/// an empty directory.
fn basic_files() -> Vec<FileSpec> {
	let text = "Lieutenant Fizz needs a 386, 2 MB and a sound card.\r\n".repeat(120);
	let mut readme = FileSpec::new("README.TXT", text.into_bytes());
	readme.dos_date = 0x5b77;
	readme.dos_time = 0x6000;
	let mut run = FileSpec::new("FIZZ.EXE", noise(7, 1500));
	run.flags = flags::EXECUTABLE | flags::REPLACE_IF_NEWER;
	run.targets = targets::DOS;
	let mut drv = FileSpec::new("SOUND/ADLIB.DRV", bytes(3, 9000));
	drv.long_name = "sound/AdLib driver.drv".into();
	drv.flags = flags::SHARED;
	vec![readme, run, drv, FileSpec::directory("SAVES")]
}

fn basic_options() -> Options {
	Options {
		chunk_log2: 12,
		..Options::default()
	}
}

fn span_files() -> Vec<FileSpec> {
	vec![
		FileSpec::new("DISK1.DAT", noise(1, 5000)),
		FileSpec::new("MIDDLE.TXT", bytes(2, 9000)),
		FileSpec::new("TAIL.BIN", noise(3, 5000)),
	]
}

fn span_options() -> Options {
	Options {
		chunk_log2: 11,
		volume_sizes: vec![6000, 6500],
		..Options::default()
	}
}

fn check(name: &str, p: &Packed) {
	for (i, v) in p.volumes.iter().enumerate() {
		let file = if i == 0 {
			format!("{name}.lpk")
		} else {
			format!("{name}.l{:02}", i + 1)
		};
		if update() {
			std::fs::write(golden(&file), v).unwrap();
		}
		let want = std::fs::read(golden(&file))
			.unwrap_or_else(|_| panic!("{file} is missing; run with LI_UPDATE_GOLDEN=1"));
		assert!(
			*v == want,
			"{file}: the writer's output differs from the golden file; if the format changed on \
			 purpose, bump its version and run with LI_UPDATE_GOLDEN=1"
		);
	}
}

fn read_golden(name: &str, volumes: usize) -> Vec<Vec<u8>> {
	(0..volumes)
		.map(|i| {
			let file = if i == 0 {
				format!("{name}.lpk")
			} else {
				format!("{name}.l{:02}", i + 1)
			};
			std::fs::read(golden(&file)).unwrap()
		})
		.collect()
}

#[test]
fn basic_pack_is_reproduced_and_read() {
	let files = basic_files();
	let p = pack(&files, &basic_options()).unwrap();
	assert_eq!(p.volumes.len(), 1);
	check("basic", &p);
	let vols = read_golden("basic", 1);
	let got = unpack_all(&[&vols[0]]).unwrap();
	assert_eq!(got.len(), 4);
	for (g, f) in got.iter().zip(&files) {
		assert_eq!(g.short, f.short_name.as_bytes());
		assert_eq!(g.long, f.long_name.as_bytes());
		assert_eq!(g.flags, f.flags);
		assert_eq!(g.data, f.data);
	}
}

#[test]
fn spanned_pack_is_reproduced_and_read() {
	let files = span_files();
	let p = pack(&files, &span_options()).unwrap();
	assert_eq!(p.volumes.len(), 3);
	check("span", &p);
	let vols = read_golden("span", 3);
	let views: Vec<&[u8]> = vols.iter().map(Vec::as_slice).collect();
	let got = unpack_all(&views).unwrap();
	for (g, f) in got.iter().zip(&files) {
		assert_eq!(g.data, f.data);
	}
}

/// The worked example in `docs/lpk-format.md` is what the packer writes.
#[test]
fn the_specifications_example_is_what_the_packer_writes() {
	let doc = std::fs::read_to_string(
		PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/lpk-format.md"),
	)
	.unwrap();
	let mut from_doc = Vec::new();
	for line in doc.lines() {
		let Some((off, hex)) = line.split_once("  ") else {
			continue;
		};
		if off.len() == 4 && off.bytes().all(|b| b.is_ascii_hexdigit()) {
			let at = usize::from_str_radix(off, 16).unwrap();
			assert_eq!(at, from_doc.len(), "the dump skips an offset");
			for h in hex.split(' ') {
				from_doc.push(u8::from_str_radix(h, 16).unwrap());
			}
		}
	}
	let mut f = FileSpec::new("HI.TXT", b"Hello, LPK!\r\n".to_vec());
	f.long_name = "hi.txt".into();
	let p = pack(&[f], &Options::default()).unwrap();
	assert_eq!(from_doc, p.volumes[0]);
	assert_eq!(
		unpack_all(&[&from_doc]).unwrap()[0].data,
		b"Hello, LPK!\r\n"
	);
}

#[test]
fn a_raw_deflate_stream_from_zlib_decodes() {
	// The stream was made by Python's zlib at level 9, so the decoder is checked against an
	// encoder other than the packer's own.
	let text = format!(
		"{}{}{}",
		"The quick brown fox jumps over the lazy dog. ".repeat(40),
		"Pack my box with five dozen liquor jugs.\r\n".repeat(30),
		(0..500)
			.map(|i| (b'A' + ((i * 7) % 26) as u8) as char)
			.collect::<String>()
	);
	let stream = std::fs::read(golden("zlib-text.deflate")).unwrap();
	// Wrap the stream as one deflate chunk of a pack by hand.
	let v = wrap_chunk(&stream, text.as_bytes());
	let got = unpack_all(&[&v]).unwrap();
	assert_eq!(got[0].data, text.as_bytes());
}

/// Builds a one-chunk, one-file pack around a ready-made deflate stream.
fn wrap_chunk(stream: &[u8], raw: &[u8]) -> Vec<u8> {
	use li_lpk::{Crc, FOOTER_MAGIC, MAGIC, crc32};
	let mut chunk = vec![1u8, 0];
	chunk.extend((raw.len() as u16).to_le_bytes());
	chunk.extend((stream.len() as u16).to_le_bytes());
	chunk.extend([0, 0]);
	let mut c = Crc::default();
	c.update(&chunk);
	c.update(stream);
	chunk.extend(c.value().to_le_bytes());
	chunk.extend(stream);
	let name = b"T.TXT";
	let entry_len = 26 + name.len();
	let mut dir = Vec::new();
	dir.extend((entry_len as u16).to_le_bytes());
	dir.extend(0u16.to_le_bytes());
	dir.extend((raw.len() as u32).to_le_bytes());
	dir.extend(crc32(raw).to_le_bytes());
	dir.extend([0, 0, 0x21, 0, 0x20, 0]);
	dir.extend(1u16.to_le_bytes());
	dir.extend((48 + entry_len as u32).to_le_bytes());
	dir.extend([name.len() as u8, 0]);
	dir.extend(name);
	let volume_len = (48 + dir.len() + chunk.len() + 32) as u32;
	let mut h = Vec::new();
	h.extend(MAGIC);
	h.extend([1, 0, 48, 0, 0, 0, 1, 0, 1, 0, 15, 0]);
	h.extend(0xabcd_u32.to_le_bytes());
	h.extend(1u32.to_le_bytes());
	h.extend(48u32.to_le_bytes());
	h.extend((dir.len() as u32).to_le_bytes());
	h.extend(crc32(&dir).to_le_bytes());
	h.extend((raw.len() as u32).to_le_bytes());
	h.extend(volume_len.to_le_bytes());
	let hc = crc32(&h);
	h.extend(hc.to_le_bytes());
	let mut f = Vec::new();
	f.extend(FOOTER_MAGIC);
	f.extend(volume_len.to_le_bytes());
	f.extend(0xabcd_u32.to_le_bytes());
	f.extend([1, 0, 1, 0]);
	f.extend(hc.to_le_bytes());
	f.extend([0; 8]);
	let fc = crc32(&f);
	f.extend(fc.to_le_bytes());
	[h, dir, chunk, f].concat()
}
