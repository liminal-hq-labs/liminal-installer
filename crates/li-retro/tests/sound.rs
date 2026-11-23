// The sound probes against a pretend port bus.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_retro::sim::HostPlatform;
use li_retro::sound::{detect, probe_adlib, probe_dsp};
use li_retro::ui::Values;

fn platform() -> HostPlatform {
	HostPlatform::new(std::path::Path::new("."), Vec::new())
}

#[test]
fn an_answering_dsp_is_found_only_where_it_sits() {
	let mut p = platform();
	// An empty ISA bus reads as all ones: a data-available bit that is set but no 0xAA.
	assert!(!probe_dsp(&mut p, 0x220));
	p.ports.insert(0x22e, 0x80);
	p.ports.insert(0x22a, 0xaa);
	assert!(probe_dsp(&mut p, 0x220));
	assert!(!probe_dsp(&mut p, 0x240));
	assert!(p.port_writes.contains(&(0x226, 1)) && p.port_writes.contains(&(0x226, 0)));
}

#[test]
fn detection_prefers_blaster_then_a_dsp_then_adlib_then_nothing() {
	let mut v = Values::default();
	let mut p = platform();
	detect(&mut p, &mut v);
	assert_eq!(v.str("sound.card"), Some("none"));
	assert_eq!(v.vars["sound.detected"], "No sound card");

	let mut p = platform();
	p.ports.insert(0x25e, 0x80);
	p.ports.insert(0x25a, 0xaa);
	detect(&mut p, &mut v);
	assert_eq!(v.str("sound.card"), Some("sbpro"));
	assert_eq!(v.str("sound.port"), Some("250"));

	let mut p = platform();
	p.env.insert("BLASTER".into(), "A220 I5 D1".into());
	detect(&mut p, &mut v);
	assert_eq!(v.str("sound.irq"), Some("5"));
	assert_eq!(v.vars["sound.detected"], "Sound Blaster");
}

#[test]
fn an_adlib_needs_the_timer_flags_to_change() {
	let mut p = platform();
	assert!(!probe_adlib(&mut p));
}
