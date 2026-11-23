#!/usr/bin/env bash
# Runs a command line in the Windows 11 dev VM through Guest Additions and prints its output.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

# Usage: VM_PASSWORD=... scripts/spikes/guest-run.sh 'cmd.exe command line'
#        VM_PASSWORD=... scripts/spikes/guest-run.sh --script FILE.cmd   (copies it to C:\spike\run.cmd and runs it; use for quotes and spaces)
#        VM_PASSWORD=... scripts/spikes/guest-run.sh --copy-to HOST_FILE 'C:\guest\dir\'
#        VM_PASSWORD=... scripts/spikes/guest-run.sh --copy-from 'C:\guest\file' HOST_FILE
#
# `VM_NAME` (default WinDev2407Eval) and `VM_USER` (default User) pick the guest. The password
# is only ever read from the environment. The guest session is a standard, non-elevated token
# even for an administrator, which the spike relies on; `guest-elevated.ps1` has the elevated
# pattern. `GT` is the timeout in seconds (default 120).
vm="${VM_NAME:-WinDev2407Eval}"
user="${VM_USER:-User}"
: "${VM_PASSWORD:?set VM_PASSWORD}"

case "${1:-}" in
--script)
	VBoxManage guestcontrol "$vm" copyto --username "$user" --password "$VM_PASSWORD" --target-directory "C:\\spike\\" "$2" >/dev/null
	exec timeout "${GT:-120}" VBoxManage guestcontrol "$vm" run --username "$user" --password "$VM_PASSWORD" \
		--wait-stdout --wait-stderr --exe 'C:\Windows\System32\cmd.exe' -- cmd.exe /c "C:\\spike\\$(basename "$2")"
	;;
--copy-to)
	exec VBoxManage guestcontrol "$vm" copyto --username "$user" --password "$VM_PASSWORD" --target-directory "$3" "$2"
	;;
--copy-from)
	exec VBoxManage guestcontrol "$vm" copyfrom --username "$user" --password "$VM_PASSWORD" --target-directory "$3" "$2"
	;;
esac

exec timeout "${GT:-120}" VBoxManage guestcontrol "$vm" run --username "$user" --password "$VM_PASSWORD" \
	--wait-stdout --wait-stderr --exe 'C:\Windows\System32\cmd.exe' -- cmd.exe /c "$*"
