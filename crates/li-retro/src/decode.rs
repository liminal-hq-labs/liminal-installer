// The compiled project's decoder, written against `postcard`'s wire format.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Project::decode` runs in `SETUP.EXE` on DOS and Windows 3.1. serde's generated deserializer
//! costs about 20 KB of code, which a Win16 program cannot spare (one code segment holds 64 KB), so
//! the runtime reads the bytes itself. The format is the one the compiler writes with
//! `postcard::to_allocvec` on the model's `Serialize` derive:
//!
//! | Type | Bytes |
//! |---|---|
//! | `u8`, `bool` | one byte |
//! | `u16`, `u32` | LEB128 (seven bits a byte, low first) |
//! | `String`, `Vec<T>` | a LEB128 length, then the bytes or the items |
//! | `Option<T>` | `0`, or `1` and the value |
//! | an enum | a LEB128 variant index (declaration order), then the fields |
//! | a struct or tuple | its fields in declaration order |
//!
//! A test in `li-retro-build` encodes the real example projects and checks this decoder against
//! serde's, so the two cannot drift apart unseen.

use alloc::string::String;
use alloc::vec::Vec;

use crate::Error;
use crate::model::*;

/// A reader that never fails midway: a short or damaged input sets `bad` and reads as zeros and
/// empty strings from then on, and the caller checks `bad` once at the end. That leaves the decoding
/// straight-line code, with none of the clean-up paths an early return from half a struct needs, which
/// is what keeps it small in a 16-bit program.
struct R<'a> {
	b: &'a [u8],
	at: usize,
	bad: bool,
	/// A variant index this runtime does not know, as a newer compiler writes. Kept apart from
	/// `bad` so the error can say the project is newer, not damaged.
	unknown: bool,
	/// The project's format version, read first, which says how many variants an enum has.
	format: u16,
}

