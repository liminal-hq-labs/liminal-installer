#!/bin/bash
# Checks the DOS workspace: format, lints, then builds SETUP.EXE and runs it under headless DOSBox when the tools are present.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/check.sh [--no-run]
#
# The format and lint checks always run. The build and the DOSBox run need the tools from dos/tools/fetch-tools.sh; without them,
# or with --no-run, they are skipped with a note, so the script is safe in a checkout that has no DOS tools.
set -euo pipefail
cd "$(dirname "$0")"

cargo fmt --all -- --check
RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings

VENDOR="${DOS_VENDOR:-$PWD/vendor}"
if [ "${1:-}" = "--no-run" ] || ! [ -f "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" ] || [ -z "$(ls "$VENDOR"/dosbox/*/dosbox 2> /dev/null)" ]; then
	echo "check: DOS tools not present (or --no-run): skipping the build and the DOSBox run"
	exit 0
fi

./build.sh
out="$(tools/dosrun.sh dist SETUP.EXE /V)"
echo "$out"
case "$out" in
Liminal\ Installer*) echo "check: SETUP.EXE ran under DOSBox" ;;
*) echo "check: unexpected output from SETUP.EXE" >&2; exit 1 ;;
esac
