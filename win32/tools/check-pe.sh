#!/usr/bin/env bash
# Fails when SETUP.EXE is not a plain 32-bit Windows GUI program that Windows 95 will load: subsystem 4.0, no relocations, no C runtime.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: win32/tools/check-pe.sh [SETUP.EXE]
set -euo pipefail
cd "$(dirname "$0")/.."
exe="${1:-dist/SETUP.EXE}"
fail() { echo "check-pe: $*" >&2; exit 1; }
if command -v file > /dev/null; then
	what="$(file -b "$exe")"
	case "$what" in
	"PE32 executable for MS Windows 4.00 (GUI), Intel i386"* | "PE32 executable (GUI) Intel 80386, for MS Windows"*) ;;
	*) fail "file says: $what" ;;
	esac
fi
OBJDUMP="$(command -v llvm-objdump llvm-objdump-19 llvm-objdump-18 llvm-objdump-17 2> /dev/null | head -1 || true)"
[ -n "$OBJDUMP" ] || { echo "check-pe: llvm-objdump not found"; [ -z "${WIN32_REQUIRE_TOOLS:-}" ] || exit 1; echo "(skipping)"; exit 0; }
hdr="$("$OBJDUMP" -p "$exe")"
field() { awk -v k="$1" '$1 == k {print $2}' <<< "$hdr" | head -1; }
[ "$(field MajorSubsystemVersion)" = 4 ] && [ "$(field MinorSubsystemVersion)" = 0 ] || fail "the subsystem version is not 4.0"
[ "$(field MajorOSystemVersion)" = 4 ] || fail "the OS version is not 4"
grep -q "relocations stripped" <<< "$hdr" || fail "the executable has relocations (it is built /FIXED)"
dlls="$(awk '/DLL Name:/ {print tolower($3)}' <<< "$hdr" | sort -u | tr '\n' ' ')"
case "$dlls" in
*msvcrt* | *ucrt* | *vcruntime* | *api-ms-*) fail "imports a C runtime: $dlls" ;;
esac
echo "check-pe: ok: PE32 GUI, subsystem 4.0, fixed base, imports only: $dlls"
