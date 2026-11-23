#!/bin/bash
# Builds Lt. Fizz for DOS, installs it under headless DOSBox with typed keys, checks the files, the receipt and three screens, then uninstalls.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/tools/e2e.sh
#
# Needs the DOS tools (dos/tools/fetch-tools.sh) and a built runtime (dos/build.sh); exits 0 with a note when the tools are missing, so a
# checkout without them is not failed. The run: `liminst build -t dos-x86` writes SETUP.EXE and SETUP.DAT; DOSBox mounts them as C:; the keys
# of the install flow are typed with AUTOTYPE; the runtime writes every screen it shows to D:\DUMP.TXT (LI_DUMP); the welcome, components and
# copying screens are compared with crates/li-retro-build/tests/golden/, the host simulator's goldens, so the DOS runtime and the host agree
# to the character. LIMINST names the CLI (default: cargo run).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
VENDOR="${DOS_VENDOR:-$HERE/vendor}"

if [ ! -f "$VENDOR/cwsdpmi/bin/CWSDSTUB.EXE" ] || [ -z "$(ls "$VENDOR"/dosbox/*/dosbox 2> /dev/null)" ]; then
	echo "e2e: DOS tools not present; skipping (run dos/tools/fetch-tools.sh)"
	exit 0
fi

"$HERE/build.sh"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/li-e2e.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
LIMINST="${LIMINST:-cargo run -q -p li-cli --bin liminst --}"
# shellcheck disable=SC2086
(cd "$ROOT/examples/lt-fizz" && $LIMINST build -t dos-x86 --version 1.0.0 --dos-runtime "$HERE/dist/SETUP.EXE" -o "$WORK/out" > "$WORK/build.log")
cat "$WORK/build.log"

DISK="$WORK/disk"
mkdir -p "$DISK"
cp "$WORK/out/dos/SETUP.EXE" "$WORK/out/dos/SETUP.DAT" "$DISK/"
GOLDEN="$ROOT/crates/li-retro-build/tests/golden"

fail() { echo "e2e: FAIL: $*" >&2; exit 1; }

# Install: welcome, licence (Tab twice to the radio group, Space to accept), folder, components, sound, ready, done.
export DOS_MOUNT_OPTS="-freesize 64"
export DOS_ENV='LI_DUMP=D:\DUMP.TXT'
export DOS_KEEP="$WORK/keep"
export DOS_TIMEOUT=120
export DOS_AUTOTYPE="-w 3 -p 0.4 enter tab tab space enter enter enter enter enter enter"
rc=0
"$HERE/tools/dosrun.sh" "$DISK" SETUP.EXE > "$WORK/install.out" || rc=$?
[ "$rc" = 0 ] || fail "SETUP.EXE exited with $rc"

for f in FIZZ.EXE FIZZ.BAT SETUP.INI INSTALL.LIR LEVELS/EP1.LVL LEVELS/EP1.PAL SOUND/ADLIB.DRV SOUND/SBPRO.DRV; do
	[ -f "$DISK/LTFIZZ/$f" ] || { find "$DISK" | sort >&2; fail "LTFIZZ/$f was not installed"; }
done
cmp "$DISK/LTFIZZ/FIZZ.EXE" "$ROOT/examples/lt-fizz/installer/dist/dos-x86/FIZZ.EXE" || fail "FIZZ.EXE differs from the payload"
cmp "$DISK/LTFIZZ/SOUND/SBPRO.DRV" "$ROOT/examples/lt-fizz/installer/dist/sound/SBPRO.DRV" || fail "SBPRO.DRV differs from the payload"
head -n 6 "$DISK/LTFIZZ/INSTALL.LIR" | tr -d '\r' | grep -qx 'dir=C:\\LTFIZZ' || fail "the receipt does not name the folder"
tr -d '\r' < "$DISK/LTFIZZ/INSTALL.LIR" | grep -q '^F SOUND\\ADLIB.DRV ' || fail "the receipt does not list ADLIB.DRV"
tr -d '\r' < "$DISK/LTFIZZ/SETUP.INI" | grep -qx 'card=none' || fail "SETUP.INI has no sound card"
echo "e2e: install: files and receipt ok"

# Three screens against the goldens.
dump="$WORK/keep/DUMP.TXT"
[ -f "$dump" ] || fail "no screen dump"
compare() { # <golden name> <marker>
	"$HERE/tools/frames.py" "$dump" "$2" > "$WORK/$1.txt" || fail "no $1 screen in the dump"
	diff -u "$GOLDEN/$1.txt" "$WORK/$1.txt" > "$WORK/$1.diff" || { cat "$WORK/$1.diff" >&2; fail "the $1 screen differs from its golden"; }
	echo "e2e: screen $1 matches its golden"
}
compare welcome "Welcome to the Lieutenant Fizz setup."
compare components "Choose what to install."
compare copying "Copying SOUND\\SBPRO.DRV"

# Uninstall: confirm, then done.
export DOS_AUTOTYPE="-w 3 -p 0.4 enter enter enter"
rm -rf "$WORK/keep"
rc=0
"$HERE/tools/dosrun.sh" "$DISK" SETUP.EXE /U > "$WORK/uninstall.out" || rc=$?
[ "$rc" = 0 ] || fail "SETUP.EXE /U exited with $rc"
[ ! -e "$DISK/LTFIZZ" ] || { find "$DISK" | sort >&2; fail "the uninstall left LTFIZZ behind"; }
echo "e2e: uninstall: the install folder is gone"
echo "e2e: ok"
