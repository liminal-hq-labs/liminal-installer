// Round trips: files of every size around the chunk edges, spanning, flags and names.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use li_lpk::writer::{FileSpec, Options, PackError, pack};
use li_lpk::{Error, Reader, SliceSource, SourceError, flags};
use proptest::prelude::*;

fn opts(log2: u8) -> Options {
	Options {
		chunk_log2: log2,
		..Options::default()
	}
}

#[test]
fn an_empty_pack_has_no_files() {
	let p = pack(&[], &Options::default()).unwrap();
	assert_eq!(p.volumes.len(), 1);
	let got = unpack_all(&[&p.volumes[0]]).unwrap();
	assert!(got.is_empty());
}

#[test]
fn sizes_at_the_chunk_edges_round_trip() {
	for log2 in [9u8, 10, 12, 15] {
		let c = 1usize << log2;
		let sizes = [
			0,
			1,
			2,
			c - 1,
			c,
			c + 1,
			2 * c - 1,
			2 * c,
			2 * c + 1,
			3 * c + 7,
		];
		let files: Vec<FileSpec> = sizes
			.iter()
			.enumerate()
			.map(|(i, &n)| FileSpec::new(format!("F{i}.BIN"), bytes(i as u32 + 1, n)))
			.collect();
		let got = round_trip(&files, &opts(log2));
		assert_eq!(got.len(), files.len());
		for (g, f) in got.iter().zip(&files) {
			assert_eq!(g.data, f.data, "log2 {log2} file {}", f.short_name);
		}
	}
}

#[test]
fn compressible_and_incompressible_chunks_mix() {
	let mut data = bytes(7, 40_000);
	data.extend(noise(9, 40_000));
	data.extend(bytes(8, 5_000));
	let f = FileSpec::new("MIX.DAT", data.clone());
	let p = pack(&[f], &Options::default()).unwrap();
	// The noise is stored, so the pack is smaller than the data plus overhead, but not by much.
	assert!(p.chunk_bytes < data.len() as u64);
	let got = unpack_all(&[&p.volumes[0]]).unwrap();
	assert_eq!(got[0].data, data);
}

#[test]
fn names_flags_and_directories_survive() {
	let mut a = FileSpec::new("SOUND/ADLIB.DRV", bytes(1, 3000));
	a.long_name = "sound/AdLib Driver.drv".into();
	a.flags = flags::EXECUTABLE | flags::REPLACE_IF_NEWER | flags::SHARED;
	let d = FileSpec::directory("EMPTY");
	let z = FileSpec::new("ZERO.TXT", Vec::new());
	let got = round_trip(&[a.clone(), d, z], &Options::default());
	assert_eq!(got[0].short, b"SOUND/ADLIB.DRV");
	assert_eq!(got[0].long, b"sound/AdLib Driver.drv");
	assert_eq!(got[0].flags, a.flags);
	assert_eq!(got[1].flags, flags::DIRECTORY);
	assert!(got[1].data.is_empty() && got[2].data.is_empty());
}

#[test]
fn the_writer_refuses_bad_names_and_options() {
	let one = |n: &str| pack(&[FileSpec::new(n, vec![1])], &Options::default());
	assert!(matches!(one("lower.txt"), Err(PackError::ShortName(_))));
	assert!(matches!(
		one("TOOLONGNAME.TXT"),
		Err(PackError::ShortName(_))
	));
	assert!(matches!(one("A/../B"), Err(PackError::ShortName(_))));
	assert!(matches!(one("/ABS"), Err(PackError::ShortName(_))));
	assert!(matches!(one("A\\B"), Err(PackError::ShortName(_))));
	let mut f = FileSpec::new("A.TXT", vec![1]);
	f.long_name = "../x".into();
	assert!(matches!(
		pack(&[f], &Options::default()),
		Err(PackError::LongName(_))
	));
	let dup = [
		FileSpec::new("A.TXT", vec![1]),
		FileSpec::new("A.TXT", vec![2]),
	];
	assert!(matches!(
		pack(&dup, &Options::default()),
		Err(PackError::Duplicate(_))
	));
	let bad = Options {
		chunk_log2: 16,
		..Options::default()
	};
	assert!(matches!(pack(&[], &bad), Err(PackError::ChunkSize(16))));
}

#[test]
fn a_set_spans_volumes_and_the_reader_asks_for_each_in_order() {
	let files: Vec<FileSpec> = (0..6)
		.map(|i| {
			FileSpec::new(
				format!("F{i}.DAT"),
				noise(i + 1, 20_000 + i as usize * 1000),
			)
		})
		.collect();
	let o = Options {
		volume_sizes: vec![50_000],
		..Options::default()
	};
	let p = pack(&files, &o).unwrap();
	assert!(p.volumes.len() >= 3, "{} volumes", p.volumes.len());
	assert!(p.volumes.iter().all(|v| v.len() <= 50_000));
	let views: Vec<&[u8]> = p.volumes.iter().map(Vec::as_slice).collect();
	let mut src = Logged {
		inner: SliceSource::new(&views),
		opened: vec![],
		refuse: None,
	};
	let got = unpack_source(&mut src).unwrap();
	for (g, f) in got.iter().zip(&files) {
		assert_eq!(g.data, f.data);
	}
	// Open volume 1 for the directory, then forward only.
	let mut forward = src.opened.clone();
	forward.dedup();
	let mut sorted = forward.clone();
	sorted.sort();
	assert_eq!(forward[0], 1);
	let after_dir: Vec<u16> = forward.iter().copied().skip_while(|&v| v == 1).collect();
	assert!(after_dir.windows(2).all(|w| w[0] < w[1]), "{forward:?}");
}

