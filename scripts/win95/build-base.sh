#!/usr/bin/env bash
# Installs Windows 95 OSR2.5 from the CD image onto a raw disk image under DOSBox-X, unattended, and freezes it as the read-only base.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: build-base.sh [--force] ISO KEYFILE
#
#   ISO      the Windows 95 OSR2 CD image (volume label WIN_95C); it is only ever read
#   KEYFILE  a text file whose last line is the OEM product key, XXXXX-OEM-XXXXXXX-XXXXX; typed into Setup, never written to disk or a log
#   --force  forget finished stages and start again from the beginning
#
# Environment: W95_HOME (default $HOME/win95-testbed), W95_DISPLAY, W95_IMAGE; see lib.sh. Result: $W95_HOME/win95-base.img (read-only), a
# 1 GB FAT32 system disk. Every stage leaves a marker in $W95_HOME/state, so a failed run resumes where it stopped; `setup` restarts from
# the snapshot `hd-prepared.img`. The install takes about ten minutes of wall time. Screenshots of each stage go to $W95_HOME/shots/build-*.png.
set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

FORCE=0
if [ "${1:-}" = "--force" ]; then
	FORCE=1
	shift
fi
ISO="${1:?usage: build-base.sh [--force] ISO KEYFILE}"
KEYFILE="${2:?usage: build-base.sh [--force] ISO KEYFILE}"
[ -f "$ISO" ] || w95_die "no such ISO: $ISO"
[ -f "$KEYFILE" ] || w95_die "no such key file: $KEYFILE"
w95_need docker Xvfb xdotool import convert compare 7z sfdisk mkfs.fat mcopy mtype mdir truncate flock md5sum
w95_lock
trap 'w95_stop; w95_xvfb_stop' EXIT

# The key, split for the four boxes of the Certificate of Authenticity page; held in shell variables only.
KEY_RE='^[A-Za-z0-9]{5}-OEM-[A-Za-z0-9]{7}-[A-Za-z0-9]{5}$'
KEY="$(tr -d '\r' < "$KEYFILE" | grep -E "$KEY_RE" | tail -1 || true)"
[ -n "$KEY" ] || w95_die "no line of the form XXXXX-OEM-XXXXXXX-XXXXX in the key file"
IFS=- read -r KEY_A _ KEY_C KEY_D <<< "$KEY"
unset KEY

BASE="$W95_HOME/win95-base.img"
HD="$W95_HOME/hd-install.img"
PREP="$W95_HOME/hd-prepared.img"
OFF=$((W95_PART_START * 512))
SHOTS="$W95_HOME/shots"
STATE="$W95_HOME/state"

if [ "$FORCE" = 1 ]; then
	rm -f "$STATE/prep.done" "$STATE/boot.done" "$STATE/setup.done" "$STATE/verify.done"
	[ -f "$BASE" ] && chmod u+w "$BASE" && rm -f "$BASE"
fi
if [ -f "$BASE" ] && [ -f "$STATE/verify.done" ]; then
	w95_log "the base image is already built: $BASE"
	exit 0
fi

