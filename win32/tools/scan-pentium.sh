#!/usr/bin/env bash
# Fails when the executable holds an instruction a Pentium lacks: MMX, SSE and later vector code, CMOV, or the newer bit tricks.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win32/tools/scan-pentium.sh [SETUP.EXE]
#
# The prebuilt Rust `core` assumes SSE2 (D62), so the build compiles `core` from source for a Pentium; this is the check that nothing
# else slipped in, such as a prebuilt object. Needs llvm-objdump; WIN32_REQUIRE_TOOLS=1 makes a missing one an error.
set -euo pipefail
cd "$(dirname "$0")/.."
exe="${1:-dist/SETUP.EXE}"
OBJDUMP="$(command -v llvm-objdump llvm-objdump-19 llvm-objdump-18 llvm-objdump-17 2> /dev/null | head -1 || true)"
[ -n "$OBJDUMP" ] || { echo "scan-pentium: llvm-objdump not found"; [ -z "${WIN32_REQUIRE_TOOLS:-}" ] || exit 1; echo "(skipping)"; exit 0; }
# A vector register, a conditional move, or one of the instructions Pentium-class chips lack.
bad="$("$OBJDUMP" -d --no-show-raw-insn "$exe" | grep -Ei '(xmm|ymm|zmm)[0-9]|[[:space:]]%?mm[0-7]([^0-9a-z]|$)|[[:space:]](cmov[a-z]+|emms|movbe|popcnt|lzcnt|tzcnt|prefetch[a-z0-9]*|[ls]fence|mfence)[[:space:]]' || true)"
if [ -n "$bad" ]; then
	echo "scan-pentium: instructions a Pentium lacks:" >&2
	echo "$bad" | head -20 >&2
	exit 1
fi
echo "scan-pentium: ok: $exe has no MMX, SSE, CMOV or later instructions"
