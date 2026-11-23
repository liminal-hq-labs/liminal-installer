#!/usr/bin/env bash
# Sends keys or text to the running Windows 95 test bed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: type-keys.sh [--text STRING | --key KEYNAME]...
#        type-keys.sh KEYNAME...                      (each argument is one xdotool key name)
#
# Key names are xdotool's: Return, Escape, Tab, alt+F4, ctrl+Escape (the Start menu), super is not used. Examples:
#   type-keys.sh ctrl+Escape u Return            shut down from the Start menu
#   type-keys.sh --text 'C:\TEST.EXE' --key Return
# Environment: W95_DISPLAY (default :95); see lib.sh.
set -euo pipefail
# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
w95_need xdotool
[ "$#" -gt 0 ] || w95_die "usage: type-keys.sh [--text STRING | --key KEYNAME]... | KEYNAME..."
while [ "$#" -gt 0 ]; do
	case "$1" in
	--text)
		w95_type "${2?--text needs a string}"
		shift 2
		;;
	--key)
		w95_key "${2?--key needs a key name}"
		shift 2
		;;
	*)
		w95_key "$1"
		shift
		;;
	esac
done
