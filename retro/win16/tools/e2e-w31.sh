#!/usr/bin/env bash
# Installs and uninstalls an example with the real SETUP.EXE on Windows 3.11 in DOSBox-X, many times, and counts the passes.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: W31_TREE=DIR [SHOTS=DIR] retro/win16/tools/e2e-w31.sh [--runs N] [lt-fizz|planboard]...
#
# W31_TREE is a Windows 3.11 installation as a DOS drive (a folder with WINDOWS/ and WIN.BAT). The repository cannot carry one, so this
# is a manual run. It copies the tree, mounts the copy as C: and a scratch folder (SETUP.EXE, SETUP.LPK, the play script) as D: in
# DOSBox-X, and for each run starts Windows twice, each time on a fresh DOSBox: `win /3 D:\SETUP.EXE /PLAY=... /DUMP=...` presses the real
# controls and writes the page text, then `SETUP.EXE /U` removes the install. A run passes when both exit 0, the pages equal the host's golden,
# the files are the receipt's sizes and the folder is gone. DOSBox-X comes from the `dosbox-x` on the path or the Docker image named by
# DOSBOX_IMAGE (default liminal-installer-retro:local, from retro/build.sh). It needs an X server (xvfb-run is used when DISPLAY is unset).
# With SHOTS set, the first run of each example saves a screenshot of every page the script waits on (`/PAUSE=2500`), one of Program
# Manager with the new group open (4 s after SETUP exits) and one of Program Manager after the uninstall, with the group gone.
# A run also needs the Program Manager conversation to have worked: the log says `progman: added` and, on removal, `progman: removed;
# group gone`, the group's .GRP file exists in C:\WINDOWS after the install and is gone after the uninstall, and PROGMAN.INI names it
# only in between. KEPT=1 removes the install with the uninstaller it keeps (`C:\\FOLDER\\UNINST.EXE /U C:\\FOLDER`, which copies itself to the
# temporary folder) in place of `SETUP /U` from the disk, and also checks that UNINST.EXE and UNINST.DAT are in the folder after the install
# and that the group holds the uninstall item next to the program's. Planboard declares a file type: before each run WIN.INI's [Extensions]
# is given a `pbd=notepad.exe ^.pbd` line, and a run needs the install to replace it with Planboard's (`pbd=C:\PLANBOAR\PLANBRD.EXE ^.pbd`),
# the receipt to keep Notepad's, and the uninstall to put `pbd=notepad.exe ^.pbd` back. With NO_OWNER=1 nothing owns `pbd` first, and the uninstall must leave no `pbd=` line at all, empty or not. PROGMAN_OPTIONAL=1 turns the Program Manager checks off, for a tree whose shell is not Program Manager.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."
: "${W31_TREE:?set W31_TREE to a Windows 3.11 tree}"
runs=3
examples=()
while [ $# -gt 0 ]; do
	case "$1" in
		--runs) runs="$2"; shift 2 ;;
		*) examples+=("$1"); shift ;;
	esac
