#!/usr/bin/env bash
# Builds the Win16 programs: Rust to an ELF with rust-lld, then tools/ne_pack.py to an NE executable.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: build.sh [--regen] [--spike]
#
# Writes out/SETUP.EXE, the Windows 3.1 installer (code in several segments, see PROVENANCE.md and tools/ne_pack.py), and its
# out/SETUP.MAP (W16_FEATURES=panic-location names the file and line of a panic in its box). `--spike` also builds the spike's MSGBOX, HELLO and SETUP proof as out/SPIKE*.EXE names. `--regen` rebuilds
# src/imports.rs from tools/imports.tbl first. Set CARGO_TARGET_DIR to keep the build output outside the tree, OUT_DIR to move out/.

cd "$(dirname "${BASH_SOURCE[0]}")"
export RUSTC_BOOTSTRAP=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p "$CARGO_TARGET_DIR"

regen=0 spike=0
for a in "$@"; do
	case "$a" in
		--regen) regen=1 ;;
		--spike) spike=1 ;;
		*) echo "build.sh: unknown option $a" >&2; exit 2 ;;
	esac
done

[ "$regen" = 1 ] && python3 tools/gen_imports.py tools/imports.tbl src/imports.rs

# bin:OUTNAME:stack:heap. The local heap is the one Windows' edit controls keep their text in.
progs=("lisetup:SETUP:0x2000:0x2000")
[ "$spike" = 1 ] && progs+=("msgbox:SPIKEMSG:0x2000:0x1000" "hello:SPIKEHEL:0x2000:0x1000" "setup:SPIKESET:0x2000:0x1000")

bins=()
for p in "${progs[@]}"; do bins+=(--bin "${p%%:*}"); done
if ! cargo build --release ${W16_FEATURES:+--features "$W16_FEATURES"} "${bins[@]}" 2>"$CARGO_TARGET_DIR/build.log"; then
	cat "$CARGO_TARGET_DIR/build.log" >&2
	exit 1
fi

out="${OUT_DIR:-out}"
mkdir -p "$out"
elf_dir="$CARGO_TARGET_DIR/win16/release"
for p in "${progs[@]}"; do
	IFS=: read -r bin name stack heap <<<"$p"
	ARGS=()
	[ -f "res/$bin.json" ] && ARGS+=(--resources "res/$bin.json")
	[ -f "res/$bin.segments.json" ] && ARGS+=(--segments "res/$bin.segments.json")
	python3 tools/ne_pack.py --module "$name" --description "Liminal Win16 $bin (Rust)" --stack "$stack" --heap "$heap" \
		--shadow 64 --map "$out/$name.MAP" ${ARGS[@]+"${ARGS[@]}"} "$elf_dir/$bin" "$out/$name.EXE"
done
