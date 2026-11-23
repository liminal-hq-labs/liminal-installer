# Diffs two guest snapshots taken by guest-snapshot.ps1, restricted to lines that matter.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File guest-diff.ps1 -A before -B after [-Out C:\spike\out\x.diff.txt]
#
# Prints "+" for lines only in B and "-" for lines only in A, registry first, then files.
# Registry noise that changes on its own (Edge update timestamps, usage counters) is filtered by
# the `-Ignore` regular expression, which has a sensible default.
param(
	[Parameter(Mandatory = $true)][string]$A,
	[Parameter(Mandatory = $true)][string]$B,
	[string]$Out = '',
	[string]$Ignore = 'EdgeUpdate|Microsoft\\Windows\\CurrentVersion\\(Explorer|Search|Notifications|AppModel|CloudStore|Windows Search|Telemetry|WINEVT|Reliability)|Microsoft\\Windows NT\\CurrentVersion\\(Tracing|Perflib|Schedule)|\\Classes\\Local Settings|Microsoft\\Windows Defender|SOFTWARE\\Microsoft\\Windows\\Windows Error Reporting|BITS|\\Windows Update\\|CapabilityAccessManager|SQMClient|\\SystemCertificates|Cryptography|Microsoft\\Edge'
)

$ErrorActionPreference = 'Stop'
$snap = 'C:\spike\snap'

function Diff-Files([string]$Kind) {
	$a = [System.Collections.Generic.HashSet[string]]::new([System.IO.File]::ReadLines("$snap\$A.$Kind.txt"))
	$b = [System.Collections.Generic.HashSet[string]]::new([System.IO.File]::ReadLines("$snap\$B.$Kind.txt"))
	foreach ($l in $b) { if (-not $a.Contains($l) -and $l -notmatch $Ignore) { "+ $l" } }
	foreach ($l in $a) { if (-not $b.Contains($l) -and $l -notmatch $Ignore) { "- $l" } }
}

$result = @('## registry') + (Diff-Files 'reg' | Sort-Object { $_.Substring(2) }) + @('', '## files') + (Diff-Files 'files' | Sort-Object { $_.Substring(2) })
if ($Out) { $result | Set-Content -Encoding UTF8 $Out } else { $result }
