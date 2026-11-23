#!/usr/bin/env bash
# Checks the Win32 workspace: format, lints, then builds SETUP.EXE and checks it: Pentium instructions only, ANSI imports on the allowlist, a Windows 95 PE.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win32/check.sh
#
# The format and lint checks need only the Rust toolchain (with rust-src). The build needs lld-link and the checks llvm-objdump (apt: lld, llvm);
# without lld-link the build and the checks are skipped with a note, unless WIN32_REQUIRE_TOOLS=1 makes that an error, as CI does.
set -euo pipefail
cd "$(dirname "$0")"
export RUSTC_BOOTSTRAP=1

cargo fmt --all -- --check
cargo clippy --release --locked -- -D warnings

if ! command -v lld-link > /dev/null; then
	echo "check: lld-link not found${WIN32_REQUIRE_TOOLS:+ (required)}"
	[ -z "${WIN32_REQUIRE_TOOLS:-}" ] || exit 1
	echo "check: skipping the build and the executable checks"
	exit 0
fi
./build.sh
tools/check-pe.sh
tools/scan-pentium.sh
tools/check-imports.sh
