#!/usr/bin/env bash
# Builds SETUP.EXE for DOS (386 extender) in Docker and smoke-tests it under headless DOSBox-X.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: build.sh [--no-run]
#
# Writes retro/out/SETUP.EXE. Unless --no-run is given, runs it in DOSBox-X with a dummy video
# driver and fails when the banner is missing from the captured output.

run_test=1
[ "${1:-}" = "--no-run" ] && run_test=0

cd "$(dirname "${BASH_SOURCE[0]}")"
image="liminal-installer-retro:local"
rm -rf out
mkdir -p out

docker build -t "$image" .

# 386 protected mode with the PMODE/W extender, linked into the executable so it runs on its own.
docker run --rm -v "$PWD:/work" -u "$(id -u):$(id -g)" "$image" \
	wcl386 -bt=dos -l=pmodew -zq -fe=out/SETUP.EXE setup.c
rm -f setup.obj ./*.o ./*.err

[ -f out/SETUP.EXE ] || { echo "build produced no SETUP.EXE" >&2; exit 1; }
echo "built: $(wc -c <out/SETUP.EXE) bytes"
head -c 2 out/SETUP.EXE | grep -q MZ || { echo "SETUP.EXE is not a DOS executable" >&2; exit 1; }

if [ "$run_test" -eq 0 ]; then
	exit 0
fi

# DOSBox-X mounts out/ as C:, runs the program, redirects its output to a file and quits.
cat >out/test.conf <<'CONF'
[sdl]
output=surface
[dosbox]
machine=svga_s3
[cpu]
core=normal
[autoexec]
@echo off
mount c /work/out
c:
setup.exe > result.txt
exit
CONF

docker run --rm -v "$PWD:/work" -u "$(id -u):$(id -g)" -e HOME=/tmp \
	-e SDL_VIDEODRIVER=dummy -e SDL_AUDIODRIVER=dummy "$image" \
	timeout 60 dosbox-x -conf /work/out/test.conf -nomenu -fastlaunch -silent || true

# DOS stores the redirect target in upper case.
[ -f out/RESULT.TXT ] || { echo "DOSBox-X wrote no output file" >&2; exit 1; }
result="$(tr -d '\r' <out/RESULT.TXT)"
echo "--- program output ---"
echo "$result"
echo "----------------------"
grep -q "SETUP OK" <<<"$result" || { echo "banner not found in output" >&2; exit 1; }
echo "DOS smoke test passed"