stage_prep() {
	w95_log "prep: boot floppy, Windows 95 files and a partitioned FAT32 disk"
	rm -rf "$W95_HOME/scratch/src"
	mkdir -p "$W95_HOME/scratch/src"
	7z e -so "$ISO" '[BOOT]/Boot-1.44M.img' > "$W95_HOME/boot.img" 2> /dev/null
	[ "$(stat -c %s "$W95_HOME/boot.img")" = 1474560 ] || w95_die "could not read the El Torito boot floppy from the ISO"
	nice -n 19 ionice -c3 7z x -y -o"$W95_HOME/scratch/src" "$ISO" 'WIN95/*' > /dev/null
	[ -f "$W95_HOME/scratch/src/WIN95/SETUP.EXE" ] || w95_die "WIN95/SETUP.EXE is missing from the ISO"
	w95_part_image "$HD" "$W95_C_SECTORS" "$W95_PART_START" c
	mkfs.fat -F 32 -s 8 -n WIN95 -h "$W95_PART_START" --offset "$W95_PART_START" "$HD" $(((W95_C_SECTORS - W95_PART_START) / 2)) > /dev/null
	nice -n 19 ionice -c3 mcopy -s -m -Q -i "$HD@@$OFF" "$W95_HOME/scratch/src/WIN95" ::/WIN95
	rm -rf "$W95_HOME/scratch/src"
	# The answer file skips the wizard pages; the product key stays out of it (Setup takes the key from the keyboard, below).
	{
		# shellcheck disable=SC2016 # the dollar signs are literal
		printf '[BatchSetup]\r\nVersion=3.0 (32-bit)\r\n\r\n[Version]\r\nSignature="$CHICAGO$"\r\n\r\n'
		printf '[Setup]\r\nExpress=1\r\nInstallDir="C:\\WINDOWS"\r\nInstallType=3\r\nEBD=0\r\nShowEula=0\r\nChangeDir=0\r\n'
		printf 'OptionalComponents=1\r\nNetwork=0\r\nSystem=0\r\nCCP=0\r\nCleanBoot=0\r\nUninstall=0\r\nNoPrompt2Boot=1\r\nTimeZone="Eastern"\r\n\r\n'
		printf '[NameAndOrg]\r\nName="Testbed"\r\nOrg="Liminal HQ"\r\nDisplay=0\r\n\r\n[Network]\r\nDisplay=0\r\n'
	} > "$W95_HOME/scratch/MSBATCH.INF"
	mcopy -o -i "$HD@@$OFF" "$W95_HOME/scratch/MSBATCH.INF" ::/WIN95/MSBATCH.INF
	# The first boot runs Setup once; the guard stops it running again after Windows is installed.
	printf '@echo off\r\nif exist C:\\WINDOWS\\WIN.COM goto end\r\nC:\\WIN95\\SETUP.EXE C:\\WIN95\\MSBATCH.INF /IS /IQ\r\n:end\r\n' > "$W95_HOME/scratch/AUTOEXEC.BAT"
	mcopy -o -i "$HD@@$OFF" "$W95_HOME/scratch/AUTOEXEC.BAT" ::/AUTOEXEC.BAT
	touch "$STATE/prep.done"
}

stage_boot() {
	w95_log "boot: writing the master boot record and the DOS system files (the Windows 95 boot floppy)"
	cp --sparse=always "$W95_HOME/boot.img" "$W95_HOME/boot-work.img"
	printf '@echo off\r\nfdisk /mbr\r\nsys c:\r\necho done > a:\\done.txt\r\n' > "$W95_HOME/scratch/BOOTAUTO.BAT"
	mcopy -o -i "$W95_HOME/boot-work.img" "$W95_HOME/scratch/BOOTAUTO.BAT" ::/AUTOEXEC.BAT
	w95_floppy_conf "$W95_HOME/conf/floppy.conf" "/w/hd-install.img" "/w/boot-work.img"
	w95_start "$W95_HOME/conf/floppy.conf"
	for _ in $(seq 1 60); do
		mtype -i "$W95_HOME/boot-work.img" ::done.txt > /dev/null 2>&1 && break
		w95_running || w95_die "DOSBox-X exited during the boot stage"
		sleep 3
	done
	mtype -i "$W95_HOME/boot-work.img" ::done.txt > /dev/null 2>&1 || w95_die "the boot floppy never finished (see $SHOTS/build-boot.png)"
	w95_shot "$SHOTS/build-boot.png"
	sleep 2
	w95_stop
	rm -f "$W95_HOME/boot-work.img"
	cp --sparse=always "$HD" "$PREP"
	touch "$STATE/boot.done"
}

