#!/usr/bin/env bash
# Drives Windows 95's mouse from the host: DOSBox-X's emulated PS/2 mouse, fed by xdotool under the test bed's Xvfb.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: mouse.sh COMMAND [ARGS...]
#   capture                  grab the pointer in DOSBox-X (ctrl+F10 toggles the grab, so this tests for it first); run once per guest
#   home                     push the pointer into the top-left corner, the one place whose position is known
#   move X Y                 move to guest pixel X Y (0..639, 0..479)
#   click [X Y]              left click, after moving there if a position is given
#   rclick [X Y]             right click
#   dblclick [X Y]           double click
#   drag X1 Y1 X2 Y2        press at the first point, move, release at the second
#   shot FILE                a screenshot (inside script lists; relative names go to the run folder under run-win95.sh)
#   key NAME...              keyboard keys (inside script lists), for example Escape
#   wait SECONDS             pause (inside script lists)
#   script "click 100 200; dblclick 50 60; drag 10 10 200 100"    a list of the above, separated by semicolons
# Also sourced by run-win95.sh for its -m option. Environment: W95_DISPLAY (default :95), W95_HOME; see lib.sh.
#
# Why it is built this way. DOSBox-X hands a PS/2 mouse only relative motion, and only while it has grabbed the pointer, so the
# host pointer cannot simply be placed at a guest position. Windows 95 also accelerates fast motion: steps of one pixel arrive
# one for one (checked against screenshots, in x and y, at several targets), while steps of two or three come out about 1.4 and
# 2 times too far. So the pointer is homed against the top-left corner (a long run of large steps clamps there) and then walked to
# the target in one-pixel steps; the position it should be at is kept in $W95_HOME/run/mouse.pos so later moves walk only the difference.
# The guest screen is the 640 by 480 window DOSBox-X opens, so a guest pixel is a window pixel; no scaling is involved.

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

W95_MOUSE_POS="$W95_HOME/run/mouse.pos"
W95_MOUSE_GAP="${W95_MOUSE_GAP:-0.02}"

w95_mouse_xdo() { DISPLAY="$W95_DISPLAY" xdotool "$@"; }

# w95_mouse_home: homes the pointer in the corner and records 0 0.
w95_mouse_home() {
	local cmd=""
	for _ in $(seq 1 24); do cmd="$cmd mousemove_relative -- -40 -40 sleep 0.02"; done
	# shellcheck disable=SC2086 # the command words are separate on purpose
	w95_mouse_xdo $cmd
	sleep 0.3
	mkdir -p "$(dirname "$W95_MOUSE_POS")"
	echo "0 0" > "$W95_MOUSE_POS"
}

# w95_mouse_walk DX DY: relative motion in one-pixel steps.
w95_mouse_walk() {
	local dx="$1" dy="$2" cmd="" sx sy
	while [ "$dx" -ne 0 ] || [ "$dy" -ne 0 ]; do
		sx=0
		sy=0
		[ "$dx" -gt 0 ] && sx=1 && dx=$((dx - 1))
		[ "$dx" -lt 0 ] && sx=-1 && dx=$((dx + 1))
		[ "$dy" -gt 0 ] && sy=1 && dy=$((dy - 1))
		[ "$dy" -lt 0 ] && sy=-1 && dy=$((dy + 1))
		cmd="$cmd mousemove_relative -- $sx $sy sleep $W95_MOUSE_GAP"
	done
	# shellcheck disable=SC2086
	[ -z "$cmd" ] || w95_mouse_xdo $cmd
}

# w95_mouse_move X Y
w95_mouse_move() {
	local x="$1" y="$2" cx cy
	[ "$x" -ge 0 ] && [ "$x" -le 639 ] && [ "$y" -ge 0 ] && [ "$y" -le 479 ] || w95_die "mouse position $x $y is outside the 640x480 guest screen"
	[ -f "$W95_MOUSE_POS" ] || w95_mouse_home
	IFS=" " read -r cx cy < "$W95_MOUSE_POS"
	w95_mouse_walk $((x - cx)) $((y - cy))
	echo "$x $y" > "$W95_MOUSE_POS"
	sleep 0.15
}

# w95_mouse_capture: makes sure DOSBox-X has grabbed the pointer, by moving it and watching the screen.
w95_mouse_capture() {
	local a="$W95_HOME/run/mouse-a.png" b="$W95_HOME/run/mouse-b.png"
	for _ in 1 2 3; do
		w95_shot "$a"
		w95_mouse_walk 40 40
		sleep 0.3
		w95_shot "$b"
		w95_mouse_walk -40 -40
		if [ "$(w95_changed "$a" "$b")" -gt 30 ]; then
			w95_mouse_home
			return 0
		fi
		w95_key ctrl+F10
		sleep 0.5
	done
	w95_die "could not grab the pointer in DOSBox-X"
}

w95_mouse_click() {
	[ "$#" -ge 2 ] && w95_mouse_move "$1" "$2"
	w95_mouse_xdo click 1
	sleep 0.3
}
w95_mouse_rclick() {
	[ "$#" -ge 2 ] && w95_mouse_move "$1" "$2"
	w95_mouse_xdo click 3
	sleep 0.3
}
w95_mouse_dblclick() {
	[ "$#" -ge 2 ] && w95_mouse_move "$1" "$2"
	w95_mouse_xdo click --repeat 2 --delay 90 1
	sleep 0.5
}
w95_mouse_drag() {
	w95_mouse_move "$1" "$2"
	w95_mouse_xdo mousedown 1
	sleep 0.2
	w95_mouse_move "$3" "$4"
	sleep 0.2
	w95_mouse_xdo mouseup 1
	sleep 0.3
}

# w95_mouse_script "click 1 2; dblclick 3 4": runs the commands in order.
w95_mouse_script() {
	local list="$1" item cmd
	local -a words items
	IFS=';' read -r -a items <<< "$list"
	for item in "${items[@]}"; do
		IFS=' ' read -r -a words <<< "$item"
		[ "${#words[@]}" -gt 0 ] || continue
		cmd="${words[0]}"
		case "$cmd" in
		capture | home) "w95_mouse_$cmd" ;;
		move | click | rclick | dblclick | drag) "w95_mouse_$cmd" "${words[@]:1}" ;;
		key) w95_key "${words[@]:1}" ;;
		wait) sleep "${words[1]:-1}" ;;
		shot) w95_shot "${W95_SHOT_DIR:+$W95_SHOT_DIR/}${words[1]:?shot needs a file name}" ;;
		*) w95_die "unknown mouse command: $cmd" ;;
		esac
	done
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
	set -euo pipefail
	w95_need xdotool import compare
	cmd="${1:?usage: mouse.sh capture|home|move|click|rclick|dblclick|drag|script ...}"
	shift
	case "$cmd" in
	script) w95_mouse_script "${1:?script needs a list}" ;;
	*) w95_mouse_script "$cmd $*" ;;
	esac
fi
