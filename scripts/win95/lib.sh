#!/usr/bin/env bash
# Shared helpers for the Windows 95 test bed: paths, the DOSBox-X container, Xvfb, screenshots, keys and image building.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Sourced by build-base.sh, run-win95.sh, screenshot.sh and type-keys.sh. Environment:
#   W95_HOME       where images, logs and runs live (default $HOME/win95-testbed); never inside the repository
#   W95_DISPLAY    the Xvfb display (default :95)
#   W95_IMAGE      the DOSBox-X container image (default liminal-win95-dosbox-x:local, built from dosbox-x.Dockerfile if missing)
#   W95_CONTAINER  the container name (default liminal-win95-dbx)

# shellcheck disable=SC2034 # the geometry constants are used by the scripts that source this file
W95_SCRIPTS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
W95_HOME="${W95_HOME:-$HOME/win95-testbed}"
W95_DISPLAY="${W95_DISPLAY:-:95}"
W95_IMAGE="${W95_IMAGE:-liminal-win95-dosbox-x:local}"
W95_CONTAINER="${W95_CONTAINER:-liminal-win95-dbx}"

# The system disk: 130 cylinders, 255 heads, 63 sectors of 512 bytes (about 1 GB), one FAT32 partition starting at sector 63.
W95_C_GEOM="512,63,255,130"
W95_C_SECTORS=2088450
W95_PART_START=63
# The exchange disk (D:): 64 cylinders, 16 heads, 63 sectors, one FAT16 partition.
W95_D_GEOM="512,63,16,64"
W95_D_SECTORS=64512

