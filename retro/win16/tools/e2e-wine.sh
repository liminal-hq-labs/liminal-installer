#!/usr/bin/env bash
# Installs and uninstalls Lt. Fizz and Planboard with ENGINE.EXE under Wine, and checks the files and the receipt.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: retro/win16/tools/e2e-wine.sh
#
# Builds the Win16 programs and the two packs (`liminst build -t win16-x86`), then for each example runs `ENGINE I C:\X` in a throwaway
# Wine prefix, checks that every file the receipt lists is there with its size, and runs `ENGINE U C:\X` and checks the folder is gone. Wine
# runs Win16 programs (its krnl386 loads NE executables), though it keeps register halves Windows itself would not, so this proves the
# logic and the file handling, not the register handling: that is Windows 3.11's run (e2e-w31.sh). Skips with a note when Wine is missing.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."
root="$PWD"

if ! command -v wine > /dev/null; then
	echo "e2e-wine: wine is not installed; skipping"
	exit 0
fi

work="$(mktemp -d)"
trap 'wineserver -k 2> /dev/null || true; rm -rf "$work"' EXIT
export WINEPREFIX="$work/prefix" WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml="
if [ -z "${DISPLAY:-}" ] && command -v xvfb-run > /dev/null && [ -z "${E2E_WINE_XVFB:-}" ]; then
	E2E_WINE_XVFB=1 exec xvfb-run -a "$0" "$@"
fi

OUT_DIR="$work/exe" bash retro/win16/build.sh > /dev/null
cargo build -q --locked -p li-cli
liminst="${CARGO_TARGET_DIR:-$root/target}/debug/liminst"
wine wineboot -i > /dev/null 2>&1 || true

fail=0
for ex in lt-fizz planboard; do
	dir="$work/$ex"
	mkdir -p "$dir/pack"
	"$liminst" build -C "examples/$ex" -t win16-x86 --version 1.0.0 -o "$dir/out" --output files > /dev/null 2>&1
	cp "$dir/out/win16/SETUP.LPK" "$work/exe/ENGINE.EXE" "$dir/pack/"
	# The engine writes upper-case folder names, and the prefix's disk is case-sensitive to a shell.
	name="E2E$(echo "$ex" | tr -d - | tr '[:lower:]' '[:upper:]')"
	target="C:\\$name"
	host="$WINEPREFIX/drive_c/$name"
	(cd "$dir/pack" && wine ENGINE.EXE I "$target" "/LOG=C:\\$ex.LOG" > /dev/null 2>&1) || true
	[ "$(tail -n1 "$WINEPREFIX/drive_c/$ex.LOG" | tr -d '\r')" = "exit 0" ] || { echo "e2e-wine: $ex install did not exit 0" >&2; fail=1; continue; }
	n=0
	while read -r kind name size _crc; do
		[ "$kind" = F ] || continue
		path="$host/$(echo "$name" | tr '\\' '/')"
		[ "$(stat -c %s "$path")" = "$size" ] || { echo "e2e-wine: $ex: $name is not $size bytes" >&2; fail=1; }
		n=$((n + 1))
	done < <(tr -d '\r' < "$host/INSTALL.LIR")
	echo "e2e-wine: $ex installed $n files, receipt checked"
	(cd "$dir/pack" && wine ENGINE.EXE U "$target" "/LOG=C:\\$ex-u.LOG" > /dev/null 2>&1) || true
	if [ -e "$host" ]; then
		echo "e2e-wine: $ex uninstall left $host behind" >&2
		fail=1
	else
		echo "e2e-wine: $ex uninstalled"
	fi
done
exit "$fail"
