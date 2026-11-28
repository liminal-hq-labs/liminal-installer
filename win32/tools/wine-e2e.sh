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
# A run ticks Lt. Fizz's screen saver over a seeded file in the Windows folder (backup, receipt, restore on uninstall).
# The last run installs examples/startup-helper (a Startup shortcut, a shortcut's command line, a program run before the uninstall), holds one of its
# files open from another process, uninstalls, and checks that the program ran first and the locked file is left for the next restart.
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

# The receipt's sizes and CRC-32s equal the files. A name is below the install folder, or a full path (a file in the Windows folder),
# which is read through the prefix's drive_c with the case of every part ignored, as Windows does. `$3` is the fewest F records expected.
check_receipt() {
	python3 -I - "$1" "$2" "$3" "$WINEPREFIX/drive_c" << 'PY'
import sys, zlib, os
d, dist, least, drive_c = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]

def find(root, parts):
    for part in parts:
        hit = [n for n in os.listdir(root) if n.lower() == part.lower()]
        assert hit, (root, part)
        root = os.path.join(root, hit[0])
    return root

lines = open(os.path.join(d, "INSTALL.LIR"), encoding="utf-8", errors="replace", newline="").read().split("\r\n")
assert lines[0] == "LIR 1", lines[0]
n = 0
for l in lines:
    if l.startswith("F "):
        name, size, crc = l[2:].rsplit(" ", 2)
        if len(name) > 2 and name[1] == ":":
            assert name[0].upper() == "C", name
            path = find(drive_c, name[3:].split("\\"))
        else:
            path = os.path.join(d, name.replace("\\", "/"))
        data = open(path, "rb").read()
        assert len(data) == int(size), name
        assert zlib.crc32(data) == int(crc, 16), name
        n += 1
assert n >= least, n
assert any(l.startswith("R ") for l in lines) and any(l.startswith("S ") for l in lines)
PY
}

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
	check_receipt "$dir" "$ROOT/examples/$project/installer/dist" 3
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

# The screen saver component installs into the Windows folder. A file of the same name is seeded there first, so the install must back it up
# below the install folder, say so on the log channel, record it in the receipt, and put it back on the uninstall.
run_screensaver() {
	local project=lt-fizz id=hq.liminal.lt-fizz name="Lieutenant Fizz — Episode One"
	local out="$WORK/$project-scr" play="$WORK/$project-scr.play"
	local seeded="$WINEPREFIX/drive_c/windows/FIZZ.SCR"
	"$LIMINST" build -C "$ROOT/examples/$project" -t windows-x86 --version 1.0.0 -o "$out" --win32-runtime "$HERE/dist/SETUP.EXE" > /dev/null
	[ -d "${seeded%/*}" ] || fail "screensaver: the prefix has no Windows folder"
	printf "someone else's screen saver\r\n" > "$seeded"
	cp "$seeded" "$WORK/seeded.scr"
	printf 'licence choose licence.accepted yes\ncomponents toggle screensaver\n' > "$play"
	win "$out/win32" SETUP.EXE /PLAY "$play" /DUMP "$WORK/scr.dump" || fail "screensaver: SETUP.EXE did not finish ok"
	local dir="$WINEPREFIX/drive_c/Program Files (x86)/$name"
	[ -d "$dir" ] || dir="$WINEPREFIX/drive_c/Program Files/$name"
	[ -d "$dir" ] || fail "screensaver: nothing was installed in $dir"
	cmp "$seeded" "$ROOT/examples/$project/installer/dist/screensaver/FIZZ.SCR" || fail "screensaver: the Windows folder's FIZZ.SCR is not the product's"
	[ -f "$dir/BACKUP/WIN/FIZZ.SCR" ] || fail "screensaver: no backup in $dir/BACKUP/WIN"
	cmp "$dir/BACKUP/WIN/FIZZ.SCR" "$WORK/seeded.scr" || fail "screensaver: the backup is not the seeded file"
	grep -qi '^notice: backup: C:\\windows\\FIZZ.SCR -> BACKUP\\WIN\\FIZZ.SCR' "$WORK/scr.dump" || { grep '^notice:' "$WORK/scr.dump" || true; fail "screensaver: the log has no backup line"; }
	tr -d '\r' < "$dir/INSTALL.LIR" | grep -qi '^F C:\\windows\\FIZZ.SCR ' || fail "screensaver: the receipt has no F record for the file"
	tr -d '\r' < "$dir/INSTALL.LIR" | grep -qi '^O C:\\windows\\FIZZ.SCR|' || fail "screensaver: the receipt has no O record"
	check_receipt "$dir" "$ROOT/examples/$project/installer/dist" 4
	echo "wine-e2e: screensaver: FIZZ.SCR is in the Windows folder, the seeded file is in BACKUP/WIN, and the receipt records both"
	local wdir="C:\\Program Files (x86)\\$name"
	[ -d "$WINEPREFIX/drive_c/Program Files (x86)/$name" ] || wdir="C:\\Program Files\\$name"
	printf '' > "$WORK/empty.play"
	win "$WORK" "$wdir\\UNINST.EXE" /U "$wdir" /PLAY "$WORK/empty.play" /DUMP "$WORK/scr.undump" || true
	[ ! -d "$dir" ] || fail "screensaver: the uninstall left the install folder: $(ls -R "$dir")"
	cmp "$seeded" "$WORK/seeded.scr" || fail "screensaver: the uninstall did not put the seeded file back"
	[ -z "$(reg "$id" /v DisplayName)" ] || fail "screensaver: the registry entry is still there"
	rm -f "$seeded"
	echo "wine-e2e: screensaver: uninstalled: the seeded file is back, and BACKUP and the install folder are gone"
}

