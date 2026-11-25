#!/usr/bin/env bash
# Installs and uninstalls Lt. Fizz and Planboard with the real SETUP.EXE under Wine, and checks the files, the receipt and the pages.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: [KEPT=1] retro/win16/tools/e2e-wine.sh
#
# With KEPT=1 the uninstall is the kept `UNINST.EXE /U` of the install folder instead of `SETUP.EXE /U`.
#
# Builds SETUP.EXE and the two packs (`liminst build -t win16-x86`), then for each example runs `SETUP.EXE /PLAY=PLAY.TXT /DUMP=...`: the
# script presses the real controls (the licence choice, then every default button) and `/DUMP` writes the text of each page, which is
# compared with the host test's golden (crates/li-retro-build/tests/golden/win16). The receipt's sizes and the files are checked, then
# `SETUP.EXE /U` removes it again and the folder must be gone. Wine runs Win16 programs through its own krnl386, which keeps the
# register halves Windows itself would not, so this proves the flow and the file handling, not the register handling: that is the
# Windows 3.11 run (e2e-w31.sh). Wine is unreliable with a Win16 program on a loaded machine, so a step that ends with no `exit` line in its log is run again (five tries). Planboard also declares a file type: a `pbd=` line in WIN.INI's `[Extensions]` replaces one seeded
# first (Notepad's), and after the uninstall the seeded line is back (Wine keeps the section in its registry). Skips with a note when Wine is missing.
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

# The `pbd` line of WIN.INI's [Extensions] section. Wine maps that section of WIN.INI to the registry, as Windows NT does, so it is read and
# seeded there; the Windows 3.11 run (e2e-w31.sh) reads the real WIN.INI.
extkey='HKCU\Software\Microsoft\Windows NT\CurrentVersion\Extensions'
pbd_line() { wine reg query "$extkey" /v pbd 2> /dev/null | tr -d '\r' | sed -n 's/^ *pbd *REG_SZ *\(.*\)$/pbd=\1/p'; }
seed_pbd() { wine reg add "$extkey" /v pbd /t REG_SZ /d 'C:\WINDOWS\NOTEPAD.EXE ^.pbd' /f > /dev/null 2>&1; }

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
	[ "$ex" != planboard ] || seed_pbd
	[ "$ex" != planboard ] || [ "$(pbd_line)" = 'pbd=C:\WINDOWS\NOTEPAD.EXE ^.pbd' ] || { echo "e2e-wine: could not seed WIN.INI: $(pbd_line)" >&2; fail=1; }
	# Wine runs a Win16 program unreliably on a loaded machine (the runtime before the optional components dies part-way about as often): a run
	# that ends with no `exit` line is run again from a clean slate, five tries at most. A run that ends with one is judged as it ended.
	for try in 1 2 3 4 5; do
		rm -rf "$host" "$WINEPREFIX/drive_c/$ex.LOG"
		[ "$ex" != planboard ] || seed_pbd
		(cd "$dir" && wine SETUP.EXE "/LOG=C:\\$ex.LOG" /PLAY=PLAY.TXT "/DUMP=C:\\$ex.TXT" > /dev/null 2>&1) || true
		grep -q '^exit' "$WINEPREFIX/drive_c/$ex.LOG" 2> /dev/null && break
		echo "e2e-wine: $ex: SETUP.EXE ended with no exit line (try $try); again" >&2
	done
	if [ "$(tail -n1 "$WINEPREFIX/drive_c/$ex.LOG" 2> /dev/null | tr -d '\r')" != "exit 0" ]; then
		echo "e2e-wine: $ex install did not exit 0" >&2
		fail=1
		continue
	fi
	# Program Manager: Wine may or may not answer the DDE conversation; either is logged and either passes, with a note.
	pm="$(tr -d '\r' < "$WINEPREFIX/drive_c/$ex.LOG" | grep '^progman:' | tr '\n' ';' || true)"
	case "$pm" in
		*"progman: added"*) echo "e2e-wine: $ex: Program Manager accepted the conversation ($pm)" ;;
		*"progman: unavailable"*) echo "e2e-wine: $ex: no Program Manager under Wine; the batch launcher was written ($pm)" ;;
		*) echo "e2e-wine: $ex: unexpected Program Manager result: ${pm:-none}" >&2; fail=1 ;;
	esac
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
	echo "e2e-wine: $ex: the receipt has $(grep -c '^G ' "$host/INSTALL.LIR") group item(s) and $(grep -c '^P ' "$host/INSTALL.LIR") made group(s)"
	if [ "$ex" = planboard ]; then
		if [ "$(pbd_line)" = "pbd=C:\\$name\\PLANBRD.EXE ^.pbd" ] && tr -d '\r' < "$host/INSTALL.LIR" | grep -qxF "A pbd|hq.liminal.planboard.pbd|C:\\$name\\PLANBRD.EXE|C:\\WINDOWS\\NOTEPAD.EXE ^.pbd"; then
			echo "e2e-wine: $ex: WIN.INI's [Extensions] names Planboard, and the receipt keeps Notepad's line"
		else
			echo "e2e-wine: $ex: WIN.INI has '$(pbd_line)' and the receipt '$(grep '^A ' "$host/INSTALL.LIR")'" >&2
			fail=1
		fi
	fi
	if [ -n "${KEPT:-}" ]; then
		# The kept uninstaller: it copies itself to the temporary folder and runs from there.
		[ -f "$host/UNINST.EXE" ] && [ -f "$host/UNINST.DAT" ] || { echo "e2e-wine: $ex: no kept uninstaller" >&2; fail=1; }
		# The copy runs from the temporary folder, so its script is given by an absolute path.
		cp "$dir/PLAY2.TXT" "$WINEPREFIX/drive_c/PLAY2.TXT"
		(cd "$dir" && wine "C:\\$name\\UNINST.EXE" /U "C:\\$name" "/LOG=C:\\$ex-u.LOG" /PLAY=C:\\PLAY2.TXT > /dev/null 2>&1) || true
		for _ in $(seq 1 60); do grep -q '^exit' "$WINEPREFIX/drive_c/$ex-u.LOG" 2> /dev/null && break; sleep 1; done
	else
		for try in 1 2 3 4 5; do
			rm -f "$WINEPREFIX/drive_c/$ex-u.LOG"
			(cd "$dir" && wine SETUP.EXE /U "C:\\$name" "/LOG=C:\\$ex-u.LOG" /PLAY=PLAY2.TXT > /dev/null 2>&1) || true
			grep -q '^exit' "$WINEPREFIX/drive_c/$ex-u.LOG" 2> /dev/null && break
			echo "e2e-wine: $ex: the uninstall ended with no exit line (try $try); again" >&2
		done
	fi
	if [ -e "$host" ]; then
		echo "e2e-wine: $ex uninstall left $host behind" >&2
		fail=1
	else
		echo "e2e-wine: $ex uninstalled"
	fi
	if [ "$ex" = planboard ]; then
		if [ "$(pbd_line)" = 'pbd=C:\WINDOWS\NOTEPAD.EXE ^.pbd' ]; then
			echo "e2e-wine: $ex: WIN.INI [Extensions] has Notepad's line back"
		else
			echo "e2e-wine: $ex: WIN.INI has '$(pbd_line)' after the uninstall" >&2
			fail=1
		fi
	fi
