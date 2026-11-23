// SETUP.EXE, first light: prints a banner and exits.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

rt::entry!(main);

fn main() -> i32 {
	rt::println!("Liminal Installer for DOS");
	0
}
