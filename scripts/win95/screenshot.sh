#!/usr/bin/env bash
# Takes a screenshot of the running Windows 95 test bed (the DOSBox-X window) as a PNG.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: screenshot.sh OUT.png
#
# Environment: W95_DISPLAY (default :95), W95_HOME; see lib.sh. Run it while build-base.sh or run-win95.sh has the guest up.
set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
OUT="${1:?usage: screenshot.sh OUT.png}"
w95_need xdotool import
w95_shot "$OUT"
echo "$OUT"