#[test]
fn a_file_may_cross_a_volume_boundary_with_a_marker() {
	let big = noise(3, 100_000);
	let files = [
		FileSpec::new("BIG.BIN", big.clone()),
		FileSpec::new("NEXT.TXT", b"after".to_vec()),
	];
	let o = Options {
		volume_sizes: vec![40_000, 40_000, 40_000],
		..Options::default()
	};
	let p = pack(&files, &o).unwrap();
	assert_eq!(p.volumes.len(), 3);
	let views: Vec<&[u8]> = p.volumes.iter().map(Vec::as_slice).collect();
	let got = unpack_all(&views).unwrap();
	assert_eq!(got[0].data, big);
	assert_eq!(got[1].data, b"after");
}

#[test]
fn a_first_volume_can_leave_room_for_a_stub() {
	let files = [FileSpec::new("A.BIN", noise(1, 30_000))];
	let o = Options {
		volume_sizes: vec![10_000, 1_000_000],
		..Options::default()
	};
	let p = pack(&files, &o).unwrap();
	assert!(p.volumes[0].len() < 10_000);
	let views: Vec<&[u8]> = p.volumes.iter().map(Vec::as_slice).collect();
	assert_eq!(unpack_all(&views).unwrap()[0].data, files[0].data);
}

#[test]
fn a_volume_too_small_is_an_error() {
	let files = [FileSpec::new("A.BIN", noise(1, 30_000))];
	let o = Options {
		volume_sizes: vec![5_000],
		..Options::default()
	};
	assert_eq!(pack(&files, &o).unwrap_err(), PackError::VolumeTooSmall);
}

#[test]
fn the_wrong_disk_and_a_declined_disk_are_typed_errors() {
	let files: Vec<FileSpec> = (0..4)
		.map(|i| FileSpec::new(format!("F{i}.DAT"), noise(i + 1, 30_000)))
		.collect();
	let o = Options {
		volume_sizes: vec![40_000],
		..Options::default()
	};
	let p = pack(&files, &o).unwrap();
	let other = pack(
		&[FileSpec::new("X.DAT", noise(99, 90_000))],
		&Options {
			volume_sizes: vec![40_000],
			..Options::default()
		},
	)
	.unwrap();
	// Volume 2 from another set.
	let swapped: Vec<&[u8]> = vec![
		&p.volumes[0],
		&other.volumes[1],
		&p.volumes[2],
		&p.volumes[3],
	];
	assert_eq!(unpack_all(&swapped).unwrap_err(), Error::WrongVolume);
	// Volume 2 and 3 swapped within the set.
	let crossed: Vec<&[u8]> = vec![&p.volumes[0], &p.volumes[2], &p.volumes[1], &p.volumes[3]];
	assert_eq!(unpack_all(&crossed).unwrap_err(), Error::WrongVolume);
	// The person declines the second disk.
	let views: Vec<&[u8]> = p.volumes.iter().map(Vec::as_slice).collect();
	let mut src = Logged {
		inner: SliceSource::new(&views),
		opened: vec![],
		refuse: Some(2),
	};
	assert_eq!(
		unpack_source(&mut src).unwrap_err(),
		Error::Source(SourceError::Cancelled)
	);
	// A missing disk.
	let two: Vec<&[u8]> = vec![&p.volumes[0]];
	assert_eq!(
		unpack_all(&two).unwrap_err(),
		Error::Source(SourceError::NoVolume)
	);
}

#[test]
fn a_window_that_is_too_small_is_refused() {
	let p = pack(
		&[FileSpec::new("A.TXT", bytes(1, 5000))],
		&Options::default(),
	)
	.unwrap();
	let vols = [p.volumes[0].as_slice()];
	let mut src = SliceSource::new(&vols);
	let mut r = Reader::open(&mut src).unwrap();
	let mut d = vec![0; r.directory_len()];
	let dir = r.read_directory(&mut src, &mut d).unwrap();
	let e = dir.entries().next().unwrap().unwrap();
	let mut small = [0u8; 100];
	let err = r
		.extract(&mut src, &e, &mut small, &mut |_: &[u8]| Ok(()))
		.unwrap_err();
	assert_eq!(err, Error::BufferTooSmall);
	let mut tiny = [0u8; 4];
	assert_eq!(
		r.read_directory(&mut src, &mut tiny).err(),
		Some(Error::BufferTooSmall)
	);
}

proptest! {
	#![proptest_config(ProptestConfig { cases: 24, failure_persistence: None, ..ProptestConfig::default() })]

	#[test]
	fn random_packs_round_trip(
		sizes in proptest::collection::vec(prop_oneof![
			Just(0usize), Just(1), 2usize..2000, 4000usize..70_000],
			0..6),
		log2 in 9u8..=15,
		vol in prop_oneof![Just(u32::MAX), 70_000u32..200_000],
		seed in any::<u32>(),
		noisy in any::<bool>(),
	) {
		let files: Vec<FileSpec> = sizes.iter().enumerate().map(|(i, &n)| {
			let data = if noisy && i % 2 == 0 { noise(seed.wrapping_add(i as u32), n) } else { bytes(seed.wrapping_add(i as u32), n) };
			FileSpec::new(format!("F{i}.BIN"), data)
		}).collect();
		let o = Options { chunk_log2: log2, volume_sizes: vec![vol], ..Options::default() };
		let packed = match pack(&files, &o) {
			Ok(p) => p,
			Err(PackError::VolumeTooSmall) => return Ok(()),
			Err(e) => panic!("{e}"),
		};
		let views: Vec<&[u8]> = packed.volumes.iter().map(Vec::as_slice).collect();
		let got = unpack_all(&views).unwrap();
		prop_assert_eq!(got.len(), files.len());
		for (g, f) in got.iter().zip(&files) {
			prop_assert_eq!(&g.data, &f.data);
		}
	}
}