# examples/startup-helper: a shortcut in the Startup folder, a shortcut with a command line, `[[uninstall.pre]]` and a file still in use at the uninstall.
# STOP.BAT stands in for `HELPER.EXE /quit`; it writes `%TEMP%\li-pre-uninstall-ran.txt` with its argument and whether HELPER.EXE was still there.
# A second process (a batch file that pings, redirecting into HELPER.LOG) holds HELPER.LOG open without sharing for delete, so Wine refuses to
# remove it as Windows does, and the uninstall must leave it for the restart: MoveFileEx's delay writes PendingFileRenameOperations.
run_helper() {
	local project=startup-helper id=hq.liminal.helper-sample name="Helper Sample"
	local out="$WORK/$project" play="$WORK/$project.play"
	"$LIMINST" build -C "$ROOT/examples/$project" -t windows-x86 --version 1.0.0 -o "$out" --win32-runtime "$HERE/dist/SETUP.EXE" > /dev/null
	printf 'licence choose licence.accepted yes\n' > "$play"
	win "$out/win32" SETUP.EXE /PLAY "$play" /DUMP "$WORK/$project.dump" || fail "$project: SETUP.EXE did not finish ok"
	local pf="$WINEPREFIX/drive_c/Program Files (x86)"
	[ -d "$pf/$name" ] || pf="$WINEPREFIX/drive_c/Program Files"
	local dir="$pf/$name" wdir="C:\\Program Files (x86)\\$name"
	[ -d "$pf/$name" ] && [ -d "$WINEPREFIX/drive_c/Program Files (x86)/$name" ] || wdir="C:\\Program Files\\$name"
	[ -f "$dir/HELPER.LOG" ] || fail "$project: nothing was installed in $dir"
	# The Startup shortcut is in the Startup folder and not in the product's Start menu folder; the other two items are in the product's folder.
	local startup stop plain
	startup="$(find "$WINEPREFIX/drive_c/users" -name 'Helper Sample at login.lnk' -path '*Start Menu/Programs/StartUp/*' | head -1)"
	[ -n "$startup" ] || fail "$project: no shortcut in the Startup folder"
	[ -z "$(find "$WINEPREFIX/drive_c/users" -name 'Helper Sample at login.lnk' -path "*Programs/$name/*")" ] || fail "$project: the Startup shortcut is also in the product's folder"
	stop="$(find "$WINEPREFIX/drive_c/users" -name 'Stop Helper Sample.lnk' -path "*Programs/$name/*" | head -1)"
	plain="$(find "$WINEPREFIX/drive_c/users" -name 'Helper Sample.lnk' -path "*Programs/$name/*" | head -1)"
	[ -n "$stop" ] && [ -n "$plain" ] || fail "$project: the Start menu items are missing"
	# A shell link keeps its command line as a string (UTF-16 when the link says so, else the system code page).
	has_args() { python3 -I - "$1" "$2" << 'PY'
import sys
data = open(sys.argv[1], "rb").read()
want = sys.argv[2]
sys.exit(0 if want.encode("utf-16le") in data or want.encode("ascii") in data else 1)
PY
	}
	has_args "$startup" "/background" || fail "$project: the Startup shortcut has no /background"
	has_args "$stop" "/quit" || fail "$project: Stop Helper Sample has no /quit"
	! has_args "$plain" "/quit" || fail "$project: the plain item has a command line"
	echo "wine-e2e: $project: the Startup folder holds the login shortcut with /background; Stop Helper Sample has /quit; the plain item has none"
	tr -d '\r' < "$dir/INSTALL.LIR" | grep -q '^S .*\\StartUp\\Helper Sample at login.lnk$' || fail "$project: the receipt has no S record for the Startup shortcut"
	tr -d '\r' < "$dir/INSTALL.LIR" | grep -q '^U .*STOP.BAT|/quit|5000|1$' || fail "$project: the receipt has no U record for the program to run first"
	echo "wine-e2e: $project: the receipt records the Startup shortcut (S) and the program to run first (U)"
	cp "$dir/INSTALL.LIR" "$WORK/$project-INSTALL.LIR"
	# Hold HELPER.LOG open from another process: a second SETUP.EXE whose /DUMP file is HELPER.LOG, which it opens for writing with sharing for
	# reading only, so Windows (and Wine) answer "sharing violation" to a delete. It waits at its first page until the prefix's server is stopped.
	( cd "$out/win32" && timeout 150 xvfb-run -a -s "-screen 0 1024x768x24" wine SETUP.EXE /DUMP "$wdir\\HELPER.LOG" > /dev/null 2>&1 ) &
	local holder=$!
	sleep 5
	printf '' > "$WORK/empty.play"
	# The uninstaller hands over to a copy of itself, which needs the display until its last page, so wait for that page, not for the server.
	timeout 150 xvfb-run -a -s "-screen 0 1024x768x24" bash -c 'wine "$1\\UNINST.EXE" /U "$1" /PLAY "$2" /DUMP "$3"; for _ in $(seq 90); do grep -q "^== done" "$3" 2> /dev/null && break; sleep 1; done; sleep 1' bash "$wdir" "$WORK/empty.play" "$WORK/$project.undump" > /dev/null 2>&1 || true
	local marker
	marker="$(find "$WINEPREFIX/drive_c/users" -name li-pre-uninstall-ran.txt | head -1)"
	[ -n "$marker" ] || fail "$project: the program to run first did not run"
	[ "$(tr -d '\r\n' < "$marker")" = "/quit files-present" ] || fail "$project: the program ran as '$(cat "$marker")', not '/quit files-present'"
	echo "wine-e2e: $project: the program ran first, with /quit, while the product's files were still there"
	[ -f "$dir/HELPER.LOG" ] || fail "$project: the locked file is gone (it was not locked)"
	[ ! -e "$dir/HELPER.EXE" ] && [ ! -e "$dir/HELPER.INI" ] && [ ! -e "$dir/INSTALL.LIR" ] && [ ! -e "$dir/UNINST.EXE" ] || fail "$project: the uninstall left files: $(ls "$dir")"
	[ -z "$(find "$WINEPREFIX/drive_c/users" -name '*Helper Sample*.lnk')" ] || fail "$project: a shortcut is still there"
	[ -z "$(reg "$id" /v DisplayName)" ] || fail "$project: the registry entry is still there"
	echo "wine-e2e: $project: every other file, all three shortcuts (the Startup one included) and the registry entry are gone"
	local pending
	pending="$(wine reg query 'HKLM\System\CurrentControlSet\Control\Session Manager' /v PendingFileRenameOperations 2>&1 | tr -d '\r' || true)"
	grep -qi 'Helper Sample.HELPER.LOG' <<< "$pending" || { echo "$pending"; fail "$project: PendingFileRenameOperations does not list HELPER.LOG"; }
	echo "wine-e2e: $project: PendingFileRenameOperations lists the locked HELPER.LOG, so Windows removes it at the next restart"
	# With --shots, the receipt the install wrote and the pages the uninstall logged are kept beside the pictures.
	if [ -n "$SHOTS" ]; then
		cp "$WORK/$project.dump" "$SHOTS/$project-install-dump.txt"
		cp "$WORK/$project.undump" "$SHOTS/$project-uninstall-dump.txt"
		printf '%s\n' "$pending" > "$SHOTS/$project-pending-file-rename-operations.txt"
		[ ! -f "$WORK/$project-INSTALL.LIR" ] || cp "$WORK/$project-INSTALL.LIR" "$SHOTS/$project-INSTALL.LIR"
	fi
	# Stops the holder: every Wine process of this scratch prefix, and no other.
	wineserver -k || true
	wait "$holder" || true
	grep -qi '^notice: removed at restart: .*HELPER.LOG' "$WORK/$project.undump" || { grep '^notice:\|^report:' "$WORK/$project.undump" || true; fail "$project: the log has no restart line"; }
	grep -q '^report: 1 file is still in use. Windows will remove it the next time it starts.' "$WORK/$project.undump" || fail "$project: the person was not told about the restart"
	echo "wine-e2e: $project: the uninstall told the person: 1 file is still in use, removed the next time Windows starts"
}

# LI_WINE_ONLY=helper runs only that part (lt-fizz, planboard, screensaver or helper); the default runs all of them.
want() { [ -z "${LI_WINE_ONLY:-}" ] || [ "$LI_WINE_ONLY" = "$1" ]; }
want lt-fizz && run_project lt-fizz hq.liminal.lt-fizz "Lieutenant Fizz — Episode One" "FIZZ.EXE INSTALL.LIR LEVELS/EP1.LVL LEVELS/EP1.PAL UNINST.DAT UNINST.EXE"
want planboard && run_project planboard hq.liminal.planboard "Planboard" "INSTALL.LIR UNINST.DAT UNINST.EXE bin/pbcli.exe docs/GETTING-STARTED.md docs/SHORTCUTS.md planboard.exe" planboard hq.liminal.planboard.planboard
want screensaver && run_screensaver
want helper && run_helper
echo "wine-e2e: ok"
