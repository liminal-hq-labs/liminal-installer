#!/usr/bin/env bash
# Installs Internet Explorer 4.0 and the Windows Desktop Update on a copy of the plain Windows 95 base, and freezes the result as win95-ie4-base.img.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: build-ie4.sh [--force]
#
# Needs $W95_HOME/win95-base.img from build-base.sh. IE4SETUP.EXE and its cabinets are already on that disk, in C:\WIN95, copied from the CD, so
# neither the ISO nor the product key is involved. A Standard Installation with the Windows Desktop Update (the IE4 shell, shell32 4.71) is
# chosen; the wizard is driven by keys. The Active Desktop and the channel bar are switched off by Explorer policy (NoActiveDesktop,
# NoChannelUI) through a .reg file merged at the end, and no connection is made. Stages: install, verify; markers in $W95_HOME/state.
# Success is judged by the version resources of COMCTL32.DLL (4.71 or newer expected; this CD gives 4.72, Internet Explorer 4.01 SP2), SHDOCVW.DLL and WININET.DLL, and by a boot to the desktop.
set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

FORCE=0
[ "${1:-}" = "--force" ] && FORCE=1
BASE="$W95_HOME/win95-base.img"
IE4="$W95_HOME/win95-ie4-base.img"
WORK="$W95_HOME/ie4-install.img"
OFF=$((W95_PART_START * 512))
SHOTS="$W95_HOME/shots"
STATE="$W95_HOME/state"
[ -f "$BASE" ] || w95_die "no plain base at $BASE; run build-base.sh first"
w95_need docker Xvfb xdotool import convert compare mcopy mdir flock python3
w95_lock
trap 'w95_stop; w95_xvfb_stop' EXIT

if [ "$FORCE" = 1 ]; then
	rm -f "$STATE/ie4-install.done" "$STATE/ie4-verify.done"
	[ -f "$IE4" ] && chmod u+w "$IE4" && rm -f "$IE4"
fi
if [ -f "$IE4" ] && [ -f "$STATE/ie4-verify.done" ]; then
	w95_log "the IE4 base is already built: $IE4"
	exit 0
fi

# w95_versions IMAGE: prints "FILE version" for the three DLLs the IE4 base is judged by.
w95_versions() {
	local img="$1" f tmp="$W95_HOME/scratch/ver"
	rm -rf "$tmp"
	mkdir -p "$tmp"
	for f in COMCTL32.DLL SHDOCVW.DLL WININET.DLL SHELL32.DLL; do
		mcopy -n -i "$img@@$OFF" "::/WINDOWS/SYSTEM/$f" "$tmp/$f" 2> /dev/null || continue
		python3 -I - "$tmp/$f" << 'PY'
import os, re, sys
p = sys.argv[1]
d = open(p, "rb").read()
m = re.search(rb"F\x00i\x00l\x00e\x00V\x00e\x00r\x00s\x00i\x00o\x00n\x00(?:\x00)+((?:[0-9.]\x00)+)", d)
print(os.path.basename(p), m.group(1).decode("utf-16le") if m else "unknown")
PY
		rm -f "$tmp/$f"
	done
	rmdir "$tmp"
}

# w95_page [KEYS]: waits for the wizard page to settle, then presses the keys (default alt+n).
w95_page() {
	w95_wait_stable 4 60 || true
	w95_key "${@:-alt+n}"
}

stage_install() {
	w95_log "install: copying the plain base and starting Internet Explorer 4 Setup"
	cp --sparse=always "$BASE" "$WORK"
	chmod u+w "$WORK"
	printf 'REGEDIT4\r\n\r\n[HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer]\r\n"NoActiveDesktop"=dword:00000001\r\n"NoActiveDesktopChanges"=dword:00000001\r\n"NoChannelUI"=dword:00000001\r\n' > "$W95_HOME/scratch/NOAD.REG"
	mcopy -o -i "$WORK@@$OFF" "$W95_HOME/scratch/NOAD.REG" ::/NOAD.REG
	w95_conf "$W95_HOME/conf/ie4.conf" "/w/ie4-install.img"
	local t0
	t0=$(date +%s)
	w95_start "$W95_HOME/conf/ie4.conf"
	w95_wait_desktop 300 || w95_die "the plain base did not reach the desktop"
	sleep 10
	w95_wait_stable 5 60 || true
	w95_key ctrl+Escape
	sleep 2
	w95_key r
	sleep 3
	w95_type 'C:\WIN95\IE4SETUP.EXE'
	w95_key Return
	w95_page Return
	w95_page alt+a
	w95_shot "$SHOTS/ie4-licence.png"
	w95_page alt+n
	w95_page alt+n
	w95_shot "$SHOTS/ie4-option.png"
	w95_page alt+n
	w95_shot "$SHOTS/ie4-desktop-update.png"
	w95_page alt+n
	w95_page alt+n
	w95_log "install: copying and registering components"
	# The progress page is never still for long; a still screen after it is the "finished" box.
	sleep 60
	w95_wait_stable 25 1500 || w95_die "IE4 Setup never finished (see $SHOTS/ie4-progress.png)"
	w95_shot "$SHOTS/ie4-finished.png"
	w95_key Return
	w95_log "install: restarting for the first run"
	sleep 120
	w95_wait_stable 20 600 || w95_die "the first run of IE4 never settled"
	w95_shot "$SHOTS/ie4-first-run.png"
	w95_key ctrl+Escape
	sleep 2
	w95_key r
	sleep 3
	w95_type 'regedit /s C:\NOAD.REG'
	w95_key Return
	sleep 8
	w95_shutdown
	w95_log "install: done in $(($(date +%s) - t0)) seconds"
	echo "$(($(date +%s) - t0))" > "$STATE/ie4.seconds"
	touch "$STATE/ie4-install.done"
}

stage_verify() {
	w95_log "verify: version resources"
	local v
	v="$(w95_versions "$WORK")"
	echo "$v" | sed 's/^/	/' >&2
	echo "$v" | grep -q '^COMCTL32.DLL 4\.7[1-9]' || w95_die "COMCTL32.DLL is older than 4.71"
	echo "$v" | grep -q '^SHDOCVW.DLL ' || w95_die "SHDOCVW.DLL is missing"
	echo "$v" | grep -q '^WININET.DLL ' || w95_die "WININET.DLL is missing"
	echo "$v" > "$W95_HOME/win95-ie4-versions.txt"
	w95_log "verify: booting a copy of the installed disk"
	local copy="$W95_HOME/ie4-verify.img"
	cp --sparse=always "$WORK" "$copy"
	w95_conf "$W95_HOME/conf/ie4.conf" "/w/ie4-verify.img"
	w95_start "$W95_HOME/conf/ie4.conf"
	w95_wait_desktop 300 || {
		w95_shot "$SHOTS/ie4-verify-nodesktop.png"
		w95_die "the IE4 system did not reach a plain desktop (see $SHOTS/ie4-verify-nodesktop.png)"
	}
	sleep 15
	w95_wait_desktop 10 || w95_die "the desktop did not stay up"
	w95_shot "$SHOTS/ie4-desktop.png"
	w95_shutdown
	rm -f "$copy"
	mcopy -n -i "$WORK@@$OFF" ::/NOAD.REG /dev/null 2> /dev/null || true
	mv "$WORK" "$IE4"
	chmod 444 "$IE4"
	touch "$STATE/ie4-verify.done"
}

[ -f "$STATE/ie4-install.done" ] || stage_install
[ -f "$STATE/ie4-verify.done" ] || stage_verify
w95_log "IE4 base ready (read-only): $IE4"
