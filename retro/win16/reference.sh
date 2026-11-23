#!/usr/bin/env bash
# Builds the Open Watcom reference hello (a Win16 NE executable) in the retro build image, for size and header comparison.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: reference.sh
#
# Needs the `liminal-installer-retro:local` image (`bash retro/build.sh --no-run` builds it). Writes out/WC_HELLO.EXE, built with the
# default settings, and out/WC_HELLO_S.EXE, built for size, then dumps the first one's NE header with tools/ne_dump.py.

cd "$(dirname "${BASH_SOURCE[0]}")"
image="liminal-installer-retro:local"
mkdir -p out

docker run --rm -v "$PWD:/work" -u "$(id -u):$(id -g)" -e INCLUDE=/opt/watcom/h:/opt/watcom/h/win "$image" sh -c '
	wcl -bt=windows -l=windows -zq -fe=out/WC_HELLO.EXE ref/hello.c
	wcl -bt=windows -l=windows -zq -os -s -fe=out/WC_HELLO_S.EXE ref/hello.c
	rm -f hello.o ./*.o'
echo "watcom: $(wc -c <out/WC_HELLO.EXE) bytes (default), $(wc -c <out/WC_HELLO_S.EXE) bytes (-os -s)"
python3 tools/ne_dump.py out/WC_HELLO.EXE
