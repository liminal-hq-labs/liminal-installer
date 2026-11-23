#!/usr/bin/env bash
# Runs `liminst build` for every example project and finishes the native packages with platform tools.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: ci-package.sh linux|windows|macos OUTDIR
#
# Expects `liminst` on PATH. Does nothing when examples/ has no installer project. Code signing and
# notarisation are separate workflow steps that run over OUTDIR afterwards.

os="${1:?usage: ci-package.sh linux|windows|macos OUTDIR}"
out_root="${2:?usage: ci-package.sh linux|windows|macos OUTDIR}"
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$out_root"
out_root="$(cd "$out_root" && pwd)"
cd "$repo"

case "$os-$(uname -m)" in
linux-x86_64) target="linux-x86_64" ;;
linux-aarch64 | linux-arm64) target="linux-aarch64" ;;
windows-*) target="windows-x86_64" ;;
macos-arm64 | macos-aarch64) target="macos-aarch64" ;;
macos-x86_64) target="macos-x86_64" ;;
*)
	echo "unsupported platform $os-$(uname -m)" >&2
	exit 2
	;;
esac

shopt -s nullglob
projects=(examples/*/installer/installer.toml)
if [ "${#projects[@]}" -eq 0 ]; then
	echo "no example installer projects under examples/; nothing to package"
	exit 0
fi

for toml in "${projects[@]}"; do
	proj="${toml%/installer/installer.toml}"
	name="$(basename "$proj")"
	if ! grep -Eq "^[[:space:]]*id[[:space:]]*=[[:space:]]*\"${target}\"" "$toml"; then
		echo "::notice::$name has no target $target; skipping"
		continue
	fi
	out="$out_root/$name"
	mkdir -p "$out"
	echo "== $name ($target)"
	liminst -C "$proj" check --target "$target"
	liminst -C "$proj" build --target "$target" --out "$out"

	case "$os" in
	windows)
		for wxs in "$out"/*.wxs; do
			wix build -d "SourceDir=$proj/installer" "$wxs" -o "${wxs%.wxs}.msi"
		done
		if [ -f "$out/AppxManifest.xml" ]; then
			makeappx="$(find "/c/Program Files (x86)/Windows Kits/10/bin" -name makeappx.exe -path '*x64*' 2>/dev/null | sort | tail -n 1)"
			if [ -n "$makeappx" ]; then
				layout="$out/msix-layout"
				mkdir -p "$layout"
				cp "$out/AppxManifest.xml" "$layout/"
				"$makeappx" pack /o /nv /d "$(cygpath -w "$layout")" /p "$(cygpath -w "$out/$name.msix")"
			else
				echo "::warning::makeappx not found; MSIX not packed"
			fi
		fi
		;;
	macos)
		if [ -f "$out/distribution.xml" ]; then
			id="$(sed -n 's/.*<pkg-ref id="\([^"]*\)" version=.*/\1/p' "$out/distribution.xml" | head -n 1)"
			slug="$(sed -n 's|.*onConclusion="none">\(.*\)\.pkg</pkg-ref>.*|\1|p' "$out/distribution.xml" | head -n 1)"
			version="$(sed -n 's/.*<pkg-ref id="[^"]*" version="\([^"]*\)".*/\1/p' "$out/distribution.xml" | head -n 1)"
			root="$out/pkgroot"
			mkdir -p "$root"
			pkgbuild --root "$root" --identifier "$id" --version "$version" "$out/$slug.pkg"
			productbuild --distribution "$out/distribution.xml" --package-path "$out" "$out/$slug-$version-$target.pkg"
			rm -rf "$root" "$out/$slug.pkg"

			app="$out/$slug.app"
			mkdir -p "$app/Contents/MacOS"
			cp "$out/Info.plist" "$app/Contents/Info.plist"
			if [ -x "$(command -v liminal-engine || true)" ]; then
				cp "$(command -v liminal-engine)" "$app/Contents/MacOS/"
			else
				echo "::notice::liminal-engine is not available; the .app has no engine yet"
			fi
			hdiutil create -volname "$name" -srcfolder "$app" -ov -format UDZO "$out/$slug-$version-$target.dmg"
		fi
		;;
	esac
done
ls -lR "$out_root"
