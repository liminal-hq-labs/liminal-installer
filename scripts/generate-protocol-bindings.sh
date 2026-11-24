#!/usr/bin/env bash
# Regenerates the TypeScript bindings that the Rust crates export with ts-rs.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Run after changing any `#[ts(export)]` type in `li-model`, `li-script`, `li-render-tui`, `li-render-web`, `li-media` or `li-studio`. The
# files land in `packages/protocol/src/generated/` (`TS_RS_EXPORT_DIR` in `.cargo/config.toml`),
# which the Studio UI imports as `@liminal-hq/installer-protocol`. ts-rs writes each file from a
# test named `export_bindings_*`, so this runs only those. Never edit a generated file by hand.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo test --locked --quiet -p li-model -p li-script -p li-render-tui -p li-render-web -p li-studio -p li-media \
	--features li-model/ts,li-script/ts,li-render-tui/ts,li-render-web/ts,li-media/ts export_bindings
echo "Regenerated the protocol bindings."
