// Starts the Studio window.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
	liminal_installer_studio_lib::run()
}
