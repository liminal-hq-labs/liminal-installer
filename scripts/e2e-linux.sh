#!/usr/bin/env bash
# Builds a themed `.run` and a signed feed on this machine, then installs, updates, repairs and
# uninstalls it inside a clean Debian container.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: scripts/e2e-linux.sh
#
# Needs cargo and docker. `liminst` is built here. The runner goes into the container, so it is
# built in a Debian-based Rust image by default, which keeps it from needing a newer glibc than
# the test container has. Environment:
#   E2E_IMAGE        the container the installer runs in (default debian:stable-slim)
#   E2E_BUILD_IMAGE  where the runner is built (default rust:1-slim)
#   E2E_BUILD=host   build the runner on this machine instead (fast, but only runs where the
#                    container's glibc is at least as new as this machine's)
# Exits 0 when every step passed.

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"
root="$PWD"
image="${E2E_IMAGE:-debian:stable-slim}"
build_image="${E2E_BUILD_IMAGE:-rust:1-slim}"

cargo build --locked -p li-cli
liminst="$root/target/debug/liminst"

if [ "${E2E_BUILD:-container}" = host ]; then
	cargo build --locked -p li-runner
	engine="$root/target/debug/liminal-engine"
else
	target="$root/target/e2e-linux"
	mkdir -p "$target"
	# RUSTUP_TOOLCHAIN overrides rust-toolchain.toml, which pins a version the image lacks.
	docker run --rm -v "$root:/src:ro" -v "$target:/target" \
		-v liminal-e2e-cargo:/usr/local/cargo/registry \
		-e CARGO_TARGET_DIR=/target -e RUSTUP_TOOLCHAIN=stable -w /src "$build_image" \
		sh -c "cargo build --locked -p li-runner && chown -R $(id -u):$(id -g) /target"
	engine="$target/debug/liminal-engine"
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
project="$work/project"
mkdir -p "$project" "$work/feed"

run() {
	(cd "$project" && "$liminst" "$@")
}

write_payload() {
	local version="$1"
	mkdir -p "$project/installer/payload/linux-x86_64"
	printf '#!/bin/sh\necho "demo %s"\n' "$version" >"$project/installer/payload/linux-x86_64/demo"
	chmod +x "$project/installer/payload/linux-x86_64/demo"
	printf 'data\n' >"$project/installer/payload/linux-x86_64/data.txt"
}

run init --id hq.test.demo --name "Demo App" --publisher "Test Co" >/dev/null
run keygen --out keys/feed >/dev/null
printf '\n[signing]\nfeed_key = "keys/feed.pub"\n' >>"$project/installer/installer.toml"
cat >"$project/installer/components.toml" <<'TOML'
[[component]]
id       = "app"
name     = "Demo App"
required = true
files    = [{ from = "payload/{target}/*", to = "{install}/bin/" }]
shortcut = [{ name = "Demo App", target = "{install}/bin/demo", in = "start_menu" }]
TOML
run check

for version in 1.0.0 2.0.0; do
	write_payload "$version"
	run build -t linux-x86_64 --version "$version" --output run --engine "$engine" >/dev/null
	out="$project/build/linux-x86_64"
	cp "$out/demo-$version-linux-x86_64.run" "$work/"
	if [ "$version" = 2.0.0 ]; then
		cp "$out/demo-2.0.0-linux-x86_64.lip" "$work/feed/"
		run feed add --feed "$work/feed/feed.json" -t linux-x86_64 \
			--file "$out/demo-2.0.0-linux-x86_64.lip" --url demo-2.0.0-linux-x86_64.lip \
			--version 2.0.0 --sign-key keys/feed.key >/dev/null
	fi
done

cp "$root/scripts/e2e-linux-container.sh" "$work/inside.sh"
echo "Running in $image"
docker run --rm -v "$work:/work:ro" "$image" sh /work/inside.sh
echo "e2e-linux: passed"
