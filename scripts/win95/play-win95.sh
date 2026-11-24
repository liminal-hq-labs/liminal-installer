#!/usr/bin/env bash
# Starts Windows 95 OSR2.5 in a real DOSBox-X window on your desktop, on a throwaway copy of a base image, for playing by hand.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: play-win95.sh [options] [PROGRAM.EXE...]
#   -b plain|ie4   which base to copy: plain Windows 95 OSR2.5 (default) or the one with Internet Explorer 4 and the Desktop Update
#   -d DIR         also put the contents of this host directory on D:
#   -q DIR         the Quill checkout to take QUILL32.EXE, QUILLDOS.EXE, QUILL.EXE and the sample documents from (default $QUILL_DIR,
#                  then ~/source/experiments/quill16)
#   -n             do not start anything at logon (the desktop only)
#   -a ARGS        arguments for the program that starts at logon (a document, say: D:\LETTER.RTF)
#   -s SCALE       window size in multiples of 640 by 480 (default 2 on a tall display, otherwise 1)
#   -o OUTPUT      the DOSBox-X video output: opengl (default, smooth scaling), openglnb (sharp pixels) or surface (no scaling at all)
#   -x             allow the container to reach the X server with xhost +SI:localuser:USER, taken back on exit (see below)
#   -N             run a DOSBox-X installed on the host (pacman, AUR or Flatpak's command on PATH) instead of the container; not tested here
#   -K             keep the working disk and the exchange disk afterwards (the run folder is printed)
#
# What happens. The chosen base is copied (sparsely) to $W95_HOME/runs/play-<time>/work.img; the base is only read. The programs on the
# command line, the -d directory and the Quill build outputs go on a FAT16 exchange disk that Windows 95 sees as D:. WIN.INI's run= line
# starts the first program on the command line at logon, or QUILL32.EXE when there is none (use -n to start nothing). DOSBox-X runs in the
# container $W95_PLAY_CONTAINER on your display (DISPLAY, X11 socket and your Xauthority file are handed in; no xhost is needed unless
# the container cannot connect and you pass -x). The window title says how to capture the pointer: click inside it, and press Ctrl+F10
# to let it go. When you close the window or Windows is shut down, the container is removed, the working disk is deleted, and whatever
# is on D: is copied to the run folder's out/ directory.
# Environment: W95_HOME (default $HOME/win95-testbed), W95_IMAGE, W95_PLAY_CONTAINER (default liminal-win95-play), QUILL_DIR.
set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

BASEKIND=plain
XDIR=""
QDIR="${QUILL_DIR:-$HOME/source/experiments/quill16}"
NOSTART=0
PARGS=""
SCALE=""
OUTPUT=opengl
XHOST=0
NATIVE=0
KEEP=0
CONTAINER="${W95_PLAY_CONTAINER:-liminal-win95-play}"
while getopts "b:d:q:na:s:o:xNK" o; do
	case "$o" in
	b) BASEKIND="$OPTARG" ;;
	d) XDIR="$OPTARG" ;;
	q) QDIR="$OPTARG" ;;
	n) NOSTART=1 ;;
	a) PARGS="$OPTARG" ;;
	s) SCALE="$OPTARG" ;;
	o) OUTPUT="$OPTARG" ;;
	x) XHOST=1 ;;
	N) NATIVE=1 ;;
	K) KEEP=1 ;;
	*) w95_die "bad option (see the header of play-win95.sh)" ;;
	esac
done
shift $((OPTIND - 1))

case "$BASEKIND" in
plain) BASE="$W95_HOME/win95-base.img" ;;
ie4) BASE="$W95_HOME/win95-ie4-base.img" ;;
*) w95_die "-b takes plain or ie4" ;;
esac
case "$OUTPUT" in surface | opengl | openglnb) ;; *) w95_die "-o takes surface, opengl or openglnb" ;; esac
[ -f "$BASE" ] || w95_die "no base image at $BASE; set W95_HOME to the folder that holds it, or run build-base.sh (and build-ie4.sh for -b ie4)"
[ -z "$XDIR" ] || [ -d "$XDIR" ] || w95_die "no such directory: $XDIR"
[ -n "${DISPLAY:-}" ] || w95_die "DISPLAY is not set; run this from a terminal on your desktop session"
for p in "$@"; do [ -f "$p" ] || w95_die "no such program: $p"; done
[ "$NATIVE" = 0 ] || w95_need dosbox-x
w95_need docker xdotool sfdisk mkfs.fat mcopy mdir truncate flock awk
w95_lock

