# Captures registry and file-system state in a Windows guest so two states can be diffed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File guest-snapshot.ps1 -Label before
#
# Writes C:\spike\snap\<Label>.reg.txt and C:\spike\snap\<Label>.files.txt. The registry file holds
# one "KEY<TAB>value line" per value under the keys the spike app's installers write, plus
# the child key names of the places an installer writes to. It deliberately never exports or
# searches a whole hive: a full `reg export` of HKLM\SOFTWARE writes hundreds of megabytes, which
# fills a VM's differencing disk, and a full `reg query /s /f` takes minutes. Run it as the same user that ran
# the installer, and compare two labels with guest-diff.ps1.
param([Parameter(Mandatory = $true)][string]$Label)

$ErrorActionPreference = 'Stop'
$snap = 'C:\spike\snap'
New-Item -ItemType Directory -Force -Path $snap | Out-Null

$lines = [System.Collections.Generic.List[string]]::new()

# Every value under the keys an installer for the spike app is known or expected to write.
$dumped = @(
	'HKCU\Software\Spike Publisher',
	'HKCU\Software\Classes\.spk',
	'HKCU\Software\Classes\SpikeFile',
	'HKCU\Software\Classes\spikeapp',
	'HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\SpikeApp',
	'HKCU\Software\Microsoft\Windows\CurrentVersion\Run',
	'HKLM\SOFTWARE\Spike Publisher',
	'HKLM\SOFTWARE\WOW6432Node\Spike Publisher',
	'HKLM\SOFTWARE\Classes\.spk',
	'HKLM\SOFTWARE\Classes\SpikeFile',
	'HKLM\SOFTWARE\Classes\spikeapp',
	'HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\SpikeApp',
	'HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\SpikeApp',
	'HKLM\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
	'HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
)
foreach ($root in $dumped) {
	$key = ''
	foreach ($line in (cmd /c "reg query `"$root`" /s 2>nul")) {
		if ($line.StartsWith('HKEY')) { $key = $line }
		elseif ($line.Trim().Length -gt 0) { $lines.Add("$key`t$($line.Trim())") }
	}
}

# Child key names of the places an installer writes to, so unexpected new keys show up too.
$listed = @(
	'HKCU:\Software',
	'HKCU:\Software\Classes',
	'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
	'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run',
	'HKLM:\SOFTWARE',
	'HKLM:\SOFTWARE\WOW6432Node',
	'HKLM:\SOFTWARE\Classes',
	'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall',
	'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall',
	'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths'
)
foreach ($k in $listed) {
	if (Test-Path $k) { Get-ChildItem $k -Name -ErrorAction SilentlyContinue | ForEach-Object { $lines.Add("[$k]`tsubkey $_") } }
}
$lines | Sort-Object -Unique | Set-Content -Encoding UTF8 (Join-Path $snap "$Label.reg.txt")

$places = @(
	"$env:LOCALAPPDATA\Programs",
	"$env:LOCALAPPDATA\SpikeApp",
	"$env:ProgramFiles",
	"${env:ProgramFiles(x86)}",
	"$env:APPDATA\Microsoft\Windows\Start Menu",
	"$env:ProgramData\Microsoft\Windows\Start Menu",
	"$env:USERPROFILE\Desktop",
	"$env:PUBLIC\Desktop",
	"$env:APPDATA\ca.liminal.spikeapp",
	"$env:LOCALAPPDATA\ca.liminal.spikeapp"
)
$files = foreach ($p in $places) {
	if (Test-Path $p) {
		Get-ChildItem -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue |
			Where-Object { $_.FullName -notmatch 'Microsoft\\Edge|EdgeWebView|EdgeUpdate|WindowsApps|Windows Defender|Microsoft Visual Studio|dotnet|Common Files|Windows Kits|MSBuild|Reference Assemblies|Windows NT|Internet Explorer|Windows Mail|Windows Media Player|Windows Photo Viewer|Microsoft Update Health|WindowsPowerShell' } |
			ForEach-Object { '{0}  {1}' -f $_.FullName, $(if ($_.PSIsContainer) { '<DIR>' } else { $_.Length }) }
	}
}
$files | Sort-Object | Set-Content -Encoding UTF8 (Join-Path $snap "$Label.files.txt")
'{0}: {1} registry lines, {2} files' -f $Label, (Get-Content (Join-Path $snap "$Label.reg.txt") | Measure-Object).Count, ($files | Measure-Object).Count
