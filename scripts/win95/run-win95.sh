#!/usr/bin/env bash
# Runs a Windows program on a throwaway copy of the Windows 95 base image under DOSBox-X, headless, and brings the results back.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: run-win95.sh [options] PROGRAM.EXE [ARGS...]
#   -w SECONDS   how long to let the program run after the desktop is up (default 30)
#   -i SECONDS   seconds between screenshots (default 5)
#   -k KEYS      keys to press once, after the wait, space separated xdotool names, e.g. "Return alt+F4"; repeatable
#   -d DIR       also copy the contents of this host directory to the exchange disk (the program is copied there too)
#   -K           keep the working images (the run folder keeps work.img and exchange.img)
#   -g COMMAND   run a program already inside Windows 95 (for example 'C:\WINDOWS\WINVER.EXE') instead of a host PROGRAM.EXE
#   -b plain|ie4 which base to copy: plain Windows 95 OSR2.5 (default) or the one with Internet Explorer 4 and the Desktop Update on top
#   -m SCRIPT    a mouse script run once the desktop is up, e.g. "click 24 466; wait 1; shot start.png; dblclick 50 60" (see mouse.sh)
#   -n NAME      a label for the run folder (default: the program name)
#
# What happens: the base image is copied to $W95_HOME/runs/<time>-<name>/work.img (a sparse copy; the base is never opened for writing). The
# program, and the -d directory if given, go on a FAT16 exchange disk that Windows 95 sees as D:. WIN.INI gets `run=D:\PROGRAM.EXE ARGS`,
# so the shell starts it at logon. After the wait, screenshots land in the run folder as shot-NN.png, the guest is shut down from the Start
# menu so its disk caches flush, and everything on D: is copied back to the run folder's out/ directory (a program that writes files next
# to itself returns them there). The folder's path is printed last. Exit status 0 means the guest reached the desktop and shut down.
# Environment: W95_HOME (default $HOME/win95-testbed), W95_DISPLAY, W95_IMAGE; see lib.sh.
set -euo pipefail
# shellcheck source=mouse.sh
. "$(dirname "${BASH_SOURCE[0]}")/mouse.sh"

WAIT=30
EVERY=5
KEEP=0
XDIR=""
NAME=""
KEYS=()
GUEST=""
BASEKIND=plain
MOUSE=""
while getopts "w:i:k:d:Kn:g:b:m:" o; do
	case "$o" in
	w) WAIT="$OPTARG" ;;
	i) EVERY="$OPTARG" ;;
	k) KEYS+=("$OPTARG") ;;
	d) XDIR="$OPTARG" ;;
	K) KEEP=1 ;;
	n) NAME="$OPTARG" ;;
	g) GUEST="$OPTARG" ;;
	b) BASEKIND="$OPTARG" ;;
	m) MOUSE="$OPTARG" ;;
	*) w95_die "bad option (see the header of run-win95.sh)" ;;
	esac
done
shift $((OPTIND - 1))
if [ -n "$GUEST" ]; then
	PROG="$GUEST"
else
	PROG="${1:?usage: run-win95.sh [options] PROGRAM.EXE [ARGS...]}"
	shift
	[ -f "$PROG" ] || w95_die "no such program: $PROG"
fi
[ -z "$XDIR" ] || [ -d "$XDIR" ] || w95_die "no such directory: $XDIR"
case "$BASEKIND" in
plain) BASE="$W95_HOME/win95-base.img" ;;
ie4) BASE="$W95_HOME/win95-ie4-base.img" ;;
*) w95_die "-b takes plain or ie4" ;;
esac
[ -f "$BASE" ] || w95_die "no base image at $BASE; run build-base.sh (and build-ie4.sh for -b ie4) first"
w95_need docker Xvfb xdotool import convert sfdisk mkfs.fat mcopy mdir truncate flock md5sum
w95_lock

PNAME="$(basename "${PROG//\\//}")"
RUN="$W95_HOME/runs/$(date +%Y%m%d-%H%M%S)-${NAME:-${PNAME%.*}}"
mkdir -p "$RUN/stage" "$RUN/out"
trap 'w95_stop; w95_xvfb_stop' EXIT
OFF=$((W95_PART_START * 512))

# The throwaway system disk.
cp --sparse=always "$BASE" "$RUN/work.img"
chmod u+w "$RUN/work.img"
[ -z "$XDIR" ] || cp -R "$XDIR"/. "$RUN/stage/"
[ -n "$GUEST" ] || cp "$PROG" "$RUN/stage/$PNAME"

# The program starts from WIN.INI's run= line at logon.
mcopy -n -i "$RUN/work.img@@$OFF" ::/WINDOWS/WIN.INI "$RUN/WIN.INI"
if [ -n "$GUEST" ]; then CMD="$GUEST"; else CMD="D:\\$PNAME${*:+ $*}"; fi
W95_CMD="$CMD" awk 'BEGIN { IGNORECASE = 1; cmd = ENVIRON["W95_CMD"] } /^[ \t]*run[ \t]*=/ { printf "run=%s\r\n", cmd; done = 1; next } { print } END { if (!done) exit 3 }' "$RUN/WIN.INI" > "$RUN/WIN.INI.new" || w95_die "WIN.INI has no run= line"
mcopy -o -i "$RUN/work.img@@$OFF" "$RUN/WIN.INI.new" ::/WINDOWS/WIN.INI
rm -f "$RUN/WIN.INI" "$RUN/WIN.INI.new"

w95_make_dimage "$RUN/exchange.img" "$RUN/stage"
w95_conf "$RUN/run.conf" "/w/${RUN#"$W95_HOME"/}/work.img" "/w/${RUN#"$W95_HOME"/}/exchange.img"
w95_log "run folder: $RUN"
T0=$(date +%s)
w95_start "$RUN/run.conf"
RC=0
if w95_wait_desktop 300; then
	w95_log "desktop after $(($(date +%s) - T0)) seconds; running $CMD for ${WAIT}s"
	n=0
	if [ -n "$MOUSE" ]; then
		sleep 5
		w95_mouse_capture
		W95_SHOT_DIR="$RUN" w95_mouse_script "$MOUSE"
	fi
	end=$(($(date +%s) + WAIT))
	while [ "$(date +%s)" -lt "$end" ]; do
		n=$((n + 1))
		w95_shot "$(printf '%s/shot-%02d.png' "$RUN" "$n")"
		sleep "$EVERY"
	done
	for k in "${KEYS[@]:-}"; do
		[ -n "$k" ] || continue
		# shellcheck disable=SC2086 # the keys are separate words on purpose
		w95_key $k
		sleep 3
		n=$((n + 1))
		w95_shot "$(printf '%s/shot-%02d.png' "$RUN" "$n")"
	done
	n=$((n + 1))
	w95_shot "$(printf '%s/shot-%02d.png' "$RUN" "$n")"
	cp "$(printf '%s/shot-%02d.png' "$RUN" "$n")" "$RUN/final.png"
	w95_shutdown
else
	w95_shot "$RUN/no-desktop.png"
	w95_log "the guest never reached the desktop (see $RUN/no-desktop.png)"
	w95_stop
	RC=1
fi

mcopy -s -n -m -Q -i "$RUN/exchange.img@@$OFF" ::/ "$RUN/out/" 2> /dev/null || true
if [ "$KEEP" = 0 ]; then
	rm -f "$RUN/work.img" "$RUN/exchange.img"
fi
rm -rf "$RUN/stage"
w95_log "finished in $(($(date +%s) - T0)) seconds"
echo "$RUN"
exit "$RC"
