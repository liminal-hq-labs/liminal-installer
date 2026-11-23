#!/usr/bin/env bash
# Cross-builds the Tauri NSIS setup variants the Windows spike probes, from a Linux host.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: scripts/spikes/build-tauri-variants.sh APP_DIR OUT_DIR [--offline]
#
# APP_DIR is a minimal Tauri 2 app (a one-line frontend, `tauri-plugin-deep-link` as the only
# plugin, `productName` SpikeApp, `publisher` Spike Publisher, `identifier` ca.liminal.spikeapp,
# version 1.0.0). OUT_DIR receives one `<variant>.exe` per variant. Set CARGO_TARGET_DIR to a
# directory you will delete afterwards, and put a `makensis` on PATH (a wrapper around a
# container is enough). `TAURI` names the Tauri CLI and defaults to `tauri`; the runner is
# `cargo-xwin`. `--offline` adds the offline WebView2 installer variant, which downloads and
# embeds about 170 MB.
#
# Variants: install mode (`cu`, `pm`, `both`), the same three with a file association and a
# deep-link scheme (`*-assoc`), the WebView2 modes (`cu-embed`, `cu-offline`), and versions for
# the upgrade and downgrade probes (`cu-v110`, `cu-v090-dg-true`, `cu-v090-dg-false`). Set `ONLY`
# to a space-separated list of variant names to build just those.

app_dir="${1:?usage: build-tauri-variants.sh APP_DIR OUT_DIR [--offline]}"
out_dir="${2:?usage: build-tauri-variants.sh APP_DIR OUT_DIR [--offline]}"
offline="${3:-}"
tauri="${TAURI:-tauri}"
target="x86_64-pc-windows-msvc"

mkdir -p "$out_dir"
cd "$app_dir"

assoc='"fileAssociations":[{"ext":["spk"],"name":"SpikeFile","description":"Spike file","role":"Editor"}]'
scheme='"plugins":{"deep-link":{"desktop":{"schemes":["spikeapp"]}}}'

mode_json() {
	printf '{"bundle":{"windows":{"nsis":{"installMode":"%s"}}}}' "$1"
}

assoc_json() {
	printf '{"bundle":{%s,"windows":{"nsis":{"installMode":"%s"}}},%s}' "$assoc" "$1" "$scheme"
}

build() {
	local name="$1" config="$2"
	# ONLY="a b" limits the run to the named variants.
	if [ -n "${ONLY:-}" ] && [[ " $ONLY " != *" $name "* ]]; then return 0; fi
	echo "== $name"
	rm -rf "${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}/$target/release/bundle/nsis"
	"$tauri" build --runner cargo-xwin --target "$target" --bundles nsis --config "$config" >"$out_dir/$name.log" 2>&1 ||
		{
			tail -20 "$out_dir/$name.log" >&2
			return 1
		}
	cp "${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}/$target/release/bundle/nsis/"*-setup.exe "$out_dir/$name.exe"
	# Keep the generated script so the exact values the template received can be read.
	cp "$CARGO_TARGET_DIR/$target/release/nsis/x64/installer.nsi" "$out_dir/$name.installer.nsi"
}

for mode in currentUser perMachine both; do
	short="cu"
	[ "$mode" = "perMachine" ] && short="pm"
	[ "$mode" = "both" ] && short="both"
	build "$short" "$(mode_json "$mode")"
	build "$short-assoc" "$(assoc_json "$mode")"
done

build cu-embed '{"bundle":{"windows":{"nsis":{"installMode":"currentUser"},"webviewInstallMode":{"type":"embedBootstrapper","silent":true}}}}'
if [ "$offline" = "--offline" ]; then
	build cu-offline '{"bundle":{"windows":{"nsis":{"installMode":"currentUser"},"webviewInstallMode":{"type":"offlineInstaller","silent":true}}}}'
fi

build cu-v110 '{"version":"1.1.0","bundle":{"windows":{"nsis":{"installMode":"currentUser"}}}}'
build cu-v090-dg-true '{"version":"0.9.0","bundle":{"windows":{"allowDowngrades":true,"nsis":{"installMode":"currentUser"}}}}'
build cu-v090-dg-false '{"version":"0.9.0","bundle":{"windows":{"allowDowngrades":false,"nsis":{"installMode":"currentUser"}}}}'

ls -l "$out_dir"/*.exe
