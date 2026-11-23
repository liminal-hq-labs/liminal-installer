#!/usr/bin/env bash
# Fails when the executable imports a function that is not on the Windows 95 allowlist, or a wide-character (`...W`) entry point.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win32/tools/check-imports.sh [SETUP.EXE]
#
# `tools/imports.allow` lists `dll!Function` per line: only functions that Windows 95's KERNEL32, USER32, GDI32, COMCTL32, SHELL32, ADVAPI32 and OLE32 export
# (the list is checked by hand against the Win32 reference's "Windows 95" column, not read from a Windows 95 image).
set -euo pipefail
cd "$(dirname "$0")/.."
exe="${1:-dist/SETUP.EXE}"
OBJDUMP="$(command -v llvm-objdump llvm-objdump-19 llvm-objdump-18 llvm-objdump-17 2> /dev/null | head -1 || true)"
[ -n "$OBJDUMP" ] || { echo "check-imports: llvm-objdump not found"; [ -z "${WIN32_REQUIRE_TOOLS:-}" ] || exit 1; echo "(skipping)"; exit 0; }
got="$("$OBJDUMP" -p "$exe" | awk '/DLL Name:/ {dll=tolower($3)} /^ +[0-9]+ +[A-Za-z_]/ {print dll "!" $2}' | sort -u)"
allow="$(grep -v '^#' tools/imports.allow | grep -v '^$' | tr 'A-Z' 'a-z' | sort -u)"
fail=0
while IFS= read -r imp; do
	[ -n "$imp" ] || continue
	lower="$(echo "$imp" | tr 'A-Z' 'a-z')"
	if ! grep -qxF "$lower" <<< "$allow"; then
		echo "check-imports: not on the Windows 95 allowlist: $imp" >&2
		fail=1
	fi
	case "$imp" in *W) echo "check-imports: wide-character entry point: $imp" >&2; fail=1 ;; esac
done <<< "$got"
[ "$fail" = 0 ] || exit 1
echo "check-imports: ok: $(echo "$got" | wc -l) imports, all on the allowlist"