done
[ ${#examples[@]} -gt 0 ] || examples=(lt-fizz planboard)

if [ -z "${DISPLAY:-}" ] && [ -z "${E2E_W31_XVFB:-}" ] && command -v xvfb-run > /dev/null; then
	E2E_W31_XVFB=1 exec xvfb-run -a -s "-screen 0 1280x960x24" "$0" --runs "$runs" "${examples[@]}"
fi

work="$(mktemp -d)"
name="w16dbx-$$"
cleanup() { docker rm -f "$name" > /dev/null 2>&1 || true; rm -rf "$work"; }
trap cleanup EXIT
mkdir -p "$work/d" "$work/conf"
cp -a --reflink=auto "$W31_TREE" "$work/c"

OUT_DIR="$work/exe" bash retro/win16/build.sh > /dev/null
cargo build -q --locked -p li-cli
liminst="${CARGO_TARGET_DIR:-$PWD/target}/debug/liminst"

# restore_pm: brings Program Manager up through the Task List (Ctrl+Esc, then Down twice to "Program Manager" in this tree, then Return),
# because this tree starts it minimized and a screenshot should show its groups.
restore_pm() {
	xdotool key ctrl+Escape 2> /dev/null || true
	sleep 1
	xdotool key Down 2> /dev/null || true
	xdotool key Down 2> /dev/null || true
	xdotool key Return 2> /dev/null || true
	sleep 3
}

# session <WIN command line> <log in C:> <seconds>: one Windows session; true when its log reaches an `exit` line.
session() {
	{
		cat << 'CONF'
[sdl]
output=surface
fullscreen=false
windowresolution=800x600
autolock=false
mouse_emulation=always
[dosbox]
machine=svga_s3
memsize=32
[cpu]
core=normal
cputype=pentium
cycles=max
[dos]
ems=false
xms=true
umb=true
[autoexec]
CONF
		printf '%s\n' '@echo off' 'mount c /c' 'mount d /d' 'c:' 'set PATH=c:\windows;c:\utils' 'cd \windows' "win /3 $1"
	} > "$work/conf/run.conf"
	rm -f "$work/c/$2"
	if command -v dosbox-x > /dev/null; then
		(SDL_AUDIODRIVER=dummy dosbox-x -conf "$work/conf/run.conf" -nomenu -fastlaunch > /dev/null 2>&1 &)
	else
		docker rm -f "$name" > /dev/null 2>&1 || true
		docker run -d --name "$name" --user "$(id -u):$(id -g)" --network host -v /tmp/.X11-unix:/tmp/.X11-unix -e DISPLAY="$DISPLAY" \
			-e SDL_AUDIODRIVER=dummy -e HOME=/tmp -v "$work/c:/c" -v "$work/d:/d" -v "$work/conf:/conf" --entrypoint dosbox-x \
			"${DOSBOX_IMAGE:-liminal-installer-retro:local}" -conf /conf/run.conf -nomenu -fastlaunch > /dev/null
	fi
}
wait_exit() { # wait_exit <log in C:> <seconds>
	for _ in $(seq 1 "$2"); do
		grep -q '^exit' "$work/c/$1" 2> /dev/null && return 0
		sleep 1
	done
	return 1
}
stop() { docker rm -f "$name" > /dev/null 2>&1 || true; }
norm() { tr -d '\r' < "$1" | sed -e 's/^text: Space available: .*/text: Space available: N/' -e 's/Free: [0-9.]* [KMG]B/Free: N/' -e 's/^text: Setup found: .*/text: Setup found: N/' -e 's/^field: \(Port\|IRQ\|DMA\) = .*/field: \1 = N/' -e '/^radio: Sound card/,/^[^ ]/ s/^  (o)/  ( )/'; }

# The `pbd` line of the tree's WIN.INI, and the one Notepad owns before a run.
winini="$work/c/WINDOWS/WIN.INI"
pbd_line() { tr -d '\r' < "$winini" | grep -i '^pbd=' || true; }
seed_pbd() {
	sed -i '/^pbd=/Id' "$winini"
	[ -z "${NO_OWNER:-}" ] || return 0
	sed -i 's/^\[Extensions\]\r\?$/&\npbd=notepad.exe ^.pbd\r/I' "$winini"
}

pass=0 total=0
for ex in "${examples[@]}"; do
	"$liminst" build -C "examples/$ex" -t win16-x86 --version 1.0.0 -o "$work/out-$ex" --output files --win16-runtime "$work/exe/SETUP.EXE" > /dev/null 2>&1
	dir="C:\\$([ "$ex" = lt-fizz ] && echo LTFIZZ || echo PLANBOAR)"
	host="$work/c/$(basename "${dir//\\//}")"
	for run in $(seq 1 "$runs"); do
		total=$((total + 1))
		ok=1
		why=''
		[ "$ex" != planboard ] || seed_pbd
		cp "$work/out-$ex/win16/SETUP.EXE" "$work/out-$ex/win16/SETUP.LPK" "$work/d/"
		printf 'licence choose licence.accepted yes\n' > "$work/d/PLAY.TXT"
		: > "$work/d/PLAY2.TXT"
		pause=""
		[ -n "${SHOTS:-}" ] && [ "$run" = 1 ] && pause=" /PAUSE=2500"
		session "D:\\SETUP.EXE /LOG=C:\\S.LOG /PLAY=D:\\PLAY.TXT /DUMP=C:\\S.TXT$pause" S.LOG 240 || true
		if [ -n "$pause" ]; then
			# a screenshot of every page: the script holds each for 2.5 s
			mkdir -p "$SHOTS"
			for i in $(seq 1 14); do
				sleep 3
				[ -f "$work/c/S.LOG" ] && grep -q '^exit' "$work/c/S.LOG" && break
				import -window root "$SHOTS/w31-$ex-$(printf %02d "$i").png" 2> /dev/null || true
			done
		fi
		wait_exit S.LOG 240 || { ok=0; why+=' install-no-exit'; }
		if [ -n "$pause" ]; then
			sleep 4
			restore_pm
			import -window root -crop 800x600+320+280 +repage "$SHOTS/w31-$ex-group.png" 2> /dev/null || true
		fi
		stop
		[ "$(tail -n1 "$work/c/S.LOG" 2> /dev/null | tr -d '\r')" = "exit 0" ] || { ok=0; why+=" install-exit:$(tail -n1 "$work/c/S.LOG" 2> /dev/null | tr -d '\r')"; }
		diff <(norm "$work/c/S.TXT") <(norm "crates/li-retro-build/tests/golden/win16/$ex.pages.txt") > "$work/diff" 2>&1 || { ok=0; why+=' pages'; head -8 "$work/diff" >&2; }
		n=0
		if [ -f "$host/INSTALL.LIR" ]; then
			while read -r kind fname size _crc; do
				[ "$kind" = F ] || continue
				[ "$(stat -c %s "$host/$(echo "$fname" | tr '\\' '/')" 2> /dev/null || echo -1)" = "$size" ] || ok=0
				n=$((n + 1))
			done < <(tr -d '\r' < "$host/INSTALL.LIR")
		else
			ok=0
		fi
		grp=''
		if [ -f "$host/INSTALL.LIR" ]; then
			grp="$(tr -d '\r' < "$host/INSTALL.LIR" | sed -n 's/^P //p' | head -n1 | LC_ALL=C tr -cd '[:alnum:]' | cut -c1-8 | tr a-z A-Z)"
			[ -z "$grp" ] || grp="$grp.GRP"
		fi
		if [ -z "${PROGMAN_OPTIONAL:-}" ]; then
			grep -q '^progman: added' "$work/c/S.LOG" 2> /dev/null || { ok=0; why+=' progman-not-added'; }
			[ -n "$grp" ] || { ok=0; why+=' no-P-record'; }
			[ -z "$grp" ] || [ -f "$work/c/WINDOWS/$grp" ] || { ok=0; why+=" no-$grp"; }
			[ -z "$grp" ] || grep -qi "$grp" "$work/c/WINDOWS/PROGMAN.INI" || { ok=0; why+=' ini-has-no-group'; }
		fi
		if [ "$ex" = planboard ]; then
			[ "$(pbd_line)" = 'pbd=C:\PLANBOAR\PLANBRD.EXE ^.pbd' ] || { ok=0; why+=" winini-after-install:$(pbd_line)"; }
			record='A pbd|hq.liminal.planboard.pbd|C:\PLANBOAR\PLANBRD.EXE|notepad.exe ^.pbd'
			[ -z "${NO_OWNER:-}" ] || record='A pbd|hq.liminal.planboard.pbd|C:\PLANBOAR\PLANBRD.EXE'
			tr -d '\r' < "$host/INSTALL.LIR" | grep -qxF "$record" || { ok=0; why+=' no-A-record'; }
		fi
		if [ -n "${KEPT:-}" ]; then
			[ -f "$host/UNINST.EXE" ] && [ -f "$host/UNINST.DAT" ] || { ok=0; why+=' no-kept-uninstaller'; }
			grep -q '^progman: group: .*,2,' "$work/c/S.LOG" 2> /dev/null || { ok=0; why+=' no-uninstall-item'; }
			uninstall="$dir\\UNINST.EXE /U $dir"
		else
			uninstall="D:\\SETUP.EXE /U $dir"
		fi
		session "$uninstall /LOG=C:\\U.LOG /PLAY=D:\\PLAY2.TXT" U.LOG 240 || true
		wait_exit U.LOG 240 || { ok=0; why+=' uninstall-no-exit'; }
		if [ -n "$pause" ]; then
			sleep 4
			restore_pm
			import -window root -crop 800x600+320+280 +repage "$SHOTS/w31-$ex-removed.png" 2> /dev/null || true
		fi
		stop
		if [ -z "${PROGMAN_OPTIONAL:-}" ]; then
			grep -q '^progman: removed; group gone' "$work/c/U.LOG" 2> /dev/null || { ok=0; why+=' progman-not-removed'; }
			[ -z "$grp" ] || [ ! -f "$work/c/WINDOWS/$grp" ] || { ok=0; why+=" $grp-left"; }
			[ -z "$grp" ] || ! grep -qi "$grp" "$work/c/WINDOWS/PROGMAN.INI" || { ok=0; why+=' ini-still-has-group'; }
		fi
		after_uninstall='pbd=notepad.exe ^.pbd'
		[ -z "${NO_OWNER:-}" ] || after_uninstall=''
		[ "$ex" != planboard ] || [ "$(pbd_line)" = "$after_uninstall" ] || { ok=0; why+=" winini-after-uninstall:$(pbd_line)"; }
		[ "$(tail -n1 "$work/c/U.LOG" 2> /dev/null | tr -d '\r')" = "exit 0" ] || { ok=0; why+=" uninstall-exit:$(tail -n1 "$work/c/U.LOG" 2> /dev/null | tr -d '\r')"; }
		[ ! -e "$host" ] || { ok=0; why+=" folder-left($(ls "$host" 2> /dev/null | tr '\n' ' '))"; }
		pass=$((pass + ok))
		echo "e2e-w31: $ex run $run: $([ "$ok" = 1 ] && echo pass || echo "FAIL$why") ($n files; $(grep -h 'heap peak' "$work/c/S.LOG" 2> /dev/null | tr -d '\r'); $(grep -h '^progman:' "$work/c/S.LOG" 2> /dev/null | tr -d '\r' | tr '\n' ';'))"
		if [ -n "$pause" ]; then
			grep -h '^progman:' "$work/c/S.LOG" "$work/c/U.LOG" 2> /dev/null | tr -d '\r' > "$SHOTS/w31-$ex-progman.log" || true
			cp "$work/c/WINDOWS/PROGMAN.INI" "$SHOTS/w31-$ex-PROGMAN.INI.after-uninstall" 2> /dev/null || true
		fi
	done
done
echo "e2e-w31: $pass of $total runs passed"
[ "$pass" = "$total" ]