w95_log() { printf '[win95 %s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
w95_die() {
	w95_log "error: $*"
	exit 1
}

w95_need() {
	local t
	for t in "$@"; do
		command -v "$t" > /dev/null 2>&1 || w95_die "missing host tool: $t"
	done
}

w95_dirs() { mkdir -p "$W95_HOME"/{logs,shots,state,run,runs,scratch,conf}; }

# A lock so two scripts never drive the same display and container.
w95_lock() {
	w95_dirs
	exec 9> "$W95_HOME/run/lock"
	flock -n 9 || w95_die "another Windows 95 test bed script is running (lock: $W95_HOME/run/lock)"
}

w95_image_ready() {
	if ! docker image inspect "$W95_IMAGE" > /dev/null 2>&1; then
		w95_log "building $W95_IMAGE"
		docker build -q -t "$W95_IMAGE" -f "$W95_SCRIPTS/dosbox-x.Dockerfile" "$W95_SCRIPTS" > /dev/null
	fi
}

w95_xvfb() {
	local num="${W95_DISPLAY#:}"
	if [ ! -S "/tmp/.X11-unix/X$num" ]; then
		Xvfb "$W95_DISPLAY" -screen 0 1024x768x24 > "$W95_HOME/logs/xvfb.log" 2>&1 &
		echo $! > "$W95_HOME/run/xvfb.pid"
		sleep 1
	fi
}

w95_xvfb_stop() {
	if [ -f "$W95_HOME/run/xvfb.pid" ]; then
		local pid
		pid="$(cat "$W95_HOME/run/xvfb.pid")"
		kill "$pid" 2> /dev/null || true
		rm -f "$W95_HOME/run/xvfb.pid"
	fi
}

# w95_conf FILE C_IMAGE [D_IMAGE]: a DOSBox-X profile (paths as seen inside the container, where W95_HOME is /w).
w95_conf() {
	local f="$1" c="$2" d="${3:-}"
	{
		printf '[sdl]\noutput=surface\nautolock=false\nmouse_emulation=always\n\n'
		printf '[dosbox]\nmachine=svga_s3\nmemsize=64\n\n'
		printf '[cpu]\ncore=normal\ncputype=pentium\ncycles=max\n\n'
		printf '[sblaster]\nsbtype=none\n\n[gus]\ngus=false\n\n'
		printf '[dos]\nver=7.1\n\n'
		printf '[autoexec]\n@echo off\n'
		printf 'imgmount c %s -size %s\n' "$c" "$W95_C_GEOM"
		[ -n "$d" ] && printf 'imgmount 3 %s -t hdd -fs none -size %s\n' "$d" "$W95_D_GEOM"
		printf 'boot -l c\n'
	} > "$f"
}

# w95_floppy_conf FILE C_IMAGE FLOPPY: boots a floppy image with the system disk attached (used to write the boot code).
w95_floppy_conf() {
	{
		printf '[sdl]\noutput=surface\n\n[dosbox]\nmachine=svga_s3\nmemsize=64\n\n[cpu]\ncore=normal\ncputype=pentium\ncycles=max\n\n'
		printf '[autoexec]\n@echo off\nimgmount 0 %s -t floppy\nimgmount 2 %s -t hdd -fs none -size %s\nboot -l a\n' "$3" "$2" "$W95_C_GEOM"
	} > "$1"
}

# w95_start CONF_IN_HOME: starts DOSBox-X in the container on the test bed display; CONF is a path under W95_HOME.
w95_start() {
	local conf="$1"
	w95_image_ready
	w95_xvfb
	docker rm -f "$W95_CONTAINER" > /dev/null 2>&1 || true
	docker run -d --name "$W95_CONTAINER" --user "$(id -u):$(id -g)" --network host \
		-v /tmp/.X11-unix:/tmp/.X11-unix -e DISPLAY="$W95_DISPLAY" -e SDL_AUDIODRIVER=dummy -e HOME=/tmp \
		-v "$W95_HOME:/w" "$W95_IMAGE" nice -n 19 dosbox-x -conf "/w/${conf#"$W95_HOME"/}" -nomenu -fastlaunch > /dev/null
	sleep 3
}

w95_stop() { docker rm -f "$W95_CONTAINER" > /dev/null 2>&1 || true; }
w95_running() { [ "$(docker inspect -f '{{.State.Running}}' "$W95_CONTAINER" 2> /dev/null)" = true ]; }

# w95_shot FILE: the DOSBox-X window (or the whole display if the window is not found) as a PNG.
w95_shot() {
	local id
	id="$(DISPLAY="$W95_DISPLAY" xdotool search --onlyvisible --class dosbox 2> /dev/null | head -1 || true)"
	if [ -n "$id" ]; then
		DISPLAY="$W95_DISPLAY" timeout 20 import -window "$id" "$1" 2> /dev/null || DISPLAY="$W95_DISPLAY" timeout 20 import -window root "$1"
	else
		DISPLAY="$W95_DISPLAY" timeout 20 import -window root "$1"
	fi
}

# w95_type TEXT: types TEXT into the guest. w95_type_stdin takes the text from stdin instead, so a secret never appears in a process list.
w95_type() { DISPLAY="$W95_DISPLAY" xdotool type --delay 60 -- "$1"; }
w95_type_stdin() { DISPLAY="$W95_DISPLAY" xdotool type --delay 90 --file /dev/stdin; }
# w95_key KEY...: presses xdotool key names (Return, alt+y, ctrl+Escape).
w95_key() {
	local k
	for k in "$@"; do
		DISPLAY="$W95_DISPLAY" xdotool key "$k"
		sleep 0.4
	done
}

# w95_changed A.png B.png: the number of pixels that differ; a blinking caret is a few dozen, a moving progress bar more.
w95_changed() {
	local n
	n="$(compare -metric AE -fuzz 3% "$1" "$2" null: 2>&1 || true)"
	n="${n%% *}"
	case "$n" in
	'' | *[!0-9]*) echo 999999 ;;
	*) echo "$n" ;;
	esac
}

# w95_hash: a stand-in for a checksum of the screen; prints the path of a fresh screenshot (see w95_changed).
w95_hash() {
	local t="$W95_HOME/run/cur.png"
	w95_shot "$t" && echo "$t"
}

