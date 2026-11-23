# Installing Tauri apps

> **Status:** built, except where a section says otherwise. `liminst import tauri`, the drift check, `prereq.webview2`, both Windows modes, per-product updates and the Studio's Tauri screen are built and tested against the simulated machine (a simulated Tauri setup and WebView2 installers included); the real machine's registry is still a stand-in, so nothing here has run on Windows yet. The [build order](#build-order) at the end says what is left.

Liminal HQ's own products are Tauri apps, and Tauri already ships installers for them: NSIS and WiX on Windows, `.app` and DMG on macOS, deb, RPM, AppImage and Pacman on Linux, and an updater. Left alone, a person meets two installers, ours and Tauri's, and the choices the product made in `tauri.conf.json` (how WebView2 arrives, per-user or per-machine, languages) live in a second place. This document records what Tauri can do and how Liminal Installer captures that state so there is one installer and one source of truth.

The research is from the Tauri v2 distribution guides, the config reference and the updater and deep-link plugin pages. Items marked **unconfirmed** were not stated there and need a spike on the real platform.

## What Tauri controls

### Windows

| Area | Config (under `bundle.windows`) | Values and behaviour |
|---|---|---|
| WebView2 | `webviewInstallMode` | `downloadBootstrapper` (default, `silent`, no size cost, needs internet), `embedBootstrapper` (+1.8 MB, needs internet, better on Windows 7 MSI), `offlineInstaller` (+127 MB, no internet), `fixedRuntime` (+180 MB, `path` to an extracted runtime, not updated by Windows), `skip` (no check at all) |
| WebView2 floor | `minimumWebview2Version` | If the installed runtime is older, the installer tries to update it |
| Install scope (NSIS) | `nsis.installMode` | `currentUser` (default, `%LOCALAPPDATA%`, HKCU, no admin), `perMachine` (Program Files, HKLM, admin), `both` (person chooses, always elevated) |
| Languages (NSIS) | `nsis.languages`, `displayLanguageSelector`, `customLanguageFiles` | Follows the OS language unless the selector is on |
| Start Menu | `nsis.startMenuFolder` | Optional publisher folder |
| Hooks and template | `nsis.installerHooks`, `nsis.template` | `NSIS_HOOK_PREINSTALL`, `POSTINSTALL`, `PREUNINSTALL`, `POSTUNINSTALL`, `PREFINISHPAGE`; or a whole replacement `.nsi` |
| Look | `nsis.headerImage`, `sidebarImage`, `installerIcon`, `uninstallerIcon`, `compression` | Bitmaps and icons |
| MSI | `wix.*` | `language`, `template`, `fragmentPaths`, `upgradeCode` (must never change), `version` |
| Downgrades | `allowDowngrades` | Default true; NSIS blocks downgrades in silent, passive or update mode when false |
| VC++ runtime | `build.windows.staticVCRuntime`, `bundleVCRuntime` | Linked statically by default; otherwise bundled or installed by a hook |
| Signing | `certificateThumbprint`, `digestAlgorithm`, `timestampUrl`, `tsp`, `signCommand` | `signCommand` (with `%1` for the file) covers EV, Azure Key Vault and Artifact Signing |

### macOS

`bundle.macOS`: `minimumSystemVersion`, `frameworks`, `files`, `entitlements`, `signingIdentity`, `hardenedRuntime`, `exceptionDomain`, `infoPlist`, and `dmg` window layout. The `.app` is the unit; the DMG is a drag-to-Applications window with no install logic. Signing and notarization use `APPLE_*` environment variables. Deep links only work from a bundled app in `/Applications`.

### Linux

`bundle.linux.deb` and `rpm` take `depends`, `recommends`, `provides`, `conflicts`, `replaces` (`obsoletes` in RPM), `files`, `desktopTemplate`, maintainer scripts and `compression`. The default dependencies are `libwebkit2gtk-4.1-0` and `libgtk-3-0` (plus an app indicator library for trays). `pacman` has `depends`, `optdepends`, `install` hooks and a PKGBUILD. `appimage` has `bundleMediaFramework` and `files` under `/usr/`; it has no install step.

### Every platform

`identifier`, `productName`, `version`, `publisher`, `category`, descriptions, `copyright`, `license`, `licenseFile`, `homepage`, `icon`, `resources`, `externalBin` (sidecars, one binary per target triple), `fileAssociations` (extension, MIME type, macOS role and rank, content types, exported types), deep-link `schemes`, `targets` and `createUpdaterArtifacts`.

### The updater

`plugins.updater` takes `pubkey` (minisign) and `endpoints`. A static `latest.json` lists, per `OS-ARCH`, a `url` and the content of the `.sig` file; the whole file is validated first. On Windows the updater runs the same NSIS or MSI installer, with `installMode` `passive`, `basicUi` or `quiet`. On Linux it updates AppImages only. Signature checking cannot be turned off.

## What the person sees today, and what we replace

The two-installer problem is the pages and prompts Tauri's installer adds on its own: the language picker, the per-user or per-machine choice, the install folder, the WebView2 download, the finish page's "Run" checkbox, and an elevation prompt. Liminal Installer replaces these with its own pages bound to the same state, and either runs the Tauri installer silently underneath or does the work itself.

## Capturing the state

### One source of truth

The product's `tauri.conf.json` (with its `tauri.windows.conf.json`, `tauri.macos.conf.json` and `tauri.linux.conf.json` overlays) stays the source. `liminst import tauri <path-to-tauri.conf.json>` reads it and writes a normalized snapshot, `installer/tauri.json`, with a SHA-256 of each source file. Nothing is copied by hand, and nothing is typed twice.

```toml
# installer.toml
[tauri]
config   = "../src-tauri/tauri.conf.json"   # relative to installer/; overlays are found beside it
windows  = "native"      # native (the default) | wrap
override = ["name"]      # [product] fields the project keeps instead of taking the import's
macos    = "native"      # always native; the fields exist so a project can say so
linux    = "native"

[tauri.webview2]         # optional: overrides what tauri.conf.json says
mode        = "offlineInstaller"
silent      = true
path        = "./Microsoft.WebView2.FixedVersionRuntime.131"   # fixedRuntime only
min_version = "110.0.1587.40"

[tauri.wrap]             # optional
installer = "dist/{target}/Plume Notes-setup.exe"   # default: dist/{target}/<productName>-setup.exe
```

What the import does, in one undoable change when it runs from the Studio:

1. **Reads** `tauri.conf.json` (JSON, Tauri 2) and merges `tauri.windows.conf.json` over it for the Windows settings, as Tauri does on Windows (RFC 7396 merge patch). The snapshot holds `productName`, `identifier`, `version`, `mainBinaryName`, `bundle.publisher`, `homepage`, `shortDescription`, `bundle.targets`, `bundle.resources` (an array or a map, normalized to a source and an install-relative destination, with `..` written `_up_` as Tauri does), `externalBin`, `fileAssociations`, the deep-link `schemes`, `bundle.windows` (`webviewInstallMode`, `minimumWebview2Version`, `allowDowngrades`, `nsis.installMode`, `startMenuFolder`, and whether `installerHooks` or a custom `template` exist), `createUpdaterArtifacts` and `plugins.updater` (endpoints, public key, Windows `installMode`).
2. **Writes** `installer/tauri.json`, tab-indented, with the hash of each source file.
3. **Fills** `[tauri] config` (and `windows = "native"` the first time) and `[product]` `id`, `name`, `publisher` and a literal `version` in `installer.toml`, keeping comments and alignment. A field named in `override` is left alone, and so is a version written as `{ from = "git-tag" }`. The import needs a project to fill: run `liminst init` first.
4. **Adds** `[updates] mode = "tauri"` when the app has an updater with endpoints and the project has no `[updates]`.
5. **Replaces** the generated block in `components.toml`, between `# >>> liminst import tauri` and `# <<< liminst import tauri`. It holds components tagged `origin = "tauri"`: one for the Windows targets and one for the rest when the project has both (`tauri-app-windows` and `tauri-app`), otherwise one `tauri-app`. Each copies the main binary, the resources and the sidecars from `dist/{target}/`, adds a Start Menu shortcut (and a desktop one on Windows), and carries the file types and URL schemes.

Running it again is how a change is accepted: it prints which snapshot values changed. Anything outside the generated block is the project's and is never touched.

- `liminst check` fails with `tauri-drift` when a source file's hash differs from the snapshot, when an overlay appeared or went away, and with `tauri-not-imported` or `tauri-config-missing` when the snapshot or the config is not there. A change to `tauri.conf.json` cannot silently change what ships.
- Imported values fill defaults. A project overrides `[product]` fields with `override` and WebView2 with `[tauri.webview2]`, and the Studio shows which value is imported and which is overridden.
- The snapshot feeds the plan (WebView2, the Windows registrations, the wrapped setup's name) and the Studio's Tauri screen.

### What each field becomes

| Tauri state | Liminal Installer | Built |
|---|---|---|
| `identifier`, `productName`, `version`, `publisher` | `[product]` `id`, `name`, `version`, `publisher` (fill, unless overridden) | yes |
| `homepage`, `shortDescription` | The snapshot; `homepage` becomes the Add or Remove Programs links on a native Windows install | yes |
| `category`, `copyright`, `license`, `icon` | Not read yet | no |
| `resources`, `externalBin` | The generated components' copy rules, per target | yes |
| `frameworks`, `files` (deb, rpm, appimage, macOS) | Not read yet | no |
| `fileAssociations` | `file_types` on Linux and macOS targets; on a native Windows install, the `Software\Classes` registry values the setup writes | yes |
| Deep-link `schemes` | `url_schemes` on Linux and macOS; on a native Windows install, the `Software\Classes\<scheme>` values | yes |
| `nsis.installMode` | Which hive and which switches `wrap` uses (`currentUser` is `HKCU`, `perMachine` is `HKLM`, `both` follows the install's scope and writes `CurrentUser` or `AllUsers`) | yes |
| `nsis.languages`, `displayLanguageSelector` | A `language` page | no |
| `nsis.startMenuFolder` | Recorded in the snapshot; the shortcut still goes to the Start Menu root | no |
| `webviewInstallMode`, `minimumWebview2Version` | `prereq.webview2` (below) | yes |
| `bundleVCRuntime`, `staticVCRuntime` | A VC++ prerequisite step | no |
| `deb.depends`, `rpm.depends`, `minimumSystemVersion` | Prerequisite checks | no |
| `allowDowngrades` | Recorded; the Tauri setup enforces it (exit 2) | partly |
| `createUpdaterArtifacts`, `plugins.updater` | Update ownership and the conflict check (below); the `latest.json` output is not written | partly |
| `signCommand`, `certificateThumbprint`, `digestAlgorithm`, `timestampUrl` | Reuse for Liminal Installer's own outputs | no |

### WebView2 as a plan step

`prereq.webview2` is a plan operation on Windows targets of a Tauri product, placed before the files so the runtime is there when the app is. Its fields are `mode`, `silent`, `path` and `min_version`, plus what the plan staged (`installer`, `source`) and what the step recorded (`found`, `ran`). The mode and its extras come from `tauri.conf.json`, with `[tauri.webview2]` on top.

**Detection.** The runtime's version is the `pv` value of the Evergreen client key `{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}`, read first in `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\…` and then in `HKCU\SOFTWARE\Microsoft\EdgeUpdate\Clients\…`, the order Tauri's NSIS template uses. A missing, empty or `0.0.0.0` value is "not installed". With `min_version` set, an older runtime counts as missing (versions compare field by field as numbers). Windows 10 and 11 ship the runtime, so most machines find it and the step does nothing.

**Install**, when the runtime is missing, by mode:

| Mode | The plan stages | Extra size | Network |
|---|---|---|---|
| `downloadBootstrapper` (default) | Nothing; the step downloads `https://go.microsoft.com/fwlink/p/?LinkId=2124703` with `curl` to `{temp}` | none | needed |
| `embedBootstrapper` | `dist/webview2/MicrosoftEdgeWebview2Setup.exe` from the payload, copied to `{temp}` | +1.8 MB | needed |
| `offlineInstaller` | `dist/webview2/MicrosoftEdgeWebView2RuntimeInstallerX64.exe` (from the standalone installer, `https://go.microsoft.com/fwlink/?linkid=2124701`) | +127 MB | not needed |
| `fixedRuntime` | The runtime folder `dist/{target}/<folder>/`, copied to `{install}/<folder>`; the step checks it is there and installs nothing | +180 MB | not needed |
| `skip` | No step at all; `check` warns that the app will not start without the runtime | none | not needed |

The bootstrapper and the offline installer run as `<exe> /silent /install` (just `/install` when `silent` is false). Any other exit code than 0 fails the step and the install rolls back. The step then reads the key again, and fails if the runtime is still not there. A non-elevated run installs the runtime per user (`HKCU`), as the spike found; an elevated one, or one that finds an older machine-wide runtime, updates it per machine. Uninstall never removes WebView2, which other apps share. `liminst build` puts the staged files in the `.lip`, and `check` reports `tauri-webview2-installer` when the file a mode carries is not in the project.

**Dry runs.** The plan carries the step with `found` filled when the simulated machine already has a usable runtime (`SimMachine::with_webview2`), and the Studio shows both outcomes for the chosen mode: what the step does on a machine without the runtime and on one with it. The simulated machine answers for `curl`, the bootstrapper and the offline installer (`offline()` makes the network fail, `elevated()` picks the hive), so the whole step runs in tests.

### Wrap or native

Two ways to install on Windows, chosen in `[tauri] windows`. Linux and macOS are always native.

- **`native`** (the default) has the engine copy the bundle's files and write the registrations the NSIS setup would have written, as it does for the other examples. The receipt holds every operation, repair checks hashes, and uninstall is exact. Add or Remove Programs and the Tauri updater see one app because the values are the setup's own, from the spike's [registry map](tauri-windows-spike.md#registry-map): the `Uninstall\<productName>` key (`DisplayName`, `DisplayVersion`, `Publisher`, `DisplayIcon`, `InstallLocation`, `MainBinaryName`, `EstimatedSize`, `NoModify`, `NoRepair`, the homepage links, and `CurrentUser` or `AllUsers` for `installMode: both`), the publisher and product key, and the file-type and URL-scheme classes. `UninstallString` is the one value that differs: it runs the engine's own uninstaller, `{install}\.liminal\runner\liminal-engine.exe uninstall`. One list of values (`registry_writes`) feeds both this plan and the simulated setup, and a test compares what the two write. The values are `registry.set` operations, so uninstall removes every one, including the product key and the `.ext` keys a silent Tauri uninstall leaves behind. The hive follows the install's scope: `HKCU` for a user install and `HKLM` for a machine one. The cost is that Tauri-only behaviour must be reproduced: an app with `installerHooks` or a custom NSIS `template` cannot be installed natively without porting that logic, and `check` warns (`tauri-native-hooks`). Not reproduced: the shortcut's AppUserModelID, which affects taskbar grouping and jump lists.
- **`wrap`** carries Tauri's own installer (the NSIS `-setup.exe`) as a payload file and runs it under Liminal Installer's pages. The plan is `prereq.webview2`, a copy of the setup to `{temp}`, then `tauri.wrap`. The engine runs `setup.exe /S [/UPDATE] [/currentuser|/allusers] [extra] /D=<install dir>`:
  - `/S` always, and never `/P`, which can raise a message box;
  - `/UPDATE` when the product's Uninstall key already exists, which is how the Tauri updater runs it and which skips the WebView2 step and shortcuts;
  - `/currentuser` or `/allusers` for `installMode: both`, because an elevated run without one silently picks all users;
  - `/D=` last and unquoted, as NSIS requires.
  The exit code is read as the spike found it: 0 is success, 2 is "stopped without installing" (a blocked downgrade, or WebView2 could not be installed), 740 is "needs elevation", anything else is a failure. After a success the engine reads the setup's Uninstall key (the hive it was told, then the other) and records an `external` entry in the receipt, inside the `tauri.wrap` operation: kind `nsis`, hive, key, uninstaller, install location, version, main binary and the publisher key. Uninstall runs `"<uninstaller>" /S _?=<install location>` so it can wait for it, then deletes `uninstall.exe` and the publisher and product key the silent uninstall leaves, and finally removes the directory. A failure after the setup ran uninstalls it as part of the rollback. A new build of the setup in the payload is a change the next update reruns (the operation records the setup's hash); the same payload is left alone. `launch` starts the recorded main binary, since a wrapped install has no shortcut of ours. Each target keeps its own mode: `wrap` changes Windows only.

**Decision:** both modes are supported on Windows, with `native` as the default. A product opts into `wrap` when it depends on logic only Tauri's installer has, such as `installerHooks` or a custom NSIS `template` (Planboard ships both today). The spike confirmed that the native path can reproduce everything the installer writes except those hooks; see [Can the native path reproduce the registrations?](tauri-windows-spike.md#implications-for-liminal-installer).

**Elevation.** The engine's machine does not elevate by itself yet. A `perMachine` or `both` setup exits 740 when the engine is not elevated, even for a per-user choice (the spike's finding), so `wrap` needs the runner to be started elevated for those two modes; the error says so. A `currentUser` build and the whole `native` mode need none.

### Updates

Not every product Liminal Installer installs is a Tauri app, and not every product wants an in-app updater, so updates are a per-product setting in `installer.toml`:

```toml
[updates]
mode                 = "both"   # none | liminal | tauri | both
feed                 = "https://example.org/plume/feed.json"
channel              = "stable"
check_interval_hours = 24       # at launch; 0 or absent checks every time
```

- **`none`:** no feed; the engine can still repair and uninstall from the receipt.
- **`liminal`** (the default when the section is absent): the installer owns updates, from its signed feed. Works for any product.
- **`tauri`:** the installed app updates itself with the Tauri updater. The installer never applies a feed update (`update` refuses with a message, `find_update` finds nothing and `launch` offers none), but it still catches the receipt up.
- **`both`:** the installer applies feed updates and the app may also update itself.

`feed` and `channel` are what the receipt remembers for later updates (an install's `--feed` and `--channel` win). The receipt records the whole `[updates]` section, so `launch`, `update` and `repair` follow what the product said at install time, and a newer release's own `[updates]` replaces it.

**The conflict rule.** The version on the machine wins:

1. Two updaters that own the same files will replace each other's work, so `check` warns (`updates-conflict`) when the app has the Tauri updater on and the mode says the installer alone owns updates (`liminal` or `none`). The way out is `both` or `tauri`, or turning the updater off in `tauri.conf.json`. The reverse (`tauri` or `both` with no updater configured) warns with `updates-no-updater`, and `tauri` or `both` without a `[tauri]` section is an error (`updates-needs-tauri`).
2. In `both`, the installer applies a feed release only if it is newer than what is installed, and never installs over a newer version.
3. The Tauri updater runs the product's own installer and so bypasses the receipt. Before `launch`, `update` and `repair`, the engine compares the version in the product's Add or Remove Programs key with the receipt. If they differ it moves the receipt to the installed version: for a `wrap` install it reads the `external` entry again, and for a native one it drops each copy's hash, so a repair restores missing files but never puts the old content back over the app's update. Native installs also warn (`updates-native-updater`) that Tauri's setup runs over the engine's files and leaves its own `uninstall.exe` behind.
4. The check interval is kept in `{install}/.liminal/last-update-check` and gates only the launch-time check; `liminal-engine update` always checks.

The two signing schemes stay separate (minisign for the Tauri updater, Ed25519 for feeds); the Tauri public key is in the snapshot. Writing `latest.json` and `.sig` files from the same artefacts is not built.

### Studio

The Tauri screen (the sixth in the main window) shows the project's `[tauri]` state: the config path, whether it was found and imported, and **Import tauri.conf.json…** (the platform's file picker) and **Re-import**; the drift (the files that changed, were added or went away, and the values a re-import would change as a from and to table); the product fields with imported, project and overridden values and a switch to keep the project's; the Windows mode (native or wrap, with a note when the app has hooks or a template); the WebView2 mode with its size, network need, what it carries, whether it is imported or overridden, and what the step does with and without the runtime; the update settings with the conflict rule written out; and the Tauri and update findings from the engine's checks. Every change is one entry in the undo history (`DocKey` `tauri`: `installer.toml`, `components.toml` and `tauri.json` together). The Dry-run screen plans a Windows target the same way: the WebView2 step and either the registrations or the wrapped setup.

The MCP tools are `get_tauri`, `import_tauri` (`config`, `label`) and `update_tauri_settings` (`windows`, `overrides`, `webview2`, `updates`, `label`). Flow editing gets the new pages (`scope`, `language`, `webview2`, `prerequisites`) from templates only once those pages exist; they are not built.

## What the Windows spike found

The Windows installer was run in a VM and the registry diffed before and after; the full results are in [`tauri-windows-spike.md`](tauri-windows-spike.md). In short:

- **Switches:** `/S` and `/D=` are NSIS built-ins; the template adds `/P`, `/UPDATE`, `/NS`, `/R` and, for `both`, `/currentuser` and `/allusers`.
- **Registry:** one Uninstall key named after the product, a publisher and product key holding the install directory, and file-type and URL-scheme classes under `Software\Classes`. A silent uninstall leaves the product key and an empty extension key behind.
- **WebView2:** a non-elevated setup installs the runtime per user.
- **Elevation:** `perMachine` and `both` exit with 740 without it.
- **Exit codes:** 0 success, 2 aborted, 740 needs elevation.
- **Upgrades:** a silent re-run upgrades in place, and `allowDowngrades: false` makes a silent downgrade exit 2.

Still open: MSI, a standard user without admin rights, and the interactive pages.

## Build order

1. Built: the `[tauri]` section, `liminst import tauri`, the snapshot and the drift check in `liminst check`.
2. Partly built: identity, resources, sidecars, file types and schemes feed generated components for every target, and the Windows registrations feed the native plan. Not built: `scope` and `language` pages, the Start Menu folder, frameworks and `files` on macOS and Linux, the package dependency checks.
3. Built and tested against the simulated machine: `prereq.webview2` and both Windows modes. Not built: the real machine's registry (today a JSON file under the user data directory, so on Windows detection and the setup's uninstall key would not be read from the real registry), the elevation helper, the shortcut's AppUserModelID, and MSI. These are the work before a run on a real Windows machine.
4. Partly built: per-product updates and the receipt catch-up. Not built: the dual feed output (`latest.json`).
5. Built: the Studio's Tauri screen and the MCP tools.
