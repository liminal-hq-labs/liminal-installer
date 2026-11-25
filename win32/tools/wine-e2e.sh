#!/usr/bin/env bash
# Installs and uninstalls Lt. Fizz and Planboard with the real SETUP.EXE under Wine (on Xvfb) and checks files, receipt, shortcut, registry and pages.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win32/tools/wine-e2e.sh [--shots DIR]
#
# `liminst build -t windows-x86` makes SETUP.EXE and SETUP.LPK; SETUP.EXE runs with /PLAY (a script that presses the real controls:
# BM_CLICK on buttons, text into the edit controls) and /DUMP (the page text read back from the controls), and the dump is compared with the
# host test's golden pages (crates/li-retro-build/tests/golden/win32). With --shots, /SHOT saves a bitmap of every page as a PNG in DIR.
# Needs wine, xvfb-run and python3; exits 0 with a note when they are missing. WINEPREFIX defaults to a scratch directory.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
SHOTS=""
[ "${1:-}" = "--shots" ] && SHOTS="$(mkdir -p "$2" && cd "$2" && pwd)"
command -v wine > /dev/null && command -v xvfb-run > /dev/null || { echo "wine-e2e: wine or xvfb-run not found; skipping"; exit 0; }
[ -n "${LI_WIN32_SKIP_BUILD:-}" ] || "$HERE/build.sh"
(cd "$ROOT" && cargo build -q -p li-cli)
LIMINST="${CARGO_TARGET_DIR:-$ROOT/target}/debug/liminst"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/li-win32.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
export WINEPREFIX="${WINEPREFIX:-$WORK/prefix}" WINEDEBUG=-all
unset WINEARCH
fail() { echo "wine-e2e: FAIL: $*" >&2; exit 1; }
# The host's pages and the controls' pages differ only in the machine's own numbers.
norm() { sed -e 's/Program Files (x86)/Program Files/g' -e 's/^text: Space available: .*/text: Space available: N/' -e 's/Free: [0-9.]* [KMG]B/Free: N/' -e '/^file:/d' -e '/^report:/d' -e '/^geometry:/d' -e 's/\r$//' "$1"; }
# A 32-bit program on 64-bit Windows (or Wine) writes below Wow6432Node, which /reg:32 reads.
reg() { wine reg query "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\$1" /reg:32 "${@:2}" 2>&1 | tr -d '\r' | grep REG_ || true; }
# The default value of a HKEY_CLASSES_ROOT key, read in both registry views (a 32-bit program's classes are redirected on 64-bit Windows).
cls() { { wine reg query "HKCR\\$1" /ve /reg:32 2>&1; wine reg query "HKCR\\$1" /ve /reg:64 2>&1; } | tr -d '\r' | grep REG_SZ | head -1 | sed -e 's/^ *[^ ]* *REG_SZ *//' || true; }
# The X server must outlive every process the program starts (the uninstaller hands over to a copy of itself), so wait for the wineserver.
win() { (cd "$1" && shift && timeout 240 xvfb-run -a -s "-screen 0 1024x768x24" bash -c 'wine "$@"; rc=$?; wineserver -w; exit $rc' wine "$@" > /dev/null 2>&1); }

