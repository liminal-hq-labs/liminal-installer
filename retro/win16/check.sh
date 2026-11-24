#!/usr/bin/env bash
# Checks the Win16 workspace: format, lints, the packer's tests, then builds SETUP.EXE and checks the executable and its segments.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: retro/win16/check.sh
#
# Nothing here needs Windows, Wine or an emulator: it is the cheap part of `make win16`. It builds SETUP.EXE, whose code the packer places
# in several segments up to the budgets in res/lisetup.segments.json (the packer fails when a segment would pass its budget, which is the
# size gate), checks that `file` reads it as a Windows NE executable (either wording: `NE version 5 for MS Windows` or `NE for MS
# Windows`), decodes every segment with `tools/ne_check.py` (no branch leaves its segment or lands inside an instruction) and prints the
# packer's report of the segments and the data.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

cargo fmt --all -- --check
RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings
python3 -m unittest discover -s tests

out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
OUT_DIR="$out" bash build.sh

for exe in SETUP; do
	desc="$(file "$out/$exe.EXE")"
	echo "$desc"
	if ! grep -Eq 'NE (version [0-9.]+ )?for MS Windows' <<<"$desc"; then
		echo "check: $exe.EXE is not an NE executable for Windows" >&2
		exit 1
	fi
done
python3 tools/ne_check.py "$out/SETUP.EXE"
python3 tools/ne_dump.py "$out/SETUP.EXE" | grep -E '^segment ' || true
echo "check: SETUP.EXE builds as an NE executable and every segment decodes"
