#!/bin/sh
# The steps `e2e-linux.sh` runs inside the container: install, update, repair, uninstall.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -eu

prefix=/opt/demo
home=${HOME:-/root}
export XDG_DATA_HOME="$home/.local/share"

fail() {
	echo "e2e-linux: $*" >&2
	exit 1
}

snapshot() {
	find "$home" /opt /tmp -mindepth 1 2>/dev/null | grep -v '^/tmp/e2e-' | sort
}

snapshot >/tmp/e2e-before.txt

sh /work/demo-1.0.0-linux-x86_64.run --prefix "$prefix" -- --headless
[ "$("$prefix/bin/demo")" = "demo 1.0.0" ] || fail "v1 did not install"
[ -f "$prefix/.liminal/receipt.json" ] || fail "no receipt"
grep -q '"installed": "1.0.0"' "$prefix/.liminal/receipt.json" || fail "receipt is not 1.0.0"

runner="$prefix/.liminal/runner/liminal-engine"
"$runner" --headless update --feed /work/feed
[ "$("$prefix/bin/demo")" = "demo 2.0.0" ] || fail "update did not install v2"

rm "$prefix/bin/data.txt"
"$runner" --headless repair
[ -f "$prefix/bin/data.txt" ] || fail "repair did not restore data.txt"

"$runner" --headless uninstall
[ ! -e "$prefix" ] || fail "the install directory is still there"

snapshot >/tmp/e2e-after.txt
if ! diff -u /tmp/e2e-before.txt /tmp/e2e-after.txt >&2; then
	fail "files were left behind"
fi
echo "container: install, update, repair and uninstall passed"