run_project() {
	local project="$1" id="$2" name="$3" want="$4" ext="${5:-}" prog="${6:-}"
	local out="$WORK/$project" play="$WORK/$project.play"
	"$LIMINST" build -C "$ROOT/examples/$project" -t windows-x86 --version 1.0.0 -o "$out" --win32-runtime "$HERE/dist/SETUP.EXE" > /dev/null
	local d="$out/win32"
	printf 'licence choose licence.accepted yes\n' > "$play"
	local shotdir=""
	[ -z "$SHOTS" ] || { shotdir="$WORK/shots-$project"; mkdir -p "$shotdir"; }
	# A file type the product declares replaces an owner: seed one, to see it taken and given back.
	[ -z "$ext" ] || wine reg add "HKCR\\.$ext" /ve /d txtfile /f /reg:32 > /dev/null 2>&1
	win "$d" SETUP.EXE /PLAY "$play" /DUMP "$WORK/$project.dump" ${shotdir:+/SHOT "$shotdir"} || fail "$project: SETUP.EXE did not finish ok"
	diff <(norm "$WORK/$project.dump") <(norm "$ROOT/crates/li-retro-build/tests/golden/win32/$project.pages.txt" | grep -v '^$') \
		> "$WORK/$project.diff" || { cat "$WORK/$project.diff"; fail "$project: the pages the controls show differ from the host's"; }
	echo "wine-e2e: $project: every page the controls show equals the host's golden"
	# The window keeps one rectangle from the first page to the last, and no page needs more than it has.
	[ "$(grep -c '^geometry:' "$WORK/$project.dump")" -ge 5 ] || fail "$project: the run logged no window geometry"
	[ "$(grep '^geometry:' "$WORK/$project.dump" | sed 's/ need .*//' | sort -u | wc -l)" = 1 ] \
		|| { grep '^geometry:' "$WORK/$project.dump"; fail "$project: the window moved or changed size between pages"; }
	grep '^geometry:' "$WORK/$project.dump" | awk '{ split($3, s, "x"); n=$5; print n }' | sort -n | tail -1 > "$WORK/$project.need"
	echo "wine-e2e: $project: one window rectangle on every page ($(grep -m1 '^geometry:' "$WORK/$project.dump" | sed 's/ need.*//'); tallest page asks for $(cat "$WORK/$project.need"))"
	local pf="$WINEPREFIX/drive_c/Program Files (x86)"
	[ -d "$pf/$name" ] || pf="$WINEPREFIX/drive_c/Program Files"
	local dir="$pf/$name"
	[ -d "$dir" ] || fail "$project: nothing was installed in $dir"
	local got
	got="$(cd "$dir" && find . -type f | sed 's|^\./||' | LC_ALL=C sort | tr '\n' ' ')"
	[ "$got" = "$want " ] || fail "$project: installed files were: $got (wanted $want)"
	echo "wine-e2e: $project: installed $got"
	# The files are the project's, byte for byte, and the receipt's CRC-32s say so.
	python3 -I - "$dir" "$ROOT/examples/$project/installer/dist" << 'PY'
import sys, zlib, os
d, dist = sys.argv[1], sys.argv[2]
lines = open(os.path.join(d, "INSTALL.LIR"), encoding="utf-8", errors="replace", newline="").read().split("\r\n")
assert lines[0] == "LIR 1", lines[0]
n = 0
for l in lines:
    if l.startswith("F "):
        name, size, crc = l[2:].rsplit(" ", 2)
        data = open(os.path.join(d, name.replace("\\", "/")), "rb").read()
        assert len(data) == int(size), name
        assert zlib.crc32(data) == int(crc, 16), name
        n += 1
assert n >= 3, n
assert any(l.startswith("R ") for l in lines) and any(l.startswith("S ") for l in lines)
PY
	echo "wine-e2e: $project: the receipt's sizes and CRC-32s match the files"
	local lnk
	lnk="$(find "$WINEPREFIX/drive_c/users" -name '*.lnk' -path "*$name*" | head -1)"
	[ -n "$lnk" ] || fail "$project: no Start menu shortcut"
	echo "wine-e2e: $project: shortcut ${lnk##*/}"
	local un
	un="$(reg "$id" /v UninstallString)"
	grep -q 'UNINST.EXE" /U "' <<< "$un" || fail "$project: no UninstallString in the registry: $un"
	reg "$id" /v DisplayName | grep -q . || fail "$project: no DisplayName"
	echo "wine-e2e: $project: Add/Remove Programs entry has an UninstallString"
	if [ -n "$ext" ]; then
		[ "$(cls ".$ext")" = "$prog" ] || fail "$project: .$ext names '$(cls ".$ext")', not $prog"
		[ "$(cls "$prog")" = "Planboard board" ] || fail "$project: $prog has no name"
		cls "$prog\\shell\\open\\command" | grep -q 'planboard.exe" "%1"$' || fail "$project: no open command: $(cls "$prog\\shell\\open\\command")"
		cls "$prog\\DefaultIcon" | grep -q 'planboard.exe,0$' || fail "$project: no icon: $(cls "$prog\\DefaultIcon")"
		grep -q "^A $ext|$prog|.*planboard.exe|txtfile" "$dir/INSTALL.LIR" || fail "$project: the receipt has no A record that keeps txtfile"
		echo "wine-e2e: $project: .$ext, $prog, its icon and open command are registered; the receipt keeps the old owner"
	fi
	# Uninstall from the installed folder, as Add/Remove Programs runs it: the copy in the temporary folder does the work.
	local wdir="C:\\Program Files (x86)\\$name"
	[ -d "$WINEPREFIX/drive_c/Program Files (x86)/$name" ] || wdir="C:\\Program Files\\$name"
	printf '' > "$WORK/empty.play"
	win "$WORK" "$wdir\\UNINST.EXE" /U "$wdir" /PLAY "$WORK/empty.play" /DUMP "$WORK/$project.undump" || true
	[ ! -e "$dir/INSTALL.LIR" ] && [ ! -e "$dir/UNINST.EXE" ] || fail "$project: the uninstall left files: $(ls "$dir")"
	[ ! -d "$dir" ] || fail "$project: the uninstall left the folder"
	[ -z "$(find "$WINEPREFIX/drive_c/users" -name '*.lnk' -path "*$name*")" ] || fail "$project: the shortcut is still there"
	[ -z "$(reg "$id" /v DisplayName)" ] || fail "$project: the registry entry is still there"
	if [ -n "$ext" ]; then
		[ "$(cls ".$ext")" = "txtfile" ] || fail "$project: .$ext names '$(cls ".$ext")' after the uninstall, not txtfile"
		[ -z "$(cls "$prog")" ] && [ -z "$(cls "$prog\\shell\\open\\command")" ] || fail "$project: $prog's keys are still there"
		echo "wine-e2e: $project: uninstalled: .$ext names txtfile again and $prog is gone"
	fi
	grep -q '^== confirm' "$WORK/$project.undump" || fail "$project: the uninstall did not show its confirm page"
	echo "wine-e2e: $project: uninstalled: files, folder, shortcut and registry entry are gone"
	if [ -n "$SHOTS" ]; then
		for f in "$WORK/shots-$project"/*.bmp; do
			magick "$f" "$SHOTS/$project-$(basename "${f%.bmp}").png"
		done
	fi
}

run_project lt-fizz hq.liminal.lt-fizz "Lieutenant Fizz — Episode One" "FIZZ.EXE INSTALL.LIR LEVELS/EP1.LVL LEVELS/EP1.PAL UNINST.DAT UNINST.EXE"
run_project planboard hq.liminal.planboard "Planboard" "INSTALL.LIR UNINST.DAT UNINST.EXE bin/pbcli.exe docs/GETTING-STARTED.md docs/SHORTCUTS.md planboard.exe" planboard hq.liminal.planboard.planboard
echo "wine-e2e: ok"