stage_setup() {
	w95_log "setup: restoring the prepared disk and running Windows 95 Setup"
	cp --sparse=always "$PREP" "$HD"
	w95_conf "$W95_HOME/conf/install.conf" "/w/hd-install.img"
	local t0 t1
	t0=$(date +%s)
	w95_start "$W95_HOME/conf/install.conf"

	w95_log "setup: waiting for the licence page"
	sleep 45
	w95_wait_stable 10 240 || w95_die "the licence page never settled (see $SHOTS/build-licence.png)"
	w95_shot "$SHOTS/build-licence.png"
	w95_key alt+y

	w95_log "setup: waiting for the Certificate of Authenticity page"
	sleep 20
	w95_wait_stable 10 300 || w95_die "the certificate page never settled (see $SHOTS/build-certificate.png)"
	w95_shot "$SHOTS/build-certificate.png"
	# Setup opens a "not valid" box over the empty page by itself; its Re-enter button puts the caret in the first box.
	w95_key Return
	sleep 3
	w95_wait_stable 3 30 || true
	printf '%s' "$KEY_A" | w95_type_stdin
	printf '%s' "$KEY_C" | w95_type_stdin
	printf '%s' "$KEY_D" | w95_type_stdin
	sleep 1
	# The page is not photographed after the key is typed.
	w95_key Return

	w95_log "setup: copying files, restarting, detecting hardware"
	local start nudges=0 still=0 prev="$W95_HOME/run/loop-prev.png" cur
	start=$(date +%s)
	while [ $(($(date +%s) - start)) -lt 2400 ]; do
		w95_running || w95_die "DOSBox-X exited during Setup"
		if w95_wait_desktop 1; then
			break
		fi
		cur="$(w95_hash)"
		if [ -f "$prev" ] && [ "$(w95_changed "$prev" "$cur")" -le 400 ]; then
			still=$((still + 1))
		else
			still=0
		fi
		cp "$cur" "$prev"
		# A page that has sat still for a minute without the desktop behind it is a wizard waiting for a key: the printer page on this disk.
		if [ "$still" -ge 12 ] && [ "$nudges" -lt 4 ]; then
			nudges=$((nudges + 1))
			w95_shot "$SHOTS/build-nudge-$nudges.png"
			w95_log "setup: a page sat still; sending Escape (nudge $nudges)"
			w95_key Escape
			still=0
		fi
		sleep 4
	done
	w95_wait_desktop 5 || w95_die "Setup never reached the desktop (see $SHOTS/build-nudge-*.png)"
	w95_log "setup: the desktop is up; letting first-run work finish"
	sleep 60
	w95_wait_desktop 30 || w95_die "the desktop went away again"
	w95_shot "$SHOTS/build-desktop.png"
	w95_shutdown
	t1=$(date +%s)
	w95_log "setup: done in $((t1 - t0)) seconds"
	echo "$((t1 - t0))" > "$STATE/setup.seconds"
	touch "$STATE/setup.done"
}

stage_verify() {
	w95_log "verify: booting a copy of the installed disk to the desktop"
	local work="$W95_HOME/verify-work.img" t0
	cp --sparse=always "$HD" "$work"
	w95_conf "$W95_HOME/conf/verify.conf" "/w/verify-work.img"
	t0=$(date +%s)
	w95_start "$W95_HOME/conf/verify.conf"
	w95_wait_desktop 300 || w95_die "the installed system did not reach the desktop (see $SHOTS/build-verify.png)"
	sleep 15
	w95_wait_desktop 10 || w95_die "the desktop did not stay up"
	w95_shot "$SHOTS/build-verify.png"
	w95_log "verify: first boot to the desktop took $(($(date +%s) - t0)) seconds"
	w95_shutdown
	rm -f "$work"
	mv "$HD" "$BASE"
	chmod 444 "$BASE"
	touch "$STATE/verify.done"
}

[ -f "$STATE/prep.done" ] || stage_prep
[ -f "$STATE/boot.done" ] || stage_boot
[ -f "$STATE/setup.done" ] || stage_setup
[ -f "$STATE/verify.done" ] || stage_verify
w95_log "base image ready (read-only): $BASE"
