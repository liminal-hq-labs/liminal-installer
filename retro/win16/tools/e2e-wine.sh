#!/usr/bin/env bash
# Installs and uninstalls Lt. Fizz and Planboard with the real SETUP.EXE under Wine, and checks the files, the receipt and the pages.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: retro/win16/tools/e2e-wine.sh
#
# Builds SETUP.EXE and the two packs (`liminst build -t win16-x86`), then for each example runs `SETUP.EXE /PLAY=PLAY.TXT /DUMP=...`: the
# script presses the real controls (the licence choice, then every default button) and `/DUMP` writes the text of each page, which is
# compared with the host test's golden (crates/li-retro-build/tests/golden/win16). The receipt's sizes and the files are checked, then
# `SETUP.EXE /U` removes it again and the folder must be gone. Wine runs Win16 programs through its own krnl386, which keeps the
# register halves Windows itself would not, so this proves the flow and the file handling, not the register handling: that is the
# Windows 3.11 run (e2e-w31.sh). Skips with a note when Wine is missing.
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

# The host's pages and the controls' pages differ only in the free space and in what the machine's sound hardware is.
norm() { tr -d '\r' < "$1" | sed -e 's/^text: Space available: .*/text: Space available: N/' -e 's/Free: [0-9.]* [KMG]B/Free: N/' -e 's/^text: Setup found: .*/text: Setup found: N/' -e 's/^field: \(Port\|IRQ\|DMA\) = .*/field: \1 = N/' -e '/^radio: Sound card/,/^[^ ]/ s/^  (o)/  ( )/'; }

fail=0
for ex in lt-fizz planboard; do
	dir="$work/$ex"
	mkdir -p "$dir"
	"$liminst" build -C "examples/$ex" -t win16-x86 --version 1.0.0 -o "$dir/out" --output files --win16-runtime "$work/exe/SETUP.EXE" > /dev/null 2>&1
	cp "$dir/out/win16/SETUP.EXE" "$dir/out/win16/SETUP.LPK" "$dir/"
	printf 'licence choose licence.accepted yes\n' > "$dir/PLAY.TXT"
	: > "$dir/PLAY2.TXT"
	name="$([ "$ex" = lt-fizz ] && echo LTFIZZ || echo PLANBOAR)"
	host="$WINEPREFIX/drive_c/$name"
	(cd "$dir" && wine SETUP.EXE "/LOG=C:\\$ex.LOG" /PLAY=PLAY.TXT "/DUMP=C:\\$ex.TXT" > /dev/null 2>&1) || true
	if [ "$(tail -n1 "$WINEPREFIX/drive_c/$ex.LOG" 2> /dev/null | tr -d '\r')" != "exit 0" ]; then
		echo "e2e-wine: $ex install did not exit 0" >&2
		fail=1
		continue
	fi
	if diff <(norm "$WINEPREFIX/drive_c/$ex.TXT") <(norm "crates/li-retro-build/tests/golden/win16/$ex.pages.txt") > "$dir/diff"; then
		echo "e2e-wine: $ex: every page the controls show equals the host's golden"
	else
		cat "$dir/diff" >&2
		fail=1
	fi
	n=0
	while read -r kind fname size _crc; do
		[ "$kind" = F ] || continue
		path="$host/$(echo "$fname" | tr '\\' '/')"
		[ "$(stat -c %s "$path")" = "$size" ] || { echo "e2e-wine: $ex: $fname is not $size bytes" >&2; fail=1; }
		n=$((n + 1))
	done < <(tr -d '\r' < "$host/INSTALL.LIR")
	echo "e2e-wine: $ex installed $n files, receipt checked"
	(cd "$dir" && wine SETUP.EXE /U "C:\\$name" "/LOG=C:\\$ex-u.LOG" /PLAY=PLAY2.TXT > /dev/null 2>&1) || true
	if [ -e "$host" ]; then
		echo "e2e-wine: $ex uninstall left $host behind" >&2
		fail=1
	else
		echo "e2e-wine: $ex uninstalled"
	fi
done
exit "$fail"
