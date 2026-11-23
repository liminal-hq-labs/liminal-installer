#!/usr/bin/env bash
# Builds the Win16 programs: Rust to an ELF with rust-lld, then tools/ne_pack.py to an NE executable.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: build.sh [--regen] [--spike] [--all]
#
# Writes out/ENGINE.EXE and out/UIDEMO.EXE, the two halves of the runtime that each fit one 64 KB code segment
# (docs/retro-win16.md). `--spike` also builds the spike's MSGBOX, HELLO and SETUP. `--all` also tries LISETUP, the whole
# runtime, which does not fit yet: its size is reported and no executable is written. `--regen` rebuilds src/imports.rs from
# tools/imports.tbl first. Set CARGO_TARGET_DIR to keep the build output outside the tree, OUT_DIR to move out/.

cd "$(dirname "${BASH_SOURCE[0]}")"
export RUSTC_BOOTSTRAP=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
mkdir -p "$CARGO_TARGET_DIR"

regen=0 spike=0 whole=0
for a in "$@"; do
	case "$a" in
		--regen) regen=1 ;;
		--spike) spike=1 ;;
		--all) whole=1 ;;
		*) echo "build.sh: unknown option $a" >&2; exit 2 ;;
	esac
done

[ "$regen" = 1 ] && python3 tools/gen_imports.py tools/imports.tbl src/imports.rs

# name:stack:heap. The local heap is the one Windows' edit controls keep their text in.
progs=("engine:0x2000:0x1000" "uidemo:0x2000:0x3000")
[ "$spike" = 1 ] && progs+=("msgbox:0x2000:0x1000" "hello:0x2000:0x1000" "setup:0x2000:0x1000")

bins=()
for p in "${progs[@]}"; do bins+=(--bin "${p%%:*}"); done
if ! cargo build --release "${bins[@]}" 2>"$CARGO_TARGET_DIR/build.log"; then
	cat "$CARGO_TARGET_DIR/build.log" >&2
	exit 1
fi

out="${OUT_DIR:-out}"
mkdir -p "$out"
elf_dir="$CARGO_TARGET_DIR/win16/release"
for p in "${progs[@]}"; do
	IFS=: read -r prog stack heap <<<"$p"
	RES_ARGS=()
	[ -f "res/$prog.json" ] && RES_ARGS=(--resources "res/$prog.json")
	upper="$(echo "$prog" | tr '[:lower:]' '[:upper:]')"
	python3 tools/ne_pack.py --module "$upper" --description "Liminal Win16 $prog (Rust)" --stack "$stack" --heap "$heap" \
		${RES_ARGS[@]+"${RES_ARGS[@]}"} "$elf_dir/$prog" "$out/$upper.EXE"
done

if [ "$whole" = 1 ]; then
	# The whole runtime does not link: its code is over one segment. Link it anyway, keeping the output, to measure it.
	RUSTFLAGS="-C link-arg=-Tlink.ld -C link-arg=--no-dynamic-linker -C link-arg=--noinhibit-exec" \
		cargo build --release --bin lisetup 2>/dev/null || true
	python3 tools/size_report.py "$elf_dir/lisetup" "$elf_dir/engine" "$elf_dir/uidemo"
fi
