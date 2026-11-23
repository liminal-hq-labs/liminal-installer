#!/usr/bin/env bash
# Checks that the committed ts-rs bindings match what the Rust types generate.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# The generated folder is snapshotted and deleted, regenerated, and compared. Deleting first
# matters: ts-rs only writes files for types that still exist and never removes a stale one, so
# a removed or renamed type would otherwise leave its old file and hide the drift. Exits 1 and
# lists the differences on drift, leaving the regenerated files in place so `git diff` shows the
# change. If anything aborts before the comparison, the snapshot is put back, so a failed check
# never leaves the bindings deleted.
set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"

dir="packages/protocol/src/generated"
snapshot="$(mktemp -d)"
compared=0

cleanup() {
	if [ "${compared}" -ne 1 ]; then
		rm -rf "${dir}"
		mkdir -p "${dir}"
		cp -r "${snapshot}/." "${dir}/"
	fi
	rm -rf "${snapshot}"
}
trap cleanup EXIT

mkdir -p "${dir}"
cp -r "${dir}/." "${snapshot}/"
rm -rf "${dir}"

bash scripts/generate-protocol-bindings.sh >/dev/null

drift=0
if ! diff -rq "${snapshot}" "${dir}"; then
	drift=1
fi
# The comparison finished, so what is on disk now is the regenerated output to keep.
compared=1

if [ "${drift}" -ne 0 ]; then
	echo
	echo "The committed bindings are out of date. Run 'bash scripts/generate-protocol-bindings.sh' and commit the result."
	exit 1
fi

echo "Generated bindings are up to date."
