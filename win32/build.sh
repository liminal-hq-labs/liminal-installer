#!/usr/bin/env bash
# Builds SETUP.EXE into win32/dist/ with no C compiler and no C library: Rust, rust-lld's lld-link and a Pentium target.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win32/build.sh        (set CARGO_TARGET_DIR to keep the build output outside the tree)
#
# `core` and `alloc` are built from source (-Zbuild-std) for a Pentium: the prebuilt ones assume SSE2, which a Windows 95 machine may
# lack. The options are unstable, so RUSTC_BOOTSTRAP=1 lets the pinned stable toolchain accept them. lld-link comes with the Rust
# toolchain's LLVM tools or the system's `lld`.
set -euo pipefail
cd "$(dirname "$0")"
export RUSTC_BOOTSTRAP=1
cargo build --release --locked
target="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p dist
cp "$target/win32/release/setup.exe" dist/SETUP.EXE
echo "build: dist/SETUP.EXE ($(wc -c < dist/SETUP.EXE) bytes)"
