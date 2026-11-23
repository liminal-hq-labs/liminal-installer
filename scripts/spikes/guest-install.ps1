# Runs one installer or uninstaller in a Windows guest between two state snapshots and diffs them.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File guest-install.ps1 -Label cu-silent -Exe C:\spike\in\cu.exe -InstallArgs '/S'
#        ... -Elevated   (launches with the RunAs verb)   or   -Direct   (CreateProcess, no auto-elevation)
#        A `~` in -InstallArgs stands for a space.
#
# Writes C:\spike\out\<Label>.exit (the process exit code), C:\spike\out\<Label>.diff.txt (the
# state difference) and prints both. The caller's own token, for a guest-control session, is a
# filtered, medium-integrity one even for an administrator.
param(
	[Parameter(Mandatory = $true)][string]$Label,
	[Parameter(Mandatory = $true)][string]$Exe,
	[string]$InstallArgs = '',
	[switch]$Elevated,
	[switch]$Direct
)

$ErrorActionPreference = 'Stop'
# `~` stands for a space, so an argument list survives the command lines the host builds.
$InstallArgs = $InstallArgs -replace '~', ' '
$out = 'C:\spike\out'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$exitFile = "$out\$Label.exit"
Remove-Item $exitFile -ErrorAction SilentlyContinue

& C:\spike\guest-snapshot.ps1 -Label "$Label-before" | Out-Null

# `Start-Process -Verb RunAs` is the elevated launch. The dev VM has UAC on with
# ConsentPromptBehaviorAdmin = 0 (elevate without prompting), so it elevates silently; on a stock
# machine it would show the consent prompt. Without -Elevated, `Start-Process` still honours an
# installer's `requireAdministrator` manifest the same way, so use -Direct to run it through
# CreateProcess, which fails with error 740 instead of elevating.
$start = @{ FilePath = $Exe; Wait = $true; PassThru = $true }
if ($InstallArgs) { $start.ArgumentList = $InstallArgs }
if ($Elevated) { $start.Verb = 'RunAs' }
if ($Direct) {
	$p = [System.Diagnostics.Process]::new()
	$p.StartInfo.FileName = $Exe
	$p.StartInfo.Arguments = $InstallArgs
	$p.StartInfo.UseShellExecute = $false
	try { $null = $p.Start(); $p.WaitForExit(); $p.ExitCode | Set-Content $exitFile }
	catch { "direct start failed: $($_.Exception.InnerException.Message) (native error $($_.Exception.InnerException.NativeErrorCode))" | Set-Content $exitFile }
} else {
	try { (Start-Process @start).ExitCode | Set-Content $exitFile }
	catch { "start failed: $($_.Exception.Message)" | Set-Content $exitFile }
}
if (-not (Test-Path $exitFile)) { 'timeout' | Set-Content $exitFile }

& C:\spike\guest-snapshot.ps1 -Label "$Label-after" | Out-Null
& C:\spike\guest-diff.ps1 -A "$Label-before" -B "$Label-after" -Out "$out\$Label.diff.txt"

"exit code: $((Get-Content $exitFile | Select-Object -First 1).Trim())"
Get-Content "$out\$Label.diff.txt"