# w95_wait_stable SECONDS TIMEOUT: returns once the screen has not changed (beyond a caret) for SECONDS, or 1 after TIMEOUT.
w95_wait_stable() {
	local need="$1" limit="$2" start since
	local prev="$W95_HOME/run/prev.png" cur="$W95_HOME/run/cur.png"
	start=$(date +%s)
	since=$start
	rm -f "$prev"
	while [ $(($(date +%s) - start)) -lt "$limit" ]; do
		w95_running || w95_die "DOSBox-X exited while waiting for the screen"
		w95_shot "$cur" || true
		if [ ! -f "$prev" ] || [ "$(w95_changed "$prev" "$cur")" -gt 400 ]; then
			since=$(date +%s)
		elif [ $(($(date +%s) - since)) -ge "$need" ]; then
			return 0
		fi
		cp "$cur" "$prev"
		sleep 2
	done
	return 1
}

# w95_wait_change TIMEOUT: returns 0 once the screen differs from how it looked at the call by more than a caret (a dialog opened).
w95_wait_change() {
	local limit="$1" start ref="$W95_HOME/run/ref.png" cur="$W95_HOME/run/cur.png"
	start=$(date +%s)
	w95_shot "$ref"
	while [ $(($(date +%s) - start)) -lt "$limit" ]; do
		sleep 1
		w95_shot "$cur" || true
		[ "$(w95_changed "$ref" "$cur")" -gt 2000 ] && return 0
	done
	return 1
}

# w95_teal: the fraction (0 to 1) of the screen in the default Windows 95 desktop colour.
w95_teal() {
	local t="$W95_HOME/run/teal.png"
	w95_shot "$t" || { echo 0; return; }
	convert "$t" -fuzz 2% -fill white -opaque '#008080' -fill black +opaque white -colorspace Gray -format '%[fx:mean]' info: 2> /dev/null
}

# w95_wait_desktop TIMEOUT: returns 0 once the teal desktop covers at least 55 percent of the screen.
w95_wait_desktop() {
	local limit="$1" start frac
	start=$(date +%s)
	while [ $(($(date +%s) - start)) -lt "$limit" ]; do
		w95_running || w95_die "DOSBox-X exited while waiting for the desktop"
		frac="$(w95_teal)"
		if awk -v f="$frac" 'BEGIN { exit !(f >= 0.55) }'; then
			return 0
		fi
		sleep 3
	done
	return 1
}

# w95_shutdown: Start menu, Shut Down, Yes; then stops the container once Windows has had time to flush its disks.
w95_shutdown() {
	w95_key ctrl+Escape
	sleep 2
	w95_key u
	sleep 2
	w95_key Return
	sleep "${W95_SHUTDOWN_WAIT:-25}"
	w95_stop
}

# w95_part_image FILE SECTORS START TYPE: an empty disk image with one partition of the given type.
w95_part_image() {
	local f="$1" sectors="$2" start="$3" type="$4"
	rm -f "$f"
	truncate -s $((sectors * 512)) "$f"
	printf 'label: dos\nstart=%s, type=%s, bootable\n' "$start" "$type" | sfdisk -q "$f" > /dev/null
}

# w95_mcopy_dir IMAGE HOST_DIR: copies the contents of a host directory to the root of the partition in an exchange image.
w95_mcopy_dir() {
	local img="$1" dir="$2" e
	[ -n "$(ls -A "$dir")" ] || return 0
	for e in "$dir"/*; do
		mcopy -s -m -Q -i "$img@@$((W95_PART_START * 512))" "$e" ::/
	done
}

# w95_make_dimage FILE HOST_DIR: a FAT16 exchange disk holding a host directory (Windows 95 sees it as D:).
w95_make_dimage() {
	local f="$1" dir="$2"
	w95_part_image "$f" "$W95_D_SECTORS" "$W95_PART_START" 6
	mkfs.fat -F 16 -n EXCHANGE --offset "$W95_PART_START" -h "$W95_PART_START" "$f" $(((W95_D_SECTORS - W95_PART_START) / 2)) > /dev/null
	w95_mcopy_dir "$f" "$dir"
}