# The display size picks the window scale: a 1280 by 960 window needs a tall screen.
if [ -z "$SCALE" ]; then
	SCALE=1
	geom="$(xdotool getdisplaygeometry 2> /dev/null || true)"
	[ "${geom##* }" -ge 1080 ] 2> /dev/null && SCALE=2
fi
case "$SCALE" in [1-4]) ;; *) w95_die "-s takes 1 to 4" ;; esac

RUN="$W95_HOME/runs/play-$(date +%Y%m%d-%H%M%S)"
STAGE="$RUN/stage"
OFF=$((W95_PART_START * 512))
mkdir -p "$STAGE" "$RUN/out"

# What goes on D:.
missing=()
if [ -d "$QDIR" ]; then
	for f in win9x/dist/QUILL32.EXE dos/dist/QUILLDOS.EXE win16/out/QUILL.EXE tests/docs/letter.rtf tests/docs/letter.qld tests/docs/plain.qld; do
		if [ -f "$QDIR/$f" ]; then cp "$QDIR/$f" "$STAGE/"; else missing+=("$f"); fi
	done
else
	missing+=(win9x/dist/QUILL32.EXE)
fi
[ -z "$XDIR" ] || cp -R "$XDIR"/. "$STAGE/"
for p in "$@"; do cp "$p" "$STAGE/"; done
if [ ! -f "$STAGE/QUILL32.EXE" ] && [ "$#" -eq 0 ] && [ "$NOSTART" = 0 ]; then
	w95_log "QUILL32.EXE is not built, so nothing will start at logon. Build it with: (cd $QDIR/win9x && ./build.sh), then run this again, or pass a program."
	NOSTART=1
fi

# What starts at logon.
if [ "$NOSTART" = 1 ]; then
	CMD=""
elif [ "$#" -gt 0 ]; then
	CMD="D:\\$(basename "$1")${PARGS:+ $PARGS}"
else
	CMD="D:\\QUILL32.EXE${PARGS:+ $PARGS}"
fi