done
# Lt. Fizz's "Bonus levels" and "Open saved games (.FZS) with Fizz" start unticked: the loop above left them so (its pages show the empty boxes and
# the receipt names three components). These runs tick them through the real check boxes, and install without pages (`/S`, `/COMPONENTS=`).
optional() {
	local dir="$work/optional" host="$WINEPREFIX/drive_c/LTFIZZ" lir="$WINEPREFIX/drive_c/LTFIZZ/INSTALL.LIR" old='C:\WINDOWS\NOTEPAD.EXE ^.fzs'
	mkdir -p "$dir"
	cp "$work/lt-fizz/SETUP.EXE" "$work/lt-fizz/SETUP.LPK" "$dir/"
	printf 'licence choose licence.accepted yes\ncomponents toggle bonus\ncomponents toggle savegames\n' > "$dir/PLAY3.TXT"
	: > "$dir/PLAY2.TXT"
	fzs_line() { wine reg query "$extkey" /v fzs 2> /dev/null | tr -d '\r' | sed -n 's/^ *fzs *REG_SZ *\(.*\)$/\1/p'; }
	seed_fzs() { wine reg add "$extkey" /v fzs /t REG_SZ /d "$old" /f > /dev/null 2>&1; }
	comps() { tr -d '\r' < "$lir" 2> /dev/null | grep '^components=' || true; }
	check() { # <what> <test command...>: records a failure and says what
		local what="$1"; shift
		"$@" || { echo "e2e-wine: optional: FAIL: $what" >&2; fail=1; }
	}
	# Wine runs a Win16 program unreliably on a loaded machine: SETUP.EXE of the commit before these runs dies part-way about as often (a run
	# that ends with no `exit` line). So each step runs again, from a clean slate for an install, until its log ends in one (five tries).
	tries() { # <log in C:> <install|uninstall> <program arguments...>
		local log="$WINEPREFIX/drive_c/$1" kind="$2" n; shift 2
		for n in 1 2 3 4 5; do
			rm -f "$log"
			(cd "$dir" && wine SETUP.EXE "$@" "/LOG=C:\\$(basename "$log")" > /dev/null 2>&1) || true
			grep -q '^exit' "$log" 2> /dev/null && return 0
			echo "e2e-wine: optional: SETUP.EXE ended with no exit line (try $n); again" >&2
			[ "$kind" = install ] && { rm -rf "$host"; seed_fzs; }
		done
		return 1
	}
	removed() { # <what>
		check "$1: the folder is gone" test ! -e "$host"
		check "$1: .fzs is '$old' again (it is '$(fzs_line)')" test "$(fzs_line)" = "$old"
	}

	# Ticked through the pages.
	seed_fzs
	tries opt.LOG install /PLAY=PLAY3.TXT "/DUMP=C:\\opt.TXT"
	check "ticked: exit 0" test "$(tail -n1 "$WINEPREFIX/drive_c/opt.LOG" 2> /dev/null | tr -d '\r')" = "exit 0"
	# The dump is each page as it first appears: the boxes are empty, and clicking them is what installs the files below.
	check "ticked: the boxes start unticked" grep -q '\[ \] Open saved games' "$WINEPREFIX/drive_c/opt.TXT"
	check "ticked: BONUS1.LVL is installed" test -f "$host/BONUS/BONUS1.LVL"
	check "ticked: the receipt says $(comps)" test "$(comps)" = "components=game,sound,levels,bonus,savegames"
	check "ticked: the receipt keeps the old line" test "$(tr -d '\r' < "$lir" | grep '^A fzs')" = "A fzs|hq.liminal.lt-fizz.fzs|C:\\LTFIZZ\\FIZZ.EXE|$old"
	check "ticked: WIN.INI names Fizz (it is '$(fzs_line)')" test "$(fzs_line)" = 'C:\LTFIZZ\FIZZ.EXE ^.fzs'
	echo "e2e-wine: optional: ticked through the pages: bonus files installed, .fzs is Fizz's, the receipt keeps Notepad's line"
	tries opt-u.LOG uninstall /U 'C:\LTFIZZ' /PLAY=PLAY2.TXT
	removed ticked
	echo "e2e-wine: optional: uninstalled: the bonus files go and WIN.INI has Notepad's line back"

	# Silent, defaults: nothing optional, and the line is left alone.
	seed_fzs
	tries opt.LOG install /S
	check "/S: exit 0" test "$(tail -n1 "$WINEPREFIX/drive_c/opt.LOG" 2> /dev/null | tr -d '\r')" = "exit 0"
	check "/S: took $(comps)" test "$(comps)" = "components=game,sound,levels"
	check "/S: no bonus files" test ! -e "$host/BONUS"
	check "/S: .fzs untouched" test "$(fzs_line)" = "$old"
	# The kept uninstaller starts a copy of itself from the temporary folder; the copy has to stay silent too.
	for n in 1 2 3 4 5; do
		rm -f "$WINEPREFIX/drive_c/opt-u.LOG"
		(cd "$dir" && wine 'C:\LTFIZZ\UNINST.EXE' /U 'C:\LTFIZZ' /S "/LOG=C:\\opt-u.LOG" > /dev/null 2>&1) || true
		for _ in $(seq 1 60); do grep -q '^exit' "$WINEPREFIX/drive_c/opt-u.LOG" 2> /dev/null && break; sleep 1; done
		grep -q '^exit' "$WINEPREFIX/drive_c/opt-u.LOG" 2> /dev/null && break
		echo "e2e-wine: optional: UNINST.EXE ended with no exit line (try $n); again" >&2
	done
	removed "/S"
	echo "e2e-wine: optional: /S installs the defaults and the kept UNINST.EXE /U /S removes them"

	# Silent, named.
	seed_fzs
	tries opt.LOG install /S /COMPONENTS=bonus,savegames
	check "/COMPONENTS=: took $(comps)" test "$(comps)" = "components=game,bonus,savegames"
	check "/COMPONENTS=: bonus file" test -f "$host/BONUS/BONUS1.LVL"
	check "/COMPONENTS=: no levels" test ! -e "$host/LEVELS"
	check "/COMPONENTS=: WIN.INI names Fizz" test "$(fzs_line)" = 'C:\LTFIZZ\FIZZ.EXE ^.fzs'
	tries opt-u.LOG uninstall /U 'C:\LTFIZZ' /S
	removed "/COMPONENTS="
	echo "e2e-wine: optional: /S /COMPONENTS=bonus,savegames installs exactly those and /U /S gives the line back"

	# A name that is no component stops before anything is written.
	tries opt3.LOG install /S /COMPONENTS=nothing
	check "/COMPONENTS=nothing stops with exit 2" test "$(tail -n1 "$WINEPREFIX/drive_c/opt3.LOG" 2> /dev/null | tr -d '\r')" = "exit 2"
	check "/COMPONENTS=nothing installs nothing" test ! -e "$host"
	echo "e2e-wine: optional: /COMPONENTS=nothing stops with exit 2 and installs nothing"
}
optional
exit "$fail"
