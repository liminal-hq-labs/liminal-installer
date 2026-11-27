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
# Windows 3.11 run (e2e-w31.sh). Planboard also declares a file type: a `pbd=` line in WIN.INI's `[Extensions]` replaces one seeded
# first (Notepad's), and after the uninstall the seeded line is back (Wine keeps the section in its registry). A third run, `lt-fizz-saver`,
# is Lt. Fizz with its Screen saver component ticked (`components toggle screensaver` in the script) over a `FIZZ.SCR` seeded in the Windows
# folder (Wine's `C:\windows`): the install must put the new file there (its receipt record is a full path, matched without regard to case),
# copy the seeded one to `BACKUP\WIN\FIZZ.SCR` in the install folder, keep an `O` record and log a `backup:` line, and the uninstall must
# put the seeded bytes back and leave no `BACKUP`; the plain Lt. Fizz run must leave no `FIZZ.SCR` in the Windows folder. Skips with a note
# when Wine is missing.
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
# resolve: a receipt name to a path under the prefix. A name with a drive (`C:\windows\FIZZ.SCR`) is found under drive_c without regard
# to case, because Wine's Windows folder is `windows`; any other name is below the install folder.
resolve() {
	case "$2" in
		[A-Za-z]:\\*)
			local rel="${2:3}"
			find "$WINEPREFIX/drive_c" -ipath "$WINEPREFIX/drive_c/$(echo "$rel" | tr '\\' '/')" 2> /dev/null | head -n1
			;;
		*) echo "$1/$(echo "$2" | tr '\\' '/')" ;;
	esac
}
winfiles="$WINEPREFIX/drive_c/windows"

for ex in lt-fizz planboard lt-fizz-saver; do
	base="${ex%-saver}"
	dir="$work/$ex"
	mkdir -p "$dir"
	"$liminst" build -C "examples/$base" -t win16-x86 --version 1.0.0 -o "$dir/out" --output files --win16-runtime "$work/exe/SETUP.EXE" > /dev/null 2>&1
	cp "$dir/out/win16/SETUP.EXE" "$dir/out/win16/SETUP.LPK" "$dir/"
	printf 'licence choose licence.accepted yes\n' > "$dir/PLAY.TXT"
	if [ "$ex" = lt-fizz-saver ]; then
		printf 'components toggle screensaver\n' >> "$dir/PLAY.TXT"
		printf 'seeded by e2e-wine\r\nthe saver that was here first\r\n' > "$winfiles/FIZZ.SCR"
		cp "$winfiles/FIZZ.SCR" "$dir/seeded.scr"
	fi
	: > "$dir/PLAY2.TXT"
	name="$([ "$base" = lt-fizz ] && echo LTFIZZ || echo PLANBOAR)"
	host="$WINEPREFIX/drive_c/$name"
	[ "$ex" != planboard ] || seed_pbd
	[ "$ex" != planboard ] || [ "$(pbd_line)" = 'pbd=C:\WINDOWS\NOTEPAD.EXE ^.pbd' ] || { echo "e2e-wine: could not seed WIN.INI: $(pbd_line)" >&2; fail=1; }
	(cd "$dir" && wine SETUP.EXE "/LOG=C:\\$ex.LOG" /PLAY=PLAY.TXT "/DUMP=C:\\$ex.TXT" > /dev/null 2>&1) || true
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
	if [ "$ex" = lt-fizz-saver ]; then
		echo "e2e-wine: $ex: the components page differs from the golden by the ticked saver, so its pages are not compared"
	elif diff <(norm "$WINEPREFIX/drive_c/$ex.TXT") <(norm "crates/li-retro-build/tests/golden/win16/$ex.pages.txt") > "$dir/diff"; then
		echo "e2e-wine: $ex: every page the controls show equals the host's golden"
	else
		cat "$dir/diff" >&2
		fail=1
	fi
	n=0
	while read -r kind fname size _crc; do
		[ "$kind" = F ] || continue
		path="$(resolve "$host" "$fname")"
		[ -n "$path" ] && [ "$(stat -c %s "$path")" = "$size" ] || { echo "e2e-wine: $ex: $fname is not $size bytes" >&2; fail=1; }
		n=$((n + 1))
	done < <(tr -d '\r' < "$host/INSTALL.LIR")
	echo "e2e-wine: $ex installed $n files, receipt checked"
	echo "e2e-wine: $ex: the receipt has $(grep -c '^G ' "$host/INSTALL.LIR") group item(s) and $(grep -c '^P ' "$host/INSTALL.LIR") made group(s)"
	receipt="$(tr -d '\r' < "$host/INSTALL.LIR")"
	if [ "$ex" = lt-fizz-saver ]; then
		if grep -qix 'F C:\\windows\\FIZZ.SCR [0-9]* [0-9A-F]*' <<< "$receipt" \
			&& grep -qix 'O C:\\windows\\FIZZ.SCR|BACKUP\\WIN\\FIZZ.SCR' <<< "$receipt" \
			&& cmp -s "$host/BACKUP/WIN/FIZZ.SCR" "$dir/seeded.scr" \
			&& ! cmp -s "$winfiles/FIZZ.SCR" "$dir/seeded.scr" \
			&& grep -q '^backup: C:\\windows\\FIZZ.SCR -> BACKUP\\WIN\\FIZZ.SCR' <<< "$(tr -d '\r' < "$WINEPREFIX/drive_c/$ex.LOG")"; then
			echo "e2e-wine: $ex: FIZZ.SCR is in the Windows folder, the seeded one is in BACKUP\\WIN, the receipt has its F and O records and the log says so"
		else
			echo "e2e-wine: $ex: the saver, its backup, the receipt or the log is wrong" >&2
			echo "$receipt" | grep -i 'FIZZ.SCR\|^O ' >&2 || true
			ls -R "$host" >&2 || true
			fail=1
		fi
	elif [ "$ex" = lt-fizz ] && [ -e "$winfiles/FIZZ.SCR" ]; then
		echo "e2e-wine: $ex: the unticked saver was installed" >&2
		fail=1
	fi
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
		(cd "$dir" && wine SETUP.EXE /U "C:\\$name" "/LOG=C:\\$ex-u.LOG" /PLAY=PLAY2.TXT > /dev/null 2>&1) || true
	fi
	if [ -e "$host" ]; then
		echo "e2e-wine: $ex uninstall left $host behind" >&2
		fail=1
	else
		echo "e2e-wine: $ex uninstalled"
	fi
	if [ "$ex" = lt-fizz-saver ]; then
		if cmp -s "$winfiles/FIZZ.SCR" "$dir/seeded.scr"; then
			echo "e2e-wine: $ex: the seeded FIZZ.SCR is back after the uninstall and BACKUP is gone"
		else
			echo "e2e-wine: $ex: the seeded FIZZ.SCR was not put back" >&2
			fail=1
		fi
		rm -f "$winfiles/FIZZ.SCR"
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
exit "$fail"