impl R<'_> {
	fn u8(&mut self) -> u8 {
		match self.b.get(self.at) {
			Some(&v) => {
				self.at += 1;
				v
			}
			None => {
				self.bad = true;
				0
			}
		}
	}

	fn var(&mut self) -> u32 {
		let mut v = 0u32;
		for i in 0..5 {
			let b = self.u8();
			v |= u32::from(b & 0x7f) << (7 * i);
			if b & 0x80 == 0 {
				return v;
			}
		}
		self.bad = true;
		0
	}

	fn u16(&mut self) -> u16 {
		let v = self.var();
		if v > 0xffff {
			self.bad = true;
		}
		v as u16
	}

	fn bool(&mut self) -> bool {
		let v = self.u8();
		if v > 1 {
			self.bad = true;
		}
		v == 1
	}

	/// An enum's variant index, which must be below `n`.
	fn tag(&mut self, n: u32) -> u32 {
		let v = self.var();
		if v >= n {
			self.unknown = true;
			self.bad = true;
			return 0;
		}
		v
	}

	/// A variant index this format does not define. The value is a stand-in the caller returns; the
	/// decode fails once it is done.
	fn unknown<T>(&mut self, stand_in: T) -> T {
		self.unknown = true;
		self.bad = true;
		stand_in
	}

	/// A length that cannot exceed what is left to read, so a bad one cannot ask for gigabytes.
	fn len(&mut self) -> usize {
		let n = self.var() as usize;
		if n > self.b.len() - self.at {
			self.bad = true;
			return 0;
		}
		n
	}

	fn bytes(&mut self) -> Vec<u8> {
		let n = self.len();
		let v = self.b[self.at..self.at + n].to_vec();
		self.at += n;
		v
	}

	fn string(&mut self) -> String {
		match String::from_utf8(self.bytes()) {
			Ok(s) => s,
			Err(_) => {
				self.bad = true;
				String::new()
			}
		}
	}

	fn opt(&mut self) -> Option<u16> {
		if self.bool() { Some(self.u16()) } else { None }
	}

	fn seq<T>(&mut self, one: fn(&mut Self) -> T) -> Vec<T> {
		let n = self.len();
		let mut v = Vec::with_capacity(n);
		for _ in 0..n {
			v.push(one(self));
		}
		v
	}

	fn rect(&mut self) -> Rect {
		Rect {
			x: self.u8(),
			y: self.u8(),
			w: self.u8(),
			h: self.u8(),
		}
	}

	fn item(&mut self) -> Item {
		Item {
			id: self.string(),
			label: self.string(),
			checked: self.bool(),
			required: self.bool(),
		}
	}

	fn component(&mut self) -> Component {
		Component {
			id: self.string(),
			name: self.string(),
			required: self.bool(),
			// Format 4 added the starting state; older projects tick everything.
			default: self.format < 4 || self.bool(),
		}
	}

	fn file(&mut self) -> FileEntry {
		FileEntry {
			component: self.u16(),
			dest: self.string(),
			stored: self.string(),
			size: self.var(),
			crc: self.var(),
		}
	}

	fn branch(&mut self) -> (Vec<u8>, u16) {
		(self.bytes(), self.u16())
	}

	fn widget(&mut self) -> Widget {
		let tag = self.tag(7);
		let rect = self.rect();
		match tag {
			0 => Widget::Text {
				rect,
				limit: self.u8(),
				attr: self.u8(),
				align: match self.tag(3) {
					0 => Align::Left,
					1 => Align::Centre,
					2 => Align::Right,
					_ => self.unknown(Align::Left),
				},
				template: self.string(),
			},
			1 => Widget::Button {
				rect,
				attr: self.u8(),
				label: self.string(),
				action: self.string(),
				default: self.bool(),
				hotkey: self.u8(),
				enabled_when: self.bytes(),
			},
			2 => Widget::List {
				rect,
				attr: self.u8(),
				check: self.bool(),
				label: self.string(),
				bind: self.string(),
				items: self.seq(Self::item),
				default: self.string(),
			},
			3 => Widget::Field {
				rect,
				attr: self.u8(),
				label: self.string(),
				bind: self.string(),
				width: self.u8(),
				placeholder: self.string(),
				default: self.string(),
			},
			4 => Widget::Meter {
				rect,
				attr: self.u8(),
				label: self.string(),
				source: match self.tag(3) {
					0 => Source::File,
					1 => Source::Total,
					2 => Source::FreeSpace,
					_ => self.unknown(Source::File),
				},
				low_below: self.u8(),
			},
			5 => Widget::Billboard {
				rect,
				attr: self.u8(),
				slides: self.seq(Self::string),
				interval: self.u8(),
			},
			6 => Widget::Doc {
				rect,
				attr: self.u8(),
				text: self.string(),
			},
			_ => self.unknown(Widget::Doc {
				rect,
				attr: 0,
				text: String::new(),
			}),
		}
	}

	fn step(&mut self) -> Step {
		Step {
			id: self.string(),
			kind: match self.tag(5) {
				0 => Kind::Page,
				1 => Kind::Action,
				2 => Kind::Branch,
				3 => Kind::Commit,
				4 => Kind::Finish,
				_ => self.unknown(Kind::Finish),
			},
			screen: self.opt(),
			when: self.bytes(),
			builtin: match self.tag(3) {
				0 => Builtin::None,
				1 => Builtin::DetectSound,
				2 => Builtin::CheckDiskSpace,
				_ => self.unknown(Builtin::None),
			},
			next: self.opt(),
			branches: self.seq(Self::branch),
		}
	}

	fn flow(&mut self) -> Flow {
		Flow {
			id: self.string(),
			start: self.u16(),
			steps: self.seq(Self::step),
		}
	}

	fn screen(&mut self) -> Screen {
		Screen {
			id: self.string(),
			title: self.string(),
			runs: self.bytes(),
			widgets: self.seq(Self::widget),
		}
	}

	fn op(&mut self) -> Op {
		// Format 3 added the file type; under format 2 its tag is not an operation at all.
		let known = if self.format >= 3 { 4 } else { 3 };
		match self.tag(known) {
			0 => Op::Mkdir {
				path: self.string(),
			},
			1 => Op::WriteIni {
				path: self.string(),
				section: self.string(),
				key: self.string(),
				value: self.string(),
				when_set: self.string(),
			},
			2 => Op::Shortcut {
				name: self.string(),
				target: self.string(),
				batch: self.string(),
			},
			3 => Op::FileType {
				ext: self.string(),
				prog_id: self.string(),
				name: self.string(),
				target: self.string(),
			},
			_ => self.unknown(Op::Mkdir {
				path: String::new(),
			}),
		}
	}

	fn theme(&mut self) -> Theme {
		Theme {
			base: self.u8(),
			dialog: self.u8(),
			focus: self.u8(),
			title: self.u8(),
			shadow: self.u8(),
			warn: self.u8(),
			status: self.u8(),
			disabled: self.u8(),
			button: self.u8(),
			button_focus: self.u8(),
			hotkey: self.u8(),
			field: self.u8(),
		}
	}
}

/// Decodes the bytes of a compiled project. The caller validates the result.
pub(crate) fn project(bytes: &[u8]) -> Result<Project, Error> {
	let mut r = R {
		b: bytes,
		at: 0,
		bad: false,
		unknown: false,
		format: 0,
	};
	let format = r.u16();
	if format > FORMAT {
		// Written by a newer compiler: its layout may differ anywhere, so nothing is read.
		return Err(Error::Format(String::from(
			"the compiled project is newer than this setup program",
		)));
	}
	r.format = format;
	let p = Project {
		format,
		id: r.string(),
		name: r.string(),
		publisher: r.string(),
		version: r.string(),
		target: r.string(),
		default_dir: r.string(),
		theme: r.theme(),
		strings: r.seq(R::string),
		components: r.seq(R::component),
		flows: r.seq(R::flow),
		screens: r.seq(R::screen),
		files: r.seq(R::file),
		ops: r.seq(R::op),
		owners: if format >= 4 {
			r.seq(R::u16)
		} else {
			Vec::new()
		},
	};
	if r.unknown {
		return Err(Error::Format(String::from(
			"the compiled project uses something this setup program does not know; it is newer",
		)));
	}
	if r.bad || r.at != bytes.len() {
		return Err(Error::Format(String::from(
			"the compiled project is cut short or damaged",
		)));
	}
	Ok(p)
}
