#!/usr/bin/env bash
# Builds liminst (and the engine runner when it exists) in release mode and archives the binaries.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: ci-stage.sh PLATFORM
#
# Writes dist/liminal-installer-PLATFORM.tar.gz. The runner crate `li-runner` is built only when
# the workspace has it, so the script works before and after that crate lands.

platform="${1:?usage: ci-stage.sh PLATFORM}"
cd "$(dirname "${BASH_SOURCE[0]}")/.."

meta="$(cargo metadata --no-deps --format-version 1 --locked)"
packages=(li-cli)
if jq -e '.packages[] | select(.name == "li-runner")' <<<"$meta" >/dev/null; then
	packages+=(li-runner)
else
	echo "li-runner is not in this workspace; building li-cli only"
fi

args=()
for p in "${packages[@]}"; do args+=(-p "$p"); done
cargo build --release --locked "${args[@]}"

exe=""
case "$platform" in windows*) exe=".exe" ;; esac

stage="dist/stage"
rm -rf "$stage"
mkdir -p "$stage"
for p in "${packages[@]}"; do
	while IFS= read -r bin; do
		cp "target/release/${bin}${exe}" "$stage/"
		echo "staged ${bin}${exe}"
	done < <(jq -r --arg p "$p" '.packages[] | select(.name == $p) | .targets[] | select(.kind | index("bin")) | .name' <<<"$meta")
done

tar -C "$stage" -czf "dist/liminal-installer-${platform}.tar.gz" .
rm -rf "$stage"
ls -l dist
