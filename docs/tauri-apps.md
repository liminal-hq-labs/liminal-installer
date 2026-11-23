# Installing Tauri apps

> **Status:** proposal. Nothing here is built; the build order at the end is the plan, and the choices to confirm are listed in the pull request that added it.

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

The product's `tauri.conf.json` (with its `tauri.windows.conf.json`, `tauri.macos.conf.json` and `tauri.linux.conf.json` overlays) stays the source. `liminst tauri import` reads it and writes a normalized snapshot, `installer/tauri.json`, with a hash of each source file. Nothing is copied by hand, and nothing is typed twice.

```toml
# installer.toml
[tauri]
config = "../apps/planboard/src-tauri/tauri.conf.json"   # overlays are found beside it
windows = "wrap"        # wrap | native
macos   = "native"
linux   = "native"
```

- `liminst check` fails when a source file's hash differs from the snapshot, so a change to `tauri.conf.json` cannot silently change what ships. `liminst tauri import` accepts the change and shows the diff.
- Imported values fill defaults. A project overrides any of them in `installer.toml`, and the Studio shows which value is imported and which is overridden.
- The snapshot feeds `liminst build` (the right flags and generated pages) and the Studio (a Tauri screen).

### What each field becomes

| Tauri state | Liminal Installer |
|---|---|
| `identifier`, `productName`, `version`, `publisher`, `category`, descriptions, `copyright`, `license`, `homepage`, `icon` | `[product]` fields, the licence page, the receipt, native package metadata |
| `resources`, `externalBin`, `frameworks`, `files` (deb, rpm, appimage, macOS) | A generated `components.toml` section tagged `origin = "tauri"`: copy operations per target triple |
| `fileAssociations` | `file_type` operations: a ProgID on Windows, a shared MIME info file and `MimeType=` on Linux, nothing on macOS (the `.app`'s `Info.plist` already carries them) |
| Deep-link `schemes` | `url_scheme` operations; on Linux the desktop file's `exec_arg` follows Tauri's rule (`%U` with schemes, `%F` with only file types) |
| `nsis.installMode` | `scope` state: `currentUser` is `user`, `perMachine` is `machine`, `both` is `ask`. A `scope` page appears only for `ask`. |
| `nsis.languages`, `displayLanguageSelector` | A `language` page (only when the selector is on) and the string tables |
| `nsis.startMenuFolder` | The `shortcut` operation's folder |
| `webviewInstallMode`, `minimumWebview2Version` | A WebView2 prerequisite step (below) |
| `bundleVCRuntime`, `staticVCRuntime` | A VC++ prerequisite step when the app does not link it statically |
| `deb.depends`, `rpm.depends`, `minimumSystemVersion` | Prerequisite checks: reported on Linux and macOS, enforced in native packages through their own metadata |
| `allowDowngrades` | Engine downgrade policy |
| `createUpdaterArtifacts`, `plugins.updater` | Feed output (below) |
| `signCommand`, `certificateThumbprint`, `digestAlgorithm`, `timestampUrl` | Reused to sign Liminal Installer's own outputs, so one signing setup covers both |

### WebView2 as a plan step

A new operation, `prereq.webview2`, with `mode`, `silent`, `path` and `min_version`, becomes part of the plan like any other operation: it detects (the runtime's version in the registry), installs (runs the bootstrapper, the embedded offline installer, or points the app at a fixed runtime folder) and records what it did in the receipt. The modes carry the same trade-offs Tauri documents, and the Studio shows them next to the installer size they cost:

- `downloadBootstrapper` and `embedBootstrapper`: online, small.
- `offlineInstaller`: no network, about 127 MB larger.
- `fixedRuntime`: no network, about 180 MB larger, and the runtime is ours to update.
- `skip`: no step, and a warning in `check` that the app will not start without the runtime.

Windows 10 and 11 ship WebView2, so on those systems the step finds it and does nothing. A flow can show a `webview2` page only when the runtime is missing or older than the minimum, so most people never see it.

### Wrap or native

Two ways to install, chosen per platform in `[tauri]`:

- **`wrap`** carries Tauri's own installer (the NSIS `-setup.exe` or the MSI) as a payload and runs it silently under Liminal Installer's pages, passing the captured choices. WebView2, hooks, a custom NSIS template, the registry layout, file associations and the Start Menu entry stay exactly as Tauri's tested installer makes them. The cost is that uninstall and repair for the product go through Tauri's uninstaller, so the receipt records an `external` entry (what ran, the uninstaller's registry key, the version) instead of file-by-file operations.
- **`native`** has the engine copy the bundle's files and write the registrations itself, as it does for the example projects. The receipt holds every operation, repair checks hashes, and uninstall is exact. The cost is that Tauri-only behaviour must be reproduced: a product that ships `installerHooks` or an NSIS `template` cannot be installed natively without porting that logic.

**Decision:** both modes are supported on Windows, with `native` as the default. A product opts into `wrap` in `[tauri]` when it depends on logic only Tauri's installer has, such as `installerHooks` or a custom NSIS `template` (Planboard ships both today). Linux and macOS are always `native`: Linux packages are already generated natively, and macOS is a copy of an `.app`. The spike confirmed that the native path can reproduce everything the installer writes except those hooks; see [Can the native path reproduce the registrations?](tauri-windows-spike.md#implications-for-liminal-installer).

### Updates

Not every product Liminal Installer installs is a Tauri app, and not every product wants an in-app updater, so updates are a per-product setting in `installer.toml`: `[updates] mode = "none" | "liminal" | "tauri" | "both"`.

- **`none`:** no feed is written; the engine can still repair and uninstall from the receipt.
- **`liminal`:** the signed feed for the engine, which any product (Tauri or not) can use, and which a product can call through `liminal-engine update`.
- **`tauri`:** for Tauri products only: a `latest.json` and `.sig` files for the Tauri updater, from the same artefacts and URLs. The engine does not update the product, but it still reconciles the receipt.
- **`both`:** both outputs from one build.

The Tauri updater runs the product's own installer and so bypasses our receipt. After an in-app update, the engine reconciles the receipt on the next launch or repair: it compares the installed version with the receipt and, for a `wrap` install, refreshes the `external` entry. The two signing schemes stay separate (minisign for the Tauri updater, Ed25519 for feeds); the Tauri public key comes from the imported `pubkey`.

### Studio

A Tauri screen shows the imported state per platform (identity, scope, languages, WebView2 mode with its size cost, prerequisites, file types, schemes, updater, signing), marks values that are overridden, shows the drift diff and offers Re-import. Flow editing gets the new pages (`scope`, `language`, `webview2`, `prerequisites`) from templates.

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

1. `[tauri]` block, `liminst tauri import`, the snapshot and the drift check in `liminst check`.
2. Identity, resources, file types, schemes, scope and languages feeding generated components and pages, for Linux and macOS (`native`).
3. `prereq.webview2` and `wrap` for Windows, behind the spike results.
4. Dual feed output and receipt reconciliation.
5. The Studio's Tauri screen.
