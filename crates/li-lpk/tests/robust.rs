// Damaged and hostile input: truncation, bit flips, random bytes and structure-aware mutation.
// Every one must come back as a typed error (or, when nothing was damaged, the right bytes).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use li_lpk::writer::{FileSpec, Options, pack};
use li_lpk::{Error, HEADER_LEN, flags};
use proptest::prelude::*;

fn small_pack() -> Vec<u8> {
	let mut a = FileSpec::new(
		"README.TXT",
		b"hello hello hello hello hello hello hello\r\n".repeat(20),
	);
	a.long_name = "ReadMe.txt".into();
	let files = [
		a,
		FileSpec::new("BIN/RUN.EXE", noise(5, 700)),
		FileSpec::directory("EMPTY"),
		FileSpec::new("ZERO", Vec::new()),
	];
	let o = Options {
		chunk_log2: 9,
		..Options::default()
	};
	pack(&files, &o).unwrap().volumes.remove(0)
}

fn spanned() -> Vec<Vec<u8>> {
	let files = [
		FileSpec::new("A.BIN", noise(1, 3000)),
		FileSpec::new("B.TXT", bytes(2, 3000)),
		FileSpec::new("C.BIN", noise(3, 3000)),
	];
	let o = Options {
		chunk_log2: 9,
		volume_sizes: vec![2200],
		..Options::default()
	};
	pack(&files, &o).unwrap().volumes
}

#[test]
fn the_undamaged_pack_unpacks() {
	let v = small_pack();
	let got = unpack_all(&[&v]).unwrap();
	assert_eq!(got.len(), 4);
	assert_eq!(got[0].long, b"ReadMe.txt");
	assert_eq!(got[0].data.len(), 43 * 20);
}

#[test]
fn every_truncation_is_an_error() {
	let v = small_pack();
	for n in 0..v.len() {
		let r = unpack_all(&[&v[..n]]);
		assert!(
			r.is_err(),
			"a pack cut to {n} of {} bytes unpacked",
			v.len()
		);
	}
}

#[test]
fn every_truncation_of_a_set_is_an_error() {
	let vols = spanned();
	assert!(vols.len() >= 3);
	for i in 0..vols.len() {
		for n in [0, 1, 47, 48, 100, vols[i].len() - 33, vols[i].len() - 1] {
			let mut cut: Vec<&[u8]> = vols.iter().map(Vec::as_slice).collect();
			cut[i] = &vols[i][..n];
			assert!(unpack_all(&cut).is_err(), "volume {} cut to {n}", i + 1);
		}
	}
}

#[test]
fn every_single_bit_flip_is_an_error() {
	let v = small_pack();
	for i in 0..v.len() {
		for bit in 0..8 {
			let mut d = v.clone();
			d[i] ^= 1 << bit;
			assert!(
				unpack_all(&[&d]).is_err(),
				"flipping bit {bit} of byte {i} went unnoticed"
			);
		}
	}
}

#[test]
fn every_bit_flip_in_every_volume_of_a_set_is_an_error() {
	let vols = spanned();
	for vi in 0..vols.len() {
		for i in 0..vols[vi].len() {
			let mut d = vols.clone();
			d[vi][i] ^= 0x10;
			let views: Vec<&[u8]> = d.iter().map(Vec::as_slice).collect();
			assert!(unpack_all(&views).is_err(), "volume {} byte {i}", vi + 1);
		}
	}
}

#[test]
fn damage_is_named_by_its_kind() {
	let v = small_pack();
	let mut d = v.clone();
	d[0] = b'X';
	assert_eq!(unpack_all(&[&d]).unwrap_err(), Error::BadMagic);
	let mut d = v.clone();
	d[20] ^= 1;
	assert_eq!(unpack_all(&[&d]).unwrap_err(), Error::HeaderChecksum);
	let mut d = v.clone();
	let n = d.len();
	d[n - 20] ^= 1;
	assert_eq!(unpack_all(&[&d]).unwrap_err(), Error::BadFooter);
	let mut d = v.clone();
	d[HEADER_LEN + 4] ^= 1;
	assert_eq!(unpack_all(&[&d]).unwrap_err(), Error::DirectoryChecksum);
	// A bit in stored data is caught by the chunk checksum, never delivered.
	let mut d = v.clone();
	d[n - 40] ^= 1;
	assert!(matches!(
		unpack_all(&[&d]).unwrap_err(),
		Error::ChunkChecksum | Error::Inflate(_)
	));
}

