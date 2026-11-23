# Tauri NSIS installers on Windows: spike results

What a Tauri 2 NSIS setup does on Windows, read from the bundler's template and checked by running cross-built installers in a Windows 11 VM. It answers the open items in [`tauri-apps.md`](tauri-apps.md#what-needs-a-spike) and feeds the `wrap` operation and the `external` receipt entry.

## Table of contents

- [Summary](#summary)
- [Method](#method)
- [Command-line switches](#command-line-switches)
- [Install locations and shortcuts](#install-locations-and-shortcuts)
- [Registry map](#registry-map)
- [What the uninstaller removes](#what-the-uninstaller-removes)
- [WebView2](#webview2)
- [Elevation](#elevation)
- [Exit codes](#exit-codes)
- [Upgrade and downgrade](#upgrade-and-downgrade)
- [MSI](#msi)
- [Implications for Liminal Installer](#implications-for-liminal-installer)
- [Unconfirmed](#unconfirmed)
- [Scripts](#scripts)
- [Setup notes](#setup-notes)

## Summary

- **Switches.** `/S` silent and `/D=` install directory are NSIS built-ins. The Tauri template adds `/P` passive, `/UPDATE`, `/NS` (no shortcuts), `/R` with `/ARGS` (start the app afterwards), and, for `installMode: both`, `/currentuser` and `/allusers`. The uninstaller takes `/S`, `/P`, `/UPDATE` and the NSIS `_?=<dir>`.
- **One Add or Remove Programs key**, named after `productName`, in `HKCU` (currentUser) or `HKLM` (perMachine), or in either for `both`. Its `UninstallString` points at `uninstall.exe` inside the install directory.
- **The bundler registers file types and URL schemes itself**, under `Software\Classes` in the same hive as the install. The deep-link plugin writes the same layout again at runtime, to `HKCU` only.
- **Silent uninstall leaves residue.** `Software\<publisher>\<productName>` and an empty `.<ext>` key stay behind, because the "delete app data" step only runs from a ticked checkbox.
- **WebView2 needs no elevation for a per-user install.** A non-elevated setup installed the runtime per user, in `%LOCALAPPDATA%`, with the embedded bootstrapper and with the offline installer.
- **A silent re-run over an existing install upgrades in place** without running the old uninstaller. `allowDowngrades: false` makes a silent downgrade exit with code 2 and change nothing.
- **A `perMachine` or `both` setup cannot start without elevation.** Started with `CreateProcess` it fails with error 740; started with `ShellExecute` it raises UAC, which this VM approves silently.

## Method

1. **Source of truth.** `installer.nsi`, `utils.nsh` and `FileAssociation.nsh` from `tauri-bundler` 2.10.6 (`src/bundle/windows/nsis/`), and the `tauri-plugin-updater` 2.14.0 and `tauri-plugin-deep-link` 2.6.3 sources. The `installer.nsi` that Tauri CLI 2.12.6 generated for the spike app matched the 2.10.6 template apart from the substituted values (SHA-256 of the template `d00089d3…967d`).
2. **Builds.** A minimal Tauri 2 app (`SpikeApp`, publisher `Spike Publisher`, identifier `ca.liminal.spikeapp`, no plugins but `tauri-plugin-deep-link`) cross-built on Linux with `cargo xwin` and NSIS 3.11 (Debian's `nsis` in a container, behind a `makensis` wrapper). Setups are 1.1 MB, 2.8 MB with the embedded bootstrapper and 299 MB with the offline WebView2 installer. Nothing was built inside the VM.
3. **Verification.** `WinDev2407Eval` (Windows 11 Enterprise Evaluation, UAC on with `ConsentPromptBehaviorAdmin = 0`, so an administrator's elevation is approved without a prompt). Each case took a before snapshot and an after snapshot of the relevant registry keys and file locations and diffed them. Commands ran through `VBoxManage guestcontrol`, whose session holds a filtered, medium-integrity token. The VM was restored to its snapshot afterwards.

## Command-line switches

Verified on the setup, `installMode: currentUser` unless noted. Switches are case-sensitive in the template (`/P`, not `/p`); NSIS matches its own `/S` and `/D=` case-sensitively too.

| Switch | Source | What it does | Verified |
| --- | --- | --- | --- |
| `/S` | NSIS | Silent: no window, no pages. Sections run, so WebView2 is checked and installed, and the desktop and Start menu shortcuts are written. | yes, exit 0 |
| `/D=<dir>` | NSIS | Install directory. Must be the last argument, with no quotes even when the path has spaces. Wins over the remembered directory in `Software\<publisher>\<product>`. | yes (`/S /D=C:\spike\custom` installed there and wrote it to the product key) |
| `/P` | template | Passive: shows the progress page only, skips welcome, licence, directory, Start menu and finish pages, and closes itself. Also writes `Installer Language` (see the registry map). Not silent: a message box would block. | yes, exit 0 |
| `/UPDATE` | template | Update mode. Skips the WebView2 install and shortcut creation, and tells the old uninstaller (when the page runs one) to keep shortcuts. | yes: no shortcuts written, no WebView2 step |
| `/NS` | template | No shortcuts: skips both the Start menu and the desktop shortcut. | yes |
| `/R` | template | In silent and passive runs, starts the installed app when done, as the user, with the arguments after `/ARGS`. Ignored in the GUI, which has a "run" checkbox. | yes (`spike-app.exe` started in the interactive session) |
| `/ARGS <args>` | template | Arguments for the app that `/R` starts. Must follow `/R`. | not run |
| `/currentuser`, `/allusers` | MultiUser (`both` only) | Chooses per-user or per-machine without the mode page. Without either, a silent run follows the elevated state: an administrator gets all users. | yes, both |
| `/NCRC` | NSIS | Skips the integrity check. | not run |

The uninstaller, `uninstall.exe`:

| Switch | What it does | Verified |
| --- | --- | --- |
| `/S` | Silent. Without `_?=` the uninstaller copies itself to `%TEMP%` and runs from there, so the start process returns early; a process-tree wait (PowerShell `Start-Process -Wait`) saw it finish. | yes |
| `/P` | Passive: skips the confirm page, auto-closes. | source only |
| `/UPDATE` | Keeps the shortcuts and the `Run` value. | source only |
| `_?=<dir>` | NSIS: run in place. The uninstaller cannot delete itself or `<dir>`; the caller must. Tauri's own reinstall step uses `/UPDATE`, `/P` and `_?=` this way and then deletes `uninstall.exe` and the directory. | yes, left `uninstall.exe` and the directory |

There is no switch for "delete app data". That choice exists only as a checkbox on the uninstaller's confirm page.

**What the updater passes.** `tauri-plugin-updater` runs the downloaded setup with `ShellExecuteW` and the verb `open`, with `/P /UPDATE /R` (passive mode, the default), `/S /UPDATE /R` (quiet) or `/UPDATE` (basic UI), then `/ARGS <current args>` when the app restarts, then any `installer_args` the product set. Because it is `ShellExecute`, a `requireAdministrator` setup raises UAC.

**Per-user versus per-machine** is fixed at build time by `installMode`, except for `both`, where `/currentuser` and `/allusers` (or the mode page) choose.

## Install locations and shortcuts

| Mode | Install directory | Start menu shortcut | Desktop shortcut |
| --- | --- | --- | --- |
| `currentUser` | `%LOCALAPPDATA%\<productName>` | `%APPDATA%\Microsoft\Windows\Start Menu\Programs\<productName>.lnk` | `%USERPROFILE%\Desktop\<productName>.lnk` |
| `perMachine` | `%ProgramFiles%\<productName>` | `%ProgramData%\Microsoft\Windows\Start Menu\Programs\<productName>.lnk` | `%PUBLIC%\Desktop\<productName>.lnk` |
| `both`, all users | `%ProgramFiles%\<productName>` | as `perMachine` | as `perMachine` |
| `both`, current user | `%LOCALAPPDATA%\Programs\<productName>` | as `currentUser` | as `currentUser` |

Notes.

- `currentUser` and `both` put a per-user install in **different directories** (`%LOCALAPPDATA%\<product>` against `%LOCALAPPDATA%\Programs\<product>`). A `wrap` that guesses the path will be wrong; read it from the Uninstall key.
- The desktop shortcut is written only in silent and passive runs (the GUI offers it on the finish page), and neither shortcut with `/NS` or `/UPDATE`.
- With a Start menu folder configured (`nsis.startMenuFolder`), the shortcut goes into that folder instead. Not run.
- The directory holds the main binary (`<mainBinaryName>.exe`), `uninstall.exe`, resources and external binaries. Shortcuts carry the app's AppUserModelID (the bundle identifier), set through COM on the `.lnk`, not through the registry.
- An install that finds a different `MainBinaryName` value from an earlier version deletes the old binary and retargets the shortcuts. Not run.

## Registry map

All keys use the 64-bit view on x64 (`HKLM\SOFTWARE\Microsoft\...`, not `WOW6432Node`). `<hive>` is `HKCU` for `currentUser` and `both`/current user, `HKLM` for `perMachine` and `both`/all users. "Shared" below means identical in every mode apart from the hive and the paths.

### Add or Remove Programs

`<hive>\Software\Microsoft\Windows\CurrentVersion\Uninstall\<productName>`:

| Value | Type | Content |
| --- | --- | --- |
| `DisplayName` | `REG_SZ` | `productName` |
| `DisplayVersion` | `REG_SZ` | `version`, written as given (prereleases included) |
| `Publisher` | `REG_SZ` | `bundle.publisher`, or the second part of the identifier when unset |
| `DisplayIcon` | `REG_SZ` | the quoted main binary |
| `InstallLocation` | `REG_SZ` | the quoted install directory |
| `UninstallString` | `REG_SZ` | the quoted path to `uninstall.exe` |
| `MainBinaryName` | `REG_SZ` | `<mainBinaryName>.exe`, read by the next install |
| `EstimatedSize` | `REG_DWORD` | size in KB, computed |
| `NoModify`, `NoRepair` | `REG_DWORD` | `1` |
| `URLInfoAbout`, `URLUpdateInfo`, `HelpLink` | `REG_SZ` | `bundle.homepage`, only when set |
| `AllUsers` or `CurrentUser` | `REG_SZ` | `1`; written by `both` only, to remember the choice (`AllUsers` for all users) |

There is no `QuietUninstallString`, `InstallDate` or `Version`.

### Product key

`<hive>\Software\<publisher>\<productName>`: default value is the install directory. It is the "installed here" record the next install and the updater read. A passive run also wrote `Installer Language` (a language ID such as `1033`) under `HKCU`, whatever the install mode, and by the template a GUI run does too; a silent run does not.

### File associations (`bundle.fileAssociations`)

Per extension `ext` with the association `name` (default: the extension) and `description`, written under `<hive>\Software\Classes`:

| Key | Value | Content |
| --- | --- | --- |
| `.<ext>` | default | `<name>` |
| `.<ext>` | `<name>_backup` | the previous default, empty if none |
| `<name>` | default | `<description>` |
| `<name>\DefaultIcon` | default | `"<exe>",0` |
| `<name>\shell` | default | `open` |
| `<name>\shell\open` | default | `Open with <productName>` |
| `<name>\shell\open\command` | default | `"<exe>" "%1"` |

It writes no `OpenWithProgids`, `Capabilities`, `RegisteredApplications` or `App Paths` entry (none appeared in the diffs).

### Deep links (`plugins.deep-link.desktop.schemes`)

The **bundler** writes these, so a setup registers the scheme without the app ever running. Per scheme, under `<hive>\Software\Classes\<scheme>`:

| Key | Value | Content |
| --- | --- | --- |
| `<scheme>` | default | `URL:<bundle identifier> protocol` |
| `<scheme>` | `URL Protocol` | empty |
| `<scheme>\DefaultIcon` | default | `"<exe>",0` |
| `<scheme>\shell\open\command` | default | `"<exe>" "%1"` |

The plugin's `register()` (read from source, not run) writes the same four values at runtime, but always under `HKCU` and with the unquoted-exe icon form `<exe>,0`. In a `perMachine` install a runtime `register()` therefore adds a per-user shadow that takes precedence for that user.

### Other writes

- `HKLM\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` is read (WebView2 present?), not written, by the setup. See [WebView2](#webview2).
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` is touched only on uninstall (the value named `productName` is deleted if present).

## What the uninstaller removes

Verified for each mode with file types and a scheme registered.

| Item | Removed |
| --- | --- |
| Main binary, resources, external binaries, `uninstall.exe`, install directory | yes (with `_?=` the caller removes `uninstall.exe` and the directory) |
| Start menu and desktop shortcuts, pinned entries, jump-list data | yes, unless `/UPDATE`; only when they still point at this install |
| `Uninstall\<productName>` key | yes |
| `Software\Classes\<name>` and `Software\Classes\<scheme>` | yes (a scheme only if its command still points at this install) |
| `Software\Classes\.<ext>` | **no**: the default is restored to the `<name>_backup` value, so the key stays with an empty default and the `_backup` value |
| `Software\<publisher>\<productName>` and an empty `Software\<publisher>` | **no**, unless the "delete app data" checkbox is ticked (not reachable from a silent run) |
| `%APPDATA%\<identifier>` and `%LOCALAPPDATA%\<identifier>` | **no**, same checkbox |
| `HKCU\...\Run` value `<productName>` | yes, unless `/UPDATE` |
| WebView2 | never |

Net residue after a silent uninstall (all three modes): `Software\<publisher>\<productName>` (default = old install directory) and `Software\Classes\.<ext>` with an empty default and `<name>_backup`.

## WebView2

The `WebView2` section runs before the files are copied. It reads the `pv` value of the Evergreen runtime's client key, first in `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}`, then in `HKCU\SOFTWARE\Microsoft\EdgeUpdate\Clients\...`. If it finds a version:

- the section is done, or, when `minimumWebview2Version` is set and the installed version is older, it runs `MicrosoftEdgeUpdate.exe /install appguid={F3017226-...}&needsadmin=true` (source only).

If it finds none, and the run is not `/UPDATE`:

| `webviewInstallMode` | What happens at install time | Extra size |
| --- | --- | --- |
| `downloadBootstrapper` (default) | `NSISdl` fetches `https://go.microsoft.com/fwlink/p/?LinkId=2124703` to `%TEMP%\MicrosoftEdgeWebview2Setup.exe`, then runs it. A failed download aborts the install. | none |
| `embedBootstrapper` | The setup carries the bootstrapper (about 1.7 MB), writes it to `%TEMP%`, and runs it. The bootstrapper still needs the network. | +1.7 MB (2.8 MB setup) |
| `offlineInstaller` | The setup carries the standalone runtime installer, writes it to `%TEMP%`, and runs it. No network. | +298 MB (299 MB setup) |
| `fixedRuntime` | No install step; the bundler ships the runtime with the app. | not run |
| `skip` | No section: the app fails to open if the runtime is missing. | none |

The installer runs the bootstrapper as `<path> /silent /install` (just `/install` when `silent` is false), and treats any exit code but 0 as a failure: `Abort` with the localized "installation aborted" message, which ends the setup with exit code 2 before any app file is written. In an updater build (`nsis` built for the updater) the mode is always `downloadBootstrapper`.

**Evidence.** To make the setup take the install path on a machine that already had WebView2, the machine-level client key was renamed (and later the per-user one deleted) before each run. The VM's system-level runtime was version 155.

| Case | Result |
| --- | --- |
| `embedBootstrapper`, no network | exit 2, nothing installed |
| `downloadBootstrapper`, no network | exit 2, nothing installed |
| `embedBootstrapper`, network, **non-elevated** | exit 0. The runtime installed **per user**: `HKCU\Software\Microsoft\EdgeUpdate\Clients\{F3017226-...}` `pv` 157.0.4341.60, `HKCU\...\Uninstall\Microsoft EdgeWebView`, files in `%LOCALAPPDATA%\Microsoft\EdgeWebView\Application\157.0.4341.60`. No elevation was needed. |
| `offlineInstaller`, no network, **non-elevated** | exit 0, per user: 158.0.4370.33 under `%LOCALAPPDATA%\Microsoft\EdgeWebView`, `HKCU` client and Uninstall keys. |
| `offlineInstaller`, no network, **elevated** | exit 0, **per machine**: `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-...}` `pv` 158.0.4370.33, `HKLM\...\Uninstall\Microsoft EdgeWebView`, files in `%ProgramFiles(x86)%\Microsoft\EdgeWebView\Application`. |

So the answer for per-user installs: the bootstrapper does **not** require elevation. It installs where its own elevation says: per user when the setup is not elevated, per machine when it is. A per-machine setup always runs the bootstrapper elevated, so it installs per machine. On the VM's stock Windows 11 the runtime is already present (it ships with the OS), so the section is skipped and no WebView2 install happens at all.

## Elevation

`RequestExecutionLevel` comes from `installMode`:

| `installMode` | Manifest level | `CreateProcess` without elevation | `ShellExecute` (double-click, updater) |
| --- | --- | --- | --- |
| `currentUser` | `user` | runs, exit 0 | runs |
| `perMachine` | `admin` | **error 740** ("requires elevation") | UAC, then runs |
| `both` | `highest` (MultiUser) | **error 740** for an administrator, even with `/currentuser` | UAC, then runs |

Observed in the VM, silent (`/S`):

| Case | Result |
| --- | --- |
| `currentUser`, non-elevated | installed per user, exit 0 |
| `perMachine`, `CreateProcess` | exit 740, nothing changed |
| `perMachine`, `ShellExecute` | UAC approved silently by the VM policy; installed per machine, exit 0 |
| `perMachine`, elevated | installed per machine, exit 0 |
| `both`, `CreateProcess` | exit 740, with or without `/currentuser` |
| `both`, elevated, no switch | **all users** (`%ProgramFiles%`, `HKLM`, `AllUsers=1`) |
| `both`, elevated, `/allusers` | all users |
| `both`, elevated, `/currentuser` | **current user**, in `%LOCALAPPDATA%\Programs\<product>`, `HKCU`, `CurrentUser=1` |

A `both` setup is elevated even for a per-user install, because its manifest asks for the highest available level. By the MultiUser source, a standard user (no administrator rights) gets no elevation there and `both` falls back to per-user, while a standard-user `perMachine` run needs administrator credentials. Neither was run (the VM has one administrator account; see [Unconfirmed](#unconfirmed)).

The uninstaller is subject to the same rule: the `perMachine` uninstaller needed an elevated launch.

## Exit codes

| Code | Meaning | Seen |
| --- | --- | --- |
| 0 | Success, including a re-run over the same version and a silent downgrade with `allowDowngrades: true` | yes |
| 1 | The user cancelled (NSIS) | no |
| 2 | Script abort: a silent, passive or update downgrade blocked by `allowDowngrades: false`; a failed WebView2 download or install; an app-still-running check that could not close the app (source) | yes (the first two) |
| 740 | Windows refused `CreateProcess`: the setup needs elevation | yes |

A blocked downgrade also writes the localized "silent downgrades" line, in red, to the parent console when the setup is started from one; no message box. In passive and update mode the same condition shows a message box instead (source; not run, because the box would block a non-interactive session).

## Upgrade and downgrade

Observed with `currentUser` setups 1.0.0, 1.1.0 and 0.9.0 (two builds of 0.9.0: `allowDowngrades` true and false). `allowDowngrades` defaults to true.

| Run | Result |
| --- | --- |
| 1.0.0 `/S`, 1.0.0 `/S` again | exit 0, no change |
| 1.1.0 `/S` over 1.0.0 | exit 0. `DisplayVersion` becomes 1.1.0. Files and keys are overwritten in place: no uninstall runs. |
| 0.9.0 (`allowDowngrades: false`) `/S` over 1.1.0 | **exit 2**, nothing changed |
| 0.9.0 (`allowDowngrades: true`) `/S` over 1.1.0 | exit 0, `DisplayVersion` becomes 0.9.0 |
| 1.1.0 `/P` over 1.0.0, 0.9.0 `/P` over 1.1.0, 1.1.0 `/P /UPDATE` over 0.9.0 | all exit 0, in place |

Why: in a silent run NSIS skips every page, including the "already installed" page, so the template's "uninstall first" choice never runs; the only gate is the `.onInit` check that `allowDowngrades: false` enables. The GUI's reinstall page offers the choice and, when upgrading, preselects "uninstall before installing" (source). Passive mode runs that page's leave handler without the page, with nothing selected, so it too proceeds in place, as observed.

## MSI

Skipped. WiX runs only on Windows, so no MSI could be cross-built from this host, and building one in the VM would break the rule against building there. What is known from the bundler source: the NSIS setup detects an earlier WiX install by `DisplayName` and `Publisher` in the `HKLM` Uninstall keys and uninstalls it with `msiexec` before installing (a migration path), and `tauri-plugin-updater` runs an MSI with `msiexec /i <msi> /qb+` (basic), `/quiet` or `/passive`, then `/promptrestart`. The `msiexec /qn` and upgrade-code behaviour is **unconfirmed**.

## Implications for Liminal Installer

### What a `wrap` operation must pass

- **Silent, always:** `/S`. Prefer it to `/P`, which can pop a message box and writes `Installer Language`.
- **Directory:** `/D=<dir>` as the last argument, unquoted. Leave it out to take the mode's default. Read the real directory back from `InstallLocation` after the run rather than trusting the default, because `both` and `currentUser` differ.
- **Scope:** for `both`, always pass `/currentuser` or `/allusers`; without one, an elevated run silently takes all users. For `perMachine`, run elevated; for `currentUser`, do not (HKCU would still be the right one, but there is nothing to gain).
- **Shortcuts:** pass `/NS` when the plan says no shortcuts; otherwise a silent run makes both.
- **Updates:** `/UPDATE` (with `/S`), the same as the Tauri updater. It skips WebView2 and shortcut creation.
- **Restart:** `/R` only when the plan asks to start the app; the app then runs as the user.
- **Exit code:** treat 0 as success, 2 as "refused or aborted" (a blocked downgrade, a WebView2 failure) and anything else as failure. Surface 740 as "needs elevation".
- **Elevation:** start with an elevated launch for `perMachine` and `both`; a plain `CreateProcess` fails with 740. Do not rely on UAC being silent: this VM's policy is not the default.
- **Prerequisite:** a `prereq.webview2` step is only needed when the product's `webviewInstallMode` is `skip`, or when the setup would otherwise hit the network. With `embedBootstrapper` or `offlineInstaller` the setup brings what it needs. A stock Windows 11 already has the runtime.

### What the `external` receipt entry must record

| Field | Value | Where it comes from |
| --- | --- | --- |
| kind | `nsis` (Tauri) | the wrapped artefact |
| uninstall key | `Software\Microsoft\Windows\CurrentVersion\Uninstall\<productName>` | the template (`UNINSTKEY`) |
| hive and scope | `HKCU` = user, `HKLM` = machine; for `both` also the `CurrentUser` or `AllUsers` value | where the key was found |
| uninstaller | the `UninstallString` value, unquoted | the key |
| install directory | `InstallLocation`, unquoted | the key (and the product key's default value) |
| version | `DisplayVersion` | the key |
| publisher key | `Software\<publisher>\<productName>` in the same hive | the template (`MANUPRODUCTKEY`) |
| main binary | `MainBinaryName` | the key |
| registrations | the file-type classes and URL-scheme classes it asked for | the imported config |
| residue after uninstall | the product key, and the empty `.<ext>` keys | this spike |

The uninstall command is `"<uninstaller>" /S`; add `_?=<dir>` to wait for it in place, and then delete `uninstall.exe` and the directory, as Tauri's own reinstall does. A silent uninstall cannot delete the product key or `%APPDATA%\<identifier>`, so Liminal Installer's own undo has to remove them if a clean uninstall is wanted, and must decide whether a user's app data counts.

### Can the native path reproduce the registrations?

Yes, for what was observed. The setup's writes are plain: files, two `.lnk` shortcuts, the Uninstall key, the product key, and the `Software\Classes` trees above. All are operations the engine already has or can add, with a receipt that undoes them exactly (and cleans the residue a Tauri uninstall leaves). Three things are not plain and need work or a decision:

- **Shortcut AppUserModelID** is set through the COM property store; without it, taskbar grouping and jump lists differ.
- **The WebView2 step** needs a prerequisite operation with the same detection (`pv` in `HKLM` WOW64, then `HKCU`) and the bootstrapper or offline installer, so `native` should reuse `prereq.webview2`.
- **`installerHooks` and a custom `template`** are not reproducible without porting them. This is the case the earlier recommendation for `wrap` rests on.

Not observed, so not claimed: whether the setup calls `SHChangeNotify` after writing associations (the NSIS macro defines it; its call site was not checked), and the behaviour of the RestartManager check that closes a running app.

## Unconfirmed

- A **standard user** (no administrator rights): `perMachine` with credentials, and `both` falling back to per-user. The VM has an administrator account only, and creating a second user needs elevation, which the guest-control token does not have.
- A UAC **consent prompt**: the VM approves elevation silently, so the prompt path (and what a setup does if it is declined) was not seen.
- `allowDowngrades: false` in **passive and update** mode (message box), and the `silentDowngrades` text on a console.
- `/ARGS`, `/NCRC`, the **Start menu folder** option, and the `MainBinaryName` migration.
- `fixedRuntime` and `minimumWebview2Version`.
- **GUI install and uninstall**, including "delete app data", the reinstall page's choices and the finish-page shortcut.
- Whether a **running app** is closed by the RestartManager step.
- **Runtime `register()`** of the deep-link plugin (read, not run) and a registered handler actually launching the app.
- **MSI** in every respect.
- Offline, embedded and downloaded WebView2 runs started on a machine with no WebView2 at all. The runtime was hidden from the setup rather than absent, so its Edge Update service and system-level copy were still present.
- Other **languages**, ARM64 and 32-bit x86 (the 32-bit registry view).

## Scripts

All under `scripts/spikes/`. They carry no password; the VM password comes from `VM_PASSWORD` in the environment.

| Script | Where it runs | Purpose |
| --- | --- | --- |
| `build-tauri-variants.sh` | host | Cross-builds every variant with `cargo xwin` and keeps each generated `installer.nsi`. |
| `guest-run.sh` | host | Runs a command, a `.cmd` file or a file copy in the VM through Guest Additions. |
| `guest-snapshot.ps1` | guest | Captures the registry keys and file locations an installer writes. Deliberately not a hive export. |
| `guest-diff.ps1` | guest | Diffs two captures. |
| `guest-install.ps1` | guest | Runs one installer or uninstaller between two captures, with `-Elevated` (RunAs) or `-Direct` (CreateProcess), and records the exit code. |

A case, with `$G` for `guest-run.sh` and the setups copied to `C:\spike\in`:

```sh
export VM_PASSWORD=...
$G --copy-to cu.exe 'C:\spike\in\'
$G 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\spike\guest-install.ps1 -Label cu-S -Exe C:\spike\in\cu.exe -InstallArgs /S'
```

Use `~` for a space inside `-InstallArgs` (`/S~/D=C:\spike\custom`).

## Setup notes

- **Disk.** Booting this VM writes about 11 GB to its differencing disk by itself, and Windows Update traffic (the NAT adapter was up) added several more gigabytes in the first minutes, so the 3 GB budget in the task was not reachable. The link was dropped with `VBoxManage controlvm WinDev2407Eval setlinkstate1 off` for everything but the one WebView2 download case, and the disk finished at 24 GB with `/mnt/games` above 50 GB free. A restore to the snapshot returns the disk to its 2 MB starting size. Do not export or search whole registry hives: an export of `HKLM\SOFTWARE` adds hundreds of megabytes.
- **Password.** The snapshot's `User` password (`WaypointVm-2026!`) has expired by the VM's clock, so a guest-control logon fails with "restricted" until it is changed at the lock screen. For this spike it was changed there, with the owner's permission, to `LiminalSpike-2026!`; the snapshot restore discarded that change. A logged-in session is also required: a guest-control logon without an interactive session has no `HKCU` loaded, and an installer's per-user registry writes silently vanish while its files still land.
- **Output.** `VBoxManage guestcontrol run --wait-stdout` returns output; the MCP `vm_guest_exec` does not. Arguments with spaces break across `VBoxManage`, so put anything with quotes in a `.cmd` file and copy it over.
- **Cleanup.** The host build output (`CARGO_TARGET_DIR`), the scratch directory, the NSIS container image and the Tauri tool cache were removed afterwards.
