// The Win16 page viewer: the compiled project's pages in real Windows 3.1 controls.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `UIDEMO` reads `SETUP.LPK` beside it and shows each page of the `install` flow in order with
//! the same `Page`, `Values` and widgets the flow VM gives the native front end. Next, Back and
//! Cancel move between pages; the copying page animates its meters. No files are copied, there is
//! no flow VM and no install engine: this is the drawing half of the runtime, which fits one code
//! segment on its own. `/LOG=path` records the pages shown.
#![no_std]
#![no_main]

extern crate alloc;

#[path = "lisetup/dosio.rs"]
mod dosio;
#[path = "lisetup/heap.rs"]
mod heap;
#[path = "lisetup/plat.rs"]
mod plat;
#[path = "lisetup/ui.rs"]
mod ui;

use alloc::string::String;
use alloc::vec::Vec;
use li_retro::model::Kind;
use li_retro::platform::NativeEvent;
use li_retro::ui::{Page, Values};
use w16::*;

#[unsafe(no_mangle)]
pub extern "C" fn w16_main(_hinst: u32, _hprev: u32, _show: u32) -> u32 {
	unsafe { SetErrorMode(1) };
	let tail = command_line();
	let log = tail
		.split_whitespace()
		.find_map(|w| w.strip_prefix("/LOG="))
		.map(String::from);
	let mut p = plat::Win16Platform::new(log);
	let code = view(&mut p);
	p.finish(code);
	code as u32
}

fn view(p: &mut plat::Win16Platform) -> i32 {
	use li_retro::Platform;
	// The pack is not needed once the project is decoded: its window is the biggest block the heap holds.
	let (project, _) = match li_retro::app::load_project(p) {
		Ok(x) => x,
		Err(_) => return 2,
	};
	let Some(flow) = project.flow("install") else {
		return 2;
	};
	// The steps that show a screen, in the order the flow reaches them.
	let mut order: Vec<usize> = Vec::new();
	let mut cur = Some(usize::from(flow.start));
	while let Some(i) = cur {
		if order.contains(&i) || order.len() > 64 {
			break;
		}
		order.push(i);
		cur = flow.steps[i].next.map(usize::from);
	}
	let steps: Vec<usize> = order
		.into_iter()
		.filter(|&i| flow.steps[i].screen.is_some())
		.collect();
	let mut v = Values::default();
	for (k, val) in [
		("product.name", project.name.as_str()),
		("product.version", project.version.as_str()),
		("product.publisher", project.publisher.as_str()),
		("target.id", "win16-x86"),
		("install.size", "12.3 MB"),
		("system.free_space", "48.6 MB"),
		("plan.summary", "5 files, 12.3 MB in 3 components"),
		("progress.file", "SOUND\\ADLIB.DRV"),
		("sound.detected", "no sound card"),
	] {
		v.set_var(k, val);
	}
	v.set_str("install.path", &project.default_dir);
	let mut at = 0usize;
	while at < steps.len() {
		let step = &flow.steps[steps[at]];
		let screen = usize::from(step.screen.unwrap_or(0));
		let now = p.millis();
		let page = Page::new(&project, screen, &mut v, now);
		p.page_shown(&step.id, &project.screens[screen].id);
		if step.kind == Kind::Commit {
			let start = p.millis();
			loop {
				let t = p.millis().wrapping_sub(start);
				let pct = (t / 25).min(100);
				v.file_pct = pct;
				v.total_pct = pct;
				v.free_pct = 100 - pct / 4;
				if !p.native_progress(&page, &project, &v) {
					p.native_leave();
					return 3;
				}
				if pct >= 100 {
					break;
				}
			}
			p.native_leave();
			at += 1;
			continue;
		}
		let ev = loop {
			match p.native_page(&page, &project, &mut v) {
				NativeEvent::Changed => continue,
				e => break e,
			}
		};
		p.native_leave();
		match ev {
			NativeEvent::Action(a) if a == "back" => at = at.saturating_sub(1),
			NativeEvent::Action(a) if a == "cancel" => return 3,
			NativeEvent::Action(_) => at += 1,
			_ => return 3,
		}
	}
	0
}

fn command_line() -> String {
	let far = unsafe { CMDLINE };
	let mut s = String::new();
	if far == 0 {
		return s;
	}
	for i in 0..120 {
		let b = far_byte(far, i);
		if b == 0 || b == b'\r' {
			break;
		}
		s.push(b as char);
	}
	s
}
