// The `detect-sound` built-in: BLASTER, then a probe of the usual Sound Blaster and AdLib ports.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! It does what Lt. Fizz's sound-card script does on the host runners, and a little more: with a
//! `BLASTER` variable such as `A220 I5 D1 T3` it takes the port, IRQ and DMA from it; without one
//! it resets a DSP at each of the ports 210h to 280h and reads the AdLib status register. The
//! results go to `sound.detected`, `sound.card`, `sound.port`, `sound.irq` and `sound.dma`.

use alloc::string::{String, ToString};

use crate::platform::Platform;
use crate::ui::Values;

/// The value that follows the letter `tag` in a `BLASTER` string, such as `220` after `A`.
pub fn blaster_field(blaster: &str, tag: char) -> Option<String> {
	blaster.split_whitespace().find_map(|w| {
		let mut cs = w.chars();
		(cs.next()?.eq_ignore_ascii_case(&tag))
			.then(|| cs.as_str().to_string())
			.filter(|s| !s.is_empty())
	})
}

fn pause<P: Platform>(p: &mut P) {
	// An ISA read takes about a microsecond whatever the CPU.
	for _ in 0..24 {
		p.port_in(0x80);
	}
}

/// Whether a Sound Blaster DSP answers at `base`: reset it and wait for the 0AAh ready byte.
pub fn probe_dsp<P: Platform>(p: &mut P, base: u16) -> bool {
	p.port_out(base + 6, 1);
	pause(p);
	p.port_out(base + 6, 0);
	for _ in 0..100 {
		pause(p);
		if p.port_in(base + 0xe) & 0x80 != 0 && p.port_in(base + 0xa) == 0xaa {
			return true;
		}
	}
	false
}

/// Whether an AdLib (OPL2) answers at 388h: the timer-overflow test.
pub fn probe_adlib<P: Platform>(p: &mut P) -> bool {
	let write = |p: &mut P, reg: u8, val: u8| {
		p.port_out(0x388, reg);
		for _ in 0..6 {
			p.port_in(0x388);
		}
		p.port_out(0x389, val);
		for _ in 0..35 {
			p.port_in(0x388);
		}
	};
	write(p, 4, 0x60);
	write(p, 4, 0x80);
	let before = p.port_in(0x388);
	write(p, 2, 0xff);
	write(p, 4, 0x21);
	for _ in 0..200 {
		pause(p);
	}
	let after = p.port_in(0x388);
	write(p, 4, 0x60);
	write(p, 4, 0x80);
	before & 0xe0 == 0 && after & 0xe0 == 0xc0
}

/// Fills the `sound.*` fields.
pub fn detect<P: Platform>(p: &mut P, v: &mut Values) {
	let set = |v: &mut Values, detected: &str, card: &str, port: &str, irq: &str, dma: &str| {
		v.set_str("sound.card", card);
		v.set_str("sound.port", port);
		v.set_str("sound.irq", irq);
		v.set_str("sound.dma", dma);
		v.set_var("sound.detected", detected);
	};
	if let Some(b) = p.getenv("BLASTER") {
		let port = blaster_field(&b, 'A').unwrap_or_else(|| "220".to_string());
		let irq = blaster_field(&b, 'I').unwrap_or_else(|| "5".to_string());
		let dma = blaster_field(&b, 'D').unwrap_or_else(|| "1".to_string());
		set(v, "Sound Blaster", "sbpro", &port, &irq, &dma);
		return;
	}
	for base in (0x210u16..=0x280).step_by(0x10) {
		if probe_dsp(p, base) {
			let port = alloc::format!("{base:X}");
			set(v, "Sound Blaster", "sbpro", &port, "5", "1");
			return;
		}
	}
	if probe_adlib(p) {
		set(v, "AdLib music card", "adlib", "388", "", "");
		return;
	}
	set(v, "No sound card", "none", "220", "5", "1");
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn reads_blaster_fields() {
		assert_eq!(blaster_field("A220 I5 D1 T3", 'A').as_deref(), Some("220"));
		assert_eq!(blaster_field("a240 i7 d3 h5", 'I').as_deref(), Some("7"));
		assert_eq!(blaster_field("A220", 'D'), None);
	}
}