# shellcheck disable=SC2329 # run by the EXIT trap
cleanup() {
	set +e
	docker rm -f "$CONTAINER" > /dev/null 2>&1
	if [ -f "$RUN/exchange.img" ]; then
		mcopy -s -n -m -Q -i "$RUN/exchange.img@@$OFF" ::/ "$RUN/out/" 2> /dev/null
		# Only what the guest added or changed is worth keeping.
		for f in "$RUN"/out/*; do
			[ -f "$f" ] && [ -f "$STAGE/$(basename "$f")" ] && cmp -s "$f" "$STAGE/$(basename "$f")" && rm -f "$f"
		done
	fi
	[ "$XHOST_ADDED" = 1 ] && xhost -SI:localuser:"$(id -un)" > /dev/null 2>&1
	if [ "$KEEP" = 0 ]; then rm -f "$RUN/work.img" "$RUN/exchange.img"; fi
	rm -rf "$STAGE"
	if [ -n "$(ls -A "$RUN/out" 2> /dev/null)" ]; then
		w95_log "stopped. Files left on D: are in $RUN/out"
	elif [ "$KEEP" = 0 ] && [ "$FAILED" = 0 ]; then
		rm -rf "$RUN"
		w95_log "stopped. Nothing was kept"
	fi
}
XHOST_ADDED=0
FAILED=0
NATIVE_PID=
trap cleanup EXIT
trap 'docker rm -f "$CONTAINER" > /dev/null 2>&1; [ -z "$NATIVE_PID" ] || kill "$NATIVE_PID" 2> /dev/null; exit 130' INT TERM HUP

w95_log "copying the $BASEKIND base (the base itself is never written)"
nice -n 19 ionice -c3 cp --sparse=always "$BASE" "$RUN/work.img"
chmod u+w "$RUN/work.img"
mcopy -n -i "$RUN/work.img@@$OFF" ::/WINDOWS/WIN.INI "$RUN/WIN.INI"
W95_CMD="$CMD" awk 'BEGIN { IGNORECASE = 1; cmd = ENVIRON["W95_CMD"] } /^[ \t]*run[ \t]*=/ { printf "run=%s\r\n", cmd; done = 1; next } { print } END { if (!done) exit 3 }' "$RUN/WIN.INI" > "$RUN/WIN.INI.new" || w95_die "WIN.INI has no run= line"
mcopy -o -i "$RUN/work.img@@$OFF" "$RUN/WIN.INI.new" ::/WINDOWS/WIN.INI
rm -f "$RUN/WIN.INI" "$RUN/WIN.INI.new"
w95_make_dimage "$RUN/exchange.img" "$STAGE"

# The DOSBox-X profile: a normal window, the pointer captured on a click, no sound.
INRUN="/w/${RUN#"$W95_HOME"/}"
[ "$NATIVE" = 0 ] || INRUN="$RUN"
{
	printf '[sdl]\noutput=%s\nfullscreen=false\nwindowresolution=%sx%s\nautolock=true\nmouse_emulation=locked\n\n' "$OUTPUT" $((640 * SCALE)) $((480 * SCALE))
	printf '[dosbox]\ntitle=Windows 95 - click to capture the mouse, Ctrl+F10 to release it\nmachine=svga_s3\nmemsize=64\n\n'
	printf '[cpu]\ncore=normal\ncputype=pentium\ncycles=max\n\n'
	printf '[sblaster]\nsbtype=none\n\n[gus]\ngus=false\n\n[dos]\nver=7.1\n\n'
	printf '[autoexec]\n@echo off\nimgmount c %s/work.img -size %s\nimgmount 3 %s/exchange.img -t hdd -fs none -size %s\nboot -l c\n' "$INRUN" "$W95_C_GEOM" "$INRUN" "$W95_D_GEOM"
} > "$RUN/play.conf"

[ "$NATIVE" = 1 ] || w95_image_ready
docker rm -f "$CONTAINER" > /dev/null 2>&1 || true

# The X server: the Xauthority cookie goes in read-only, and the hostname matches the host's so the cookie is found.
XARGS=(-e "DISPLAY=$DISPLAY" -e SDL_VIDEODRIVER=x11 -e LIBGL_ALWAYS_SOFTWARE=1 -v /tmp/.X11-unix:/tmp/.X11-unix --hostname "$(uname -n)")
XA="${XAUTHORITY:-$HOME/.Xauthority}"
[ ! -f "$XA" ] || XARGS+=(-v "$XA:/tmp/.Xauthority:ro" -e XAUTHORITY=/tmp/.Xauthority)
if [ "$XHOST" = 1 ]; then
	if xhost 2> /dev/null | grep -qi "SI:localuser:$(id -un)\$"; then
		w95_log "xhost already allows $(id -un); leaving it alone"
	else
		xhost +SI:localuser:"$(id -un)" > /dev/null
		XHOST_ADDED=1
		w95_log "xhost: allowed local user $(id -un) for this session; it is taken back on exit"
	fi
fi

cat >&2 << EOF

  Windows 95 ($BASEKIND base) is starting in a ${SCALE}x window. ${CMD:+It will run: $CMD}
    Click inside the window to capture the mouse; press Ctrl+F10 to release it.
    D: holds the Quill files and the sample documents.
    Close Windows from Start > Shut Down, then close the window (or press Ctrl+C here). Nothing you do is kept.
    Stop it from another terminal: docker rm -f $CONTAINER
    No sound: the emulated machine has no sound card.

EOF
rc=0
if [ "$NATIVE" = 1 ]; then
	SDL_AUDIODRIVER=dummy dosbox-x -conf "$INRUN/play.conf" -fastlaunch > "$RUN/dosbox.log" 2>&1 &
	NATIVE_PID=$!
else
	docker run --rm --name "$CONTAINER" --user "$(id -u):$(id -g)" --network host \
		"${XARGS[@]}" -e SDL_AUDIODRIVER=dummy -e HOME=/tmp -v "$W95_HOME:/w" \
		"$W95_IMAGE" dosbox-x -conf "$INRUN/play.conf" -fastlaunch > "$RUN/dosbox.log" 2>&1 &
fi
wait $! || rc=$?
# 130, 137 and 143 are the window closed with a signal, docker rm or Ctrl+C: a normal way to stop.
case "$rc" in 130 | 137 | 143) rc=0 ;; esac
if [ "$rc" -ne 0 ]; then
	FAILED=1
	tail -n 15 "$RUN/dosbox.log" >&2
	w95_log "DOSBox-X exited with status $rc (if it could not open the display, try again with -x; the log is $RUN/dosbox.log)"
fi
exit "$rc"