#[test]
fn the_compatibility_rule() {
	let v = small_pack();
	// A later minor version of the same major is read.
	let mut d = v.clone();
	d[5] = 9;
	fix_checksums(&mut d);
	assert_eq!(unpack_all(&[&d]).unwrap().len(), 4);
	// A later major is refused, before anything else is looked at.
	let mut d = v.clone();
	d[4] = 2;
	assert_eq!(unpack_all(&[&d]).unwrap_err(), Error::UnsupportedVersion);
	// A must-understand header flag is refused; an advisory one is not.
	let mut d = v.clone();
	d[8] = 1;
	fix_checksums(&mut d);
	assert_eq!(unpack_all(&[&d]).unwrap_err(), Error::UnsupportedFeature);
	let mut d = v.clone();
	d[9] = 0x80;
	fix_checksums(&mut d);
	assert_eq!(unpack_all(&[&d]).unwrap().len(), 4);
	// A must-understand entry flag is refused; an unknown advisory one is read.
	let mut d = v.clone();
	d[HEADER_LEN + 3] |= 0x10;
	fix_checksums(&mut d);
	assert_eq!(unpack_all(&[&d]).unwrap_err(), Error::UnsupportedFeature);
	let mut d = v.clone();
	d[HEADER_LEN + 2] |= 0x80;
	fix_checksums(&mut d);
	assert_eq!(unpack_all(&[&d]).unwrap().len(), 4);
	// A header chunk size past 32 KiB would need a bigger window than any runtime has.
	let mut d = v.clone();
	d[14] = 16;
	fix_checksums(&mut d);
	assert_eq!(unpack_all(&[&d]).unwrap_err(), Error::BadHeader);
}

#[test]
fn unsafe_names_in_a_directory_are_refused() {
	for (bad, _) in [
		(&b"../EVIL.TXT"[..], 0),
		(b"/ABS.TXT", 0),
		(b"A\\B.TXT", 0),
		(b"C:/X.TXT", 0),
		(b"lower.txt", 0),
		(b"A//B.TXT", 0),
	] {
		// Rewrite the first entry's short name in place, keeping its length.
		let mut v = small_pack();
		let at = HEADER_LEN + 26;
		let len = v[HEADER_LEN + 24] as usize;
		let mut name = bad.to_vec();
		name.resize(len, b'X');
		v[at..at + len].copy_from_slice(&name);
		fix_checksums(&mut v);
		assert_eq!(
			unpack_all(&[&v]).unwrap_err(),
			Error::BadName,
			"{:?}",
			String::from_utf8_lossy(bad)
		);
	}
	let _ = flags::DIRECTORY;
}

proptest! {
	#![proptest_config(ProptestConfig { cases: 2000, failure_persistence: None, ..ProptestConfig::default() })]

	#[test]
	fn random_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..3000)) {
		let _ = unpack_all(&[&data]);
	}

	#[test]
	fn random_bytes_behind_a_good_magic_never_panic(tail in proptest::collection::vec(any::<u8>(), 0..400)) {
		let mut d = b"LPK\x1a\x01\x00\x30\x00".to_vec();
		d.extend(tail);
		let _ = unpack_all(&[&d]);
	}

	#[test]
	fn mutated_packs_never_panic_and_never_lie(
		edits in proptest::collection::vec((0usize..4000, any::<u8>()), 1..6),
		fix in any::<bool>(),
	) {
		let v = small_pack();
		let good = unpack_all(&[&v]).unwrap();
		let mut d = v.clone();
		for (i, b) in edits {
			let i = i % d.len();
			d[i] = b;
		}
		if fix {
			// Repair the checksums over the directory and header, so the checks behind them run.
			fix_checksums(&mut d);
		}
		if let Ok(got) = unpack_all(&[&d]) {
			// Only possible when the edits changed nothing that matters (an advisory bit, say).
			let same_data: Vec<_> = got.iter().map(|g| &g.data).collect();
			let was: Vec<_> = good.iter().map(|g| &g.data).collect();
			prop_assert_eq!(same_data, was);
		}
	}

	#[test]
	fn mutated_sets_never_panic(
		edits in proptest::collection::vec((0usize..3, 0usize..3000, any::<u8>()), 1..5),
	) {
		let mut vols = spanned();
		for (v, i, b) in edits {
			let v = v % vols.len();
			let i = i % vols[v].len();
			vols[v][i] = b;
		}
		let views: Vec<&[u8]> = vols.iter().map(Vec::as_slice).collect();
		let _ = unpack_all(&views);
	}
}
