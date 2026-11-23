#!/usr/bin/env bash
# Installs and uninstalls an example on Windows 3.11 in DOSBox-X, with ENGINE.EXE, and shows UIDEMO.EXE's pages in screenshots.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: W31_TREE=DIR [SHOTS=DIR] retro/win16/tools/e2e-w31.sh [lt-fizz|planboard]
#
# W31_TREE is a Windows 3.11 installation as a DOS drive (a folder with WINDOWS/ and WIN.BAT). The repository cannot carry one, so this is a
# manual run: it copies the tree, mounts the copy as C: and a scratch folder as D: in DOSBox-X, and starts Windows with
# `win /3 D:\ENGINE.EXE I C:\X /LOG=C:\ENGINE.LOG`. DOSBox-X comes from the `dosbox-x` on the path or, failing that, the Docker image named by
# DOSBOX_IMAGE (default liminal-installer-retro:local, from retro/build.sh). It needs an X server (xvfb-run is used when DISPLAY is unset) and
# ImageMagick's `import` for the screenshots. With SHOTS set, a screenshot of the desktop after each run goes there.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."
ex="${1:-lt-fizz}"
: "${W31_TREE:?set W31_TREE to a Windows 3.11 tree}"

if [ -z "${DISPLAY:-}" ] && [ -z "${E2E_W31_XVFB:-}" ] && command -v xvfb-run > /dev/null; then
	E2E_W31_XVFB=1 exec xvfb-run -a -s "-screen 0 1024x768x24" "$0" "$@"
fi

work="$(mktemp -d)"
cleanup() { docker rm -f "w16dbx-$$" > /dev/null 2>&1 || true; rm -rf "$work"; }
trap cleanup EXIT
mkdir -p "$work/d" "$work/conf"
cp -a --reflink=auto "$W31_TREE" "$work/c"

OUT_DIR="$work/exe" bash retro/win16/build.sh > /dev/null
cargo build -q --locked -p li-cli
liminst="${CARGO_TARGET_DIR:-$PWD/target}/debug/liminst"
"$liminst" build -C "examples/$ex" -t win16-x86 --version 1.0.0 -o "$work/out" --output files > /dev/null 2>&1
cp "$work/out/win16/SETUP.LPK" "$work/exe/ENGINE.EXE" "$work/exe/UIDEMO.EXE" "$work/d/"

run() { # run <command line for WIN> <log> <shot name>
	{
		sed -n '1,/^\[autoexec\]/p' <<'CONF'
[sdl]
output=surface
fullscreen=false
windowresolution=800x600
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
	local runner=(dosbox-x)
	if ! command -v dosbox-x > /dev/null; then
		docker rm -f "w16dbx-$$" > /dev/null 2>&1 || true
		docker run -d --name "w16dbx-$$" --user "$(id -u):$(id -g)" --network host -v /tmp/.X11-unix:/tmp/.X11-unix -e DISPLAY="$DISPLAY" \
			-e SDL_AUDIODRIVER=dummy -e HOME=/tmp -v "$work/c:/c" -v "$work/d:/d" -v "$work/conf:/conf" --entrypoint dosbox-x \
			"${DOSBOX_IMAGE:-liminal-installer-retro:local}" -conf /conf/run.conf -nomenu -fastlaunch > /dev/null
	else
		(SDL_AUDIODRIVER=dummy "${runner[@]}" -conf "$work/conf/run.conf" -nomenu -fastlaunch > /dev/null 2>&1 &)
	fi
	for _ in $(seq 1 240); do
		grep -q '^exit' "$work/c/$2" 2> /dev/null && break
		sleep 1
	done
	sleep 2
	[ -z "${SHOTS:-}" ] || { mkdir -p "$SHOTS"; import -window root "$SHOTS/$3.png" || true; }
	docker rm -f "w16dbx-$$" > /dev/null 2>&1 || true
	grep -q '^exit 0' "$work/c/$2" 2> /dev/null
}

name="E2E$(echo "$ex" | tr -d - | tr '[:lower:]' '[:upper:]')"
target="C:\\$name"
host="$work/c/$name"
fail=0
run "D:\\ENGINE.EXE I $target /LOG=C:\\ENGINE.LOG" ENGINE.LOG "w31-$ex-install" || { echo "e2e-w31: $ex install did not exit 0" >&2; fail=1; }
n=0
if [ -f "$host/INSTALL.LIR" ]; then
	while read -r kind name size _crc; do
		[ "$kind" = F ] || continue
		[ "$(stat -c %s "$host/$(echo "$name" | tr '\\' '/')")" = "$size" ] || { echo "e2e-w31: $name is not $size bytes" >&2; fail=1; }
		n=$((n + 1))
	done < <(tr -d '\r' < "$host/INSTALL.LIR")
	echo "e2e-w31: $ex installed $n files, receipt checked; $(grep -E '^heap peak' "$work/c/ENGINE.LOG" | tr -d '\r')"
else
	echo "e2e-w31: no receipt" >&2
	fail=1
fi
run "D:\\ENGINE.EXE U $target /LOG=C:\\ENGINE2.LOG" ENGINE2.LOG "w31-$ex-uninstall" || { echo "e2e-w31: $ex uninstall did not exit 0" >&2; fail=1; }
if [ -e "$host" ]; then
	echo "e2e-w31: $ex uninstall left $host behind" >&2
	fail=1
else
	echo "e2e-w31: $ex uninstalled"
fi
exit "$fail"
