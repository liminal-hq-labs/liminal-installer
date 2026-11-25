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
norm() { sed -e 's/Program Files (x86)/Program Files/g' -e 's/^text: Space available: .*/text: Space available: N/' -e 's/Free: [0-9.]* [KMG]B/Free: N/' -e '/^file:/d' -e '/^report:/d' -e 's/\r$//' "$1"; }
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

# Lt. Fizz's "Bonus levels" and "Open saved games (.FZS) with Fizz" start unticked. The run above left them so (its pages show the empty boxes
# and no file of theirs is installed); these runs tick them through the real check boxes, and install without pages (`/S`, `/COMPONENTS=`).
run_optional() {
	local name="Lieutenant Fizz — Episode One" id=hq.liminal.lt-fizz prog=hq.liminal.lt-fizz.fzs
	local out="$WORK/optional" d dir pf
	"$LIMINST" build -C "$ROOT/examples/lt-fizz" -t windows-x86 --version 1.0.0 -o "$out" --win32-runtime "$HERE/dist/SETUP.EXE" > /dev/null
	d="$out/win32"
	pf="$WINEPREFIX/drive_c/Program Files (x86)"
	[ -d "$pf" ] || pf="$WINEPREFIX/drive_c/Program Files"
	dir="$pf/$name"
	local wdir="C:\\${pf#$WINEPREFIX/drive_c/}\\$name"
	local lir="$dir/INSTALL.LIR"
	seed() { wine reg add "HKCR\\.fzs" /ve /d txtfile /f /reg:32 > /dev/null 2>&1; }
	components() { tr -d '\r' < "$lir" | grep '^components='; }
	gone() { [ ! -d "$dir" ] || fail "optional: $1: the uninstall left the folder: $(ls "$dir")"; [ "$(cls .fzs)" = txtfile ] || fail "optional: $1: .fzs names '$(cls .fzs)' after the uninstall, not txtfile"; [ -z "$(cls "$prog")" ] || fail "optional: $1: $prog is still registered"; }

	# Ticked through the pages: the check boxes are clicked, so the file type is taken and the previous owner kept.
	printf 'licence choose licence.accepted yes\ncomponents toggle bonus\ncomponents toggle savegames\n' > "$WORK/optional.play"
	seed
	win "$d" SETUP.EXE /PLAY "$WORK/optional.play" /DUMP "$WORK/optional.dump" || fail "optional: ticked: SETUP.EXE did not finish ok"
	# The dump is each page as it first appears: the boxes are empty, and clicking them is what installs the files below.
	grep -q '\[ \] Bonus levels' "$WORK/optional.dump" && grep -q '\[ \] Open saved games' "$WORK/optional.dump" || fail "optional: ticked: the check boxes did not start unticked"
	[ -f "$dir/BONUS/BONUS1.LVL" ] || fail "optional: ticked: BONUS1.LVL was not installed"
	[ "$(components)" = "components=game,levels,bonus,savegames" ] || fail "optional: ticked: the receipt says $(components)"
	grep -q "^A fzs|$prog|.*FIZZ.EXE|txtfile" "$lir" || fail "optional: ticked: the receipt has no A record that keeps txtfile"
	[ "$(cls .fzs)" = "$prog" ] && [ "$(cls "$prog")" = "Lt. Fizz saved game" ] || fail "optional: ticked: .fzs names '$(cls .fzs)'"
	echo "wine-e2e: optional: ticked through the pages: the bonus files are installed, .fzs is Fizz's and the receipt keeps txtfile"
	printf '' > "$WORK/empty.play"
	win "$WORK" "$wdir\\UNINST.EXE" /U "$wdir" /PLAY "$WORK/empty.play" /DUMP "$WORK/optional.undump" || true
	gone ticked
	echo "wine-e2e: optional: uninstalled: the bonus files and the folder are gone and .fzs names txtfile again"

	# Silent with the defaults: nothing optional is installed, and the extension is left alone.
	seed
	win "$d" SETUP.EXE /S || fail "optional: /S: SETUP.EXE did not finish ok"
	[ "$(components)" = "components=game,levels" ] && [ ! -e "$dir/BONUS" ] && [ "$(cls .fzs)" = txtfile ] || fail "optional: /S: took $(components)"
	! grep -q '^A ' "$lir" || fail "optional: /S: the receipt records a file type"
	win "$WORK" "$wdir\\UNINST.EXE" /U "$wdir" /S || true
	gone "/S"
	echo "wine-e2e: optional: /S installs the defaults and /U /S removes them; .fzs was never touched"

	# Silent with the optional ones named: exactly these, besides the required component.
	seed
	win "$d" SETUP.EXE /S /COMPONENTS=bonus,savegames || fail "optional: /COMPONENTS=: SETUP.EXE did not finish ok"
	[ "$(components)" = "components=game,bonus,savegames" ] && [ -f "$dir/BONUS/BONUS1.LVL" ] && [ ! -e "$dir/LEVELS" ] || fail "optional: /COMPONENTS=: took $(components)"
	[ "$(cls .fzs)" = "$prog" ] || fail "optional: /COMPONENTS=: .fzs names '$(cls .fzs)'"
	win "$WORK" "$wdir\\UNINST.EXE" /U "$wdir" /S || true
	gone "/COMPONENTS="
	echo "wine-e2e: optional: /S /COMPONENTS=bonus,savegames installs exactly those, and /U /S gives .fzs back"

	# A name that is no component stops the run before anything is written.
	if win "$d" SETUP.EXE /S /COMPONENTS=nothing; then fail "optional: /COMPONENTS=nothing should fail"; fi
	[ ! -d "$dir" ] || fail "optional: /COMPONENTS=nothing installed something"
	echo "wine-e2e: optional: /COMPONENTS=nothing stops and installs nothing"
}

run_project lt-fizz hq.liminal.lt-fizz "Lieutenant Fizz — Episode One" "FIZZ.EXE INSTALL.LIR LEVELS/EP1.LVL LEVELS/EP1.PAL UNINST.DAT UNINST.EXE"
run_project planboard hq.liminal.planboard "Planboard" "INSTALL.LIR UNINST.DAT UNINST.EXE bin/pbcli.exe docs/GETTING-STARTED.md docs/SHORTCUTS.md planboard.exe" planboard hq.liminal.planboard.planboard
run_optional
echo "wine-e2e: ok"
