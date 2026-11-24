I found no media UI in Studio, and no build or export UI of any kind. The Preferences runner text says so directly: "Builds from the Studio are not available yet." The plan below adds a Media screen in about 12 stacked Studio PRs. It recommends one change to the installer plan: M5 should put the media planning and build code in a new shared library crate (`li-media`) instead of the CLI, so the CLI, Studio and MCP all call the same code.

Tags: **[V]** I read it in the repo, **[U]** not checked.

## 1. Audit

**What Studio has today**:

- **Screens [V]:** Project, Flow, Layouts, Scripts, Dry-run, Tauri. They are listed in `apps/studio/src/app/router.tsx` and `screens.ts`, and the `Screen` union in `src/chrome/menuModel.ts` has those six values only. No route, store, message or component mentions media, floppy, ISO, image or build.
- **Outputs [V]:** the only place they appear is a plain text column on the Project screen. `ProjectScreen.tsx` prints `target.outputs.join(' · ')` from `TargetInfo.outputs`.
- **Builds [V]:** Preferences > Tools has a Runner path, and its text (`prefs.runner.desc` in `messages.ts`) ends "Builds from the Studio are not available yet."
- **Tauri commands [V]:** `src-tauri/src/lib.rs` registers project, flow, layout, script, preview, TUI, Tauri-import, history, MCP-token, `new_project`, `app_info` and `reveal`. Nothing builds anything.
- **MCP tools [V]:** `crates/li-studio/src/mcp.rs` lines 35–52 list 18 tools: get/list project and targets, flows, layouts, scripts, previews, TUI sessions, the Tauri tools and `check_project`. There is no build, size or media tool, and no progress notifications. Tool calls run one request at a time through `spawn_blocking`.
- **Crate links [V]:** `li-studio` depends on `li-model`, `li-engine`, `li-retro`, `li-render-*` and `li-script`, but not on `li-package`, `li-retro-build` or `li-lpk`. Studio cannot build an image without a new dependency.
- **Dry-run [V]:** it draws the flow's layouts. `sim.ts` has a sample `A:` floppy drive for the free-space meter, but nothing about the medium the installer itself runs from. The DOS, Win16 and Win32 runtimes' own built-in pages are not previewed.

**What the CLI has today**:

- **Build output [V]:** `liminst build -t <target>` writes `<root>/build/<target>/` (`cli.rs` around line 393). It prints one line per `BuiltArtefact {kind, path, note}`, a type that lives in `li-cli/src/build.rs`, not in a library Studio can use.
- **Retro targets [V]:** `li-cli/src/dos.rs` `build_for` writes `SETUP.EXE`, `SETUP.LPK` and `README.TXT` into a loose folder, plus `zip` and `floppy-1440`. A payload that does not fit is reported as a `floppy-1440` line with the note "not built: …" and the exit code is still 0.
- **Floppy writer on `main` [V]:** `li-package/src/dos.rs` writes the root directory only, with a label derived from the slug.
- **M1, PR #38 [V]:** adds `floppy_1440_labelled(label, serial, files)`, `check_label` (11 characters at most), `clusters_needed`, `free_bytes`, `boot_label_and_serial` and folders.
- **M2, PR #39 [V]:** adds the `common.lpk` golden and the reader check.
- **Not built yet:** `[[media]]`, `--media`, `SIZES.txt`, the ISO writer and `AUTORUN.INF`.

**Gap:** there is no way to see, edit, size, build, test or release a medium in Studio, and no shared library for it. The CLI's sizes would be text only (`SIZES.txt`, from M5).

**Useful fact [V]:** in `.LPK` a file's data is its own run of chunks (`lpk-format.md`, "Chunks"). Each file's packed size can therefore be attributed exactly, and so can what excluding a component frees, apart from cluster rounding.

## 2. Media screen

**Where it goes**:

- New `Screen` value `'media'`, path `/media`, View menu item "Media" with `Ctrl+7`, and a new `floppy` icon in `src/ui/iconPaths.ts`.
- New files in the existing Tauri-screen pattern:
  - `src/screens/MediaScreen.tsx`, `MediaScreen.module.css`
  - `src/screens/media/` with `MediaList`, `CapacityMeter`, `DiskMap`, `SizesTable`, `TargetsCard`, `LabelCard`, `ReadmePreview`, `AutorunPreview`, `BuildPanel`
  - `src/stores/media.ts`, `src/lib/media.ts`, `src/lib/demoMedia.ts`, `src/lib/mediaFixture.ts`, `src/app/mediaScreen.test.tsx`

**Media list (left column)**:

- One row per `[[media]]` entry: kind icon, id, label, a "fits" pill (ok / warning / error, the same pills as the Project screen), free bytes, and "2 disks" when it spans.
- "Add floppy" and "Add CD" create an entry with every retro target. Defaults: DOS target at the root, Windows 3.x in `WIN31`, Windows 95 in `WIN95`; label is the slug in upper case, cut to 11 characters; `common = "auto"`; `overflow = "error"`; `reserve` 16 KiB; `autorun` is the Win32 target, on CDs only.
- Remove asks for confirmation in the existing confirm child window. Every edit is one undoable history entry, the same as layout edits.

**Capacity meter (top of the detail pane)**:

- Shows used, free and the reserve band, plus a second thin bar: "Without sharing: needs 1,705,472 — over by 247,808". That makes the bytes saved by `COMMON.LPK` visible.
- Numbers come from the engine's plan in bytes, shown with a size formatter in `src/lib/format.ts`.
- For a CD: capacity 650 MB, shown as a plain bar. The Joliet label and size are shown too.

**Tabs**:

1. **Disk map:** a tree of the image. Each row has the name, a role (`SETUP.EXE`, own pack, common pack, README, AUTORUN, extra), size on disk in whole clusters (512 bytes on a floppy, 2048 on a CD) and its volume. Expanding `SETUP.LPK` or `COMMON.LPK` lists its entries (`C0000001`…) with the destination name for each target. Picking a row fills a detail side panel: raw size, packed size, chunk size, which targets use it, its component.
2. **Sizes:** a per-component table (raw, packed, which pack, which targets, which disk) plus a per-file table, both sortable. Each optional component has an "On this medium" switch. Its note shows what the switch changes, worked out by the engine, for example "leaving it off frees 418,816" or "putting it back needs 421,376 (does not fit)". Free space updates as soon as the switch changes: the switch writes `exclude` (undoable) and the plan is fetched again.
3. **Targets:** one row per target with its folder (8.3, empty means the root), whether it is used, and its `SETUP.EXE` source ("found at …" or "estimated: no runtime"). The folder field validates as you type.
4. **Label and README:**
   - Volume label, at most 11 characters, upper case, FAT characters only; shows a live "7 / 11" count.
   - Serial: `auto` (worked out from the version) or 8 hex digits shown as `XXXX-XXXX`.
   - CD only: the ISO volume id (d-characters) and the Joliet label (16 characters at most).
   - README: the generated or chosen text in a VT323 76-column box with a CRLF and width check, plus "Use my own file…" to set `readme`.
5. **Autorun** (CD only): the `AUTORUN.INF` text, the target it opens and its icon. Disabled with a note when no Win32 target is on the medium.

**Spanning** (`overflow = "span"`, once M8 lands): the disk map splits into "Disk 1 — QUILL" and "Disk 2 — QUILL 2" columns, with their own meters. A note names the components that trigger the disk-2 prompt ("Only people who tick More dictionaries are asked for disk 2").

**Warnings** (from the engine, as `Problem`s with a code so the UI can link to a tab):

| Code | What it says | Links to |
|---|---|---|
| `does_not_fit` | the overrun, with the largest files listed | Sizes |
| `below_reserve` | less than the reserve is free | Capacity |
| `default_on_later_disk` | a component that is ticked by default lands on disk 2 | Sizes |
| `win16_window` | "`COMMON.LPK` uses 4 KiB chunks because Windows 3.x reads it; that costs about N bytes against 32 KiB" (the plan works out both) | — |
| `label` / `dir_not_8_3` | the label or folder breaks the 8.3 or 11-character rules | Label / Targets |
| `autorun_target` | `autorun` names a target that is not on the medium | Autorun |
| `runtime_estimated` | no `SETUP.EXE` was found, so its budgeted size is used | Targets |
| `runtime_lacks` | the runtime is older than a feature the medium needs: spanning (M8) or the redirect page (M6) | Targets |
| `name_83` | a destination path does not suit DOS or Win16 | Disk map |

I read "imports" in your list as `runtime_lacks` **[U]**. If you meant the Windows 3.x `WinExec` import check, it fits as one more code.

## 3. Actions (`BuildPanel`)

- **Build:**
  - The "Build" button (and "Build all") starts a background job and shows phases: compile per target, de-duplicate, pack, lay out, write the image, write the ISO, zip, `SIZES.txt`.
  - A progress bar, a log that scrolls with `aria-live="polite"` (throttled), and Cancel.
  - Errors are shown as `{kind, message}`, and a `does_not_fit` result links to Sizes.
  - Output goes to `<root>/build/media-<id>/`, the same folder the CLI uses.
- **Results:** one row per artefact (`.img`, `.iso`, `.zip`, `SIZES.txt`, the loose folder) with its size and SHA-256. Buttons: "Reveal" (the existing `tools::reveal`) and "Copy SHA-256" (the clipboard, as Connect an agent already does).
- **Write to a physical floppy:** defer (section 9). When it is built, it opens a child window `write-floppy`:
  - lists removable devices only: Linux `/sys/block/*/removable == 1` and size 1,474,560, plus USB floppies;
  - never picks one for you;
  - shows the model, size and whether it is mounted;
  - asks you to type the device name and the volume label before "Erase and write" becomes enabled;
  - runs `pkexec dd`, then reads the disk back and checks its SHA-256.
- **Burn to disc:** "Reveal ISO" and "Open with…" (the system's default app for `.iso`). No burner is built in.
- **Test in an emulator:**
  - New Tools paths for DOSBox Staging and DOSBox-X, checked with `--version` the same way the runner path is today.
  - DOS: run `dosbox-staging` with a throwaway `C:` folder in the app cache, `-c "imgmount a <img> -t floppy" -c "a:" -c "setup"`; for a CD, `imgmount d <iso> -t iso` and then `D:\SETUP`.
  - Windows 3.x and 95: offered only when a checkout harness is found (`scripts/win95/play-win95.sh` plus a base image in `W95_HOME`, `retro/win16/tools/e2e-w31.sh`).
  - Today `play-win95.sh` only has `-d DIR` **[V]**. The `-a IMG` and `-i ISO` options are the media plan's own item, so until then the Studio copies the tree onto D:.
- **Release:**
  - "Open release workflow" opens `.github/workflows/release.yml` in the project when it exists; otherwise the button is hidden.
  - A checklist card: the version matches the project, every expected asset is present, and `SHA256SUMS` is written by the build job.

## 4. Dry-run preview of a medium

- **New controls:** the Dry-run toolbar gets "Run from: none / floppy (id) / CD (id)". For a retro target it also gets "Started on: DOS / Windows 3.x DOS box / Windows 95 DOS box / Windows 3.x / Windows 95". The choices live in the preview store and are passed to the preview as `media: Option<MediaContext {media_id, host}>`.
- **New steps (`kind: "media"`)** that `li-studio` adds to the walked flow:
  - `redirect` before the first page when the host is the wrong OS for the target. DOS gets the text page ("Windows 95 is running… Press D…"); Win16 on Windows 95 gets a message box with Yes and No.
  - `insert-disk` inside the copy step when the chosen components reach volume 2. The copy meter pauses at the byte where the disk changes, using `MediaPlan.volume_boundaries`, then shows the Retry/Cancel dialog; choosing Retry carries on.
  - A `common-missing` test toggle shows the set-id error page.
- **Shared text:** the wording sits in one new `li-retro` module (`li-retro::media_text`) that the runtimes (M6, M8) and the Studio both use, so they cannot drift. Each step's `layout_tree` is built from it as a `dialog` with text and buttons, so it draws in all looks. Text from `li-retro`, like the flow-validation messages, is not in the Studio catalogue.

## 5. Engine, CLI, Tauri and MCP

**Where the code lives: new `crates/li-media`**

- Depends on `li-model`, `li-retro-build`, `li-package` and `li-lpk`, with an optional `ts` feature like `li-model`. No UI dependency.
- `li-engine` stays untouched.
- `li-cli` and `li-studio` both depend on it.
- **Recommendation:** amend M5 so `build --media` is written there and not in `li-cli/src/dos.rs`.

**Functions**:

- `media::list(&Project) -> Vec<MediaInfo>`
- `media::plan(&Project, id, &PlanInput{version, runtimes: RuntimeSet, overrides: Option<MediaEdit>}, &mut SizeCache) -> Result<MediaPlan, MediaError>`. It compiles each target, de-duplicates, packs per file at each pack's chunk size, places files on volumes, and counts FAT clusters (`clusters_needed`) or ISO sectors. It also works out the per-component toggle deltas and both chunk-size costs. `SizeCache` is keyed on (CRC, length, chunk size), so a toggle does not compress again.
- `media::build(&MediaPlan, out, &mut dyn FnMut(BuildEvent) -> bool) -> Result<Vec<Artefact>, MediaError>`
- `media::runtimes::discover(...)`: the runtime lookup moved out of `li-cli/src/dos.rs`, with no automatic `build.sh` when called from Studio.
- `media::edit::apply(installer_toml, &MediaEdit) -> String`: a format-keeping edit, like `li-studio/src/tauri.rs` → `settings::apply`.
- `media::validate`: label, serial and 8.3 rules (it calls M1's `check_label`).

**Wire types** (derive `ts_rs::TS`, generated into `packages/protocol`): `MediaInfo`, `MediaTarget`, `MediaEdit`, `MediaPlan`, `VolumePlan`, `DiskEntry`, `PackEntryPlan`, `ComponentSize {raw, packed, pack, targets, volume, delta_if_toggled, fits_if_toggled}`, `MediaWarning {code, severity, subject, message}`, `BuildEvent {job, phase, fraction, line}`, `MediaArtefact {kind, path, bytes, sha256}`, `MediaBuildStatus`.

**Studio core** (`crates/li-studio/src/media.rs`)

- `Studio::list_media`, `media_plan(id, overrides)` (run off the UI thread), `set_media(id, MediaEdit, label, source)` and `remove_media`.
- New `DocKey::Media` for `installer.toml`, so every change is one undo step.
- `start_media_build` / `media_build_status` / `cancel_media_build`: one job per medium, holding the log.

**Tauri commands** (`lib.rs`): `list_media`, `get_media_plan`, `set_media`, `remove_media`, `start_media_build` (async; sends `studio://media-build` events), `media_build_status`, `cancel_media_build`, `list_removable_floppies` (later), `test_media_in_emulator`, `reveal_path`. `api_contract.rs` checks the names automatically.

**MCP tools**:

- `list_media`, `get_media_layout` (the plan: tree, sizes, warnings), `estimate_media` (the plan with unsaved `overrides`, nothing written), `set_media` (patch or replace, undoable), `build_media` (returns a `job`), `get_media_build` (`job`, log tail, `artefacts`).
- The current server answers each request once over HTTP, so build progress is read by asking again, not pushed **[V]**.

**CLI**:

- `liminst build --media <id>` (M5).
- `liminst media plan <id> [--json] [--exclude c] [--include c]` prints the same `MediaPlan`, so the CLI and the agents get exactly what the screen shows.

## 6. Retro look and wireframes

- **Frame:** Studio cards and `--lis-*` tokens.
- **Retro parts:** the meter, disk map and README box use VT323 (already bundled) and CP437 block cells (█ used, ▓ common pack, ░ free, ▒ reserve), one cell per 16 KiB. Patterns as well as colour, so they read in high contrast.
- **Media art:** the kind icon is a pixel-art floppy or CD as inline SVG.
- **Wrong-OS and disk prompts:** the dialogs reuse `packages/renderer`'s `pvDos` and Windows 95 classes, so they look exactly like Dry-run.

**Media screen**:

```text
┌ Media ─────────────────────────────────────────────── [+ Floppy] [+ CD] [Build all] ┐
│ One disk or disc that holds several targets. Shared files are stored once.          │
├───────────────────┬─────────────────────────────────────────────────────────────────┤
│ ▣ floppy    [ok]  │ ▣ floppy · QUILL · 3½" 1.44 MB · 3 targets          [Build] [⋯] │
│   171,008 free    │ ┌ Capacity ───────────────────────────────────────────────────┐ │
│ ◎ cd        [ok]  │ │ ██████████████████████████▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░▒          │ │
│   1.75 MB on 650  │ │ Used 1,286,656 · Common pack 412,160 · Free 171,008         │ │
│                   │ │ Reserve 16,384 ✓     Without sharing: over by 247,808        │ │
│ ⚠ 1 warning       │ └─────────────────────────────────────────────────────────────┘ │
│  4 KiB chunks for │ [Disk map] [Sizes] [Targets] [Label & README] [Autorun]          │
│  Windows 3.x      │  … tab content …                                                │
│  cost 29,184      │ ┌ Build ──────────────────────────────────────────────────────┐ │
│                   │ │ ▓▓▓▓▓▓▓▓░░░░ Packing WIN31\SETUP.LPK         [Cancel]       │ │
│                   │ │ quill-0.1.0-floppy.img  1,474,560  [Reveal] [Copy SHA-256]  │ │
│                   │ │ [Test in DOSBox Staging] [Write to floppy…] [Open release…] │ │
└───────────────────┴─────────────────────────────────────────────────────────────────┘
```

**Disk map and sizes**:

```text
┌ Disk map ─────────────────────────────── Disk 1 ─┬─ Details ─────────────────────┐
│ A:\  QUILL   Serial QD01-0100                    │ C0000003  (EN_CA.DIC)         │
│ ├ SETUP.EXE        165,376  DOS program          │ Raw 300,086 · Packed 210,547  │
│ ├ SETUP.LPK        131,072  own pack (DOS)       │ Pack COMMON.LPK · 4 KiB       │
│ ├ COMMON.LPK       612,352  common pack  ▸       │ Used by DOS, Win 3.x, Win 95  │
│ │ ├ C0000001 QUILL.HLP  2,152  DOS·W31·W95       │ Component: Canadian dictionary│
│ │ └ C0000003 EN_CA.DIC  210,547  DOS·W31·W95     │ Dest: \QUILL\EN_CA.DIC        │
│ ├ README.TXT         2,048                       │ Leaving it off frees 210,944  │
│ ├ WIN31\ SETUP.EXE 206,336 · SETUP.LPK 158,208   │                               │
│ └ WIN95\ SETUP.EXE 140,288 · SETUP.LPK 236,032   │                               │
├ Sizes ───────────────────────────────────────────┴───────────────────────────────┤
│ Component          On medium  Raw       Packed    Pack     Disk  Change          │
│ Program            required   884,608   513,731   own      1     —               │
│ Canadian dict.     [■ on ]    300,086   210,547   common   1     frees 210,944   │
│ More dictionaries  [□ off]    596,724   418,817   common   2     needs 419,328 ✗ │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Write-to-floppy confirmation (child window `write-floppy`, modal)**

```text
┌ Write Floppy Image ────────────────────────────────────────── [×] ┐
│ ⚠ Everything on the chosen disk is erased.                         │
│ Image  quill-0.1.0-floppy.img · 1,474,560 · label QUILL            │
│ Disk   ( ) /dev/sdc  TEAC USB FDD · 1.44 MB · not mounted          │
│        ( ) /dev/fd0  Floppy drive 0 · 1.44 MB                      │
│        (no disk is chosen for you; only removable 1.44 MB disks)   │
│ Type the disk to confirm:   [ /dev/sdc        ]                    │
│ Type the label to confirm:  [ QUILL           ]                    │
│ [ ] Read it back and check the SHA-256 afterwards  (on by default) │
│                                   [Cancel]   [Erase and write]     │
└────────────────────────────────────────────────────────────────────┘
```

## 7. Accessibility, i18n, keys, tests, docs

- **Accessibility:**
  - The meter is `role="meter"` with `aria-valuenow/max` and a text alternative ("1,286,656 of 1,457,664 bytes used; 171,008 free; reserve met"). The block-cell drawing is `aria-hidden`.
  - The disk map is an ARIA `tree` (arrow keys move, Right opens a node, Enter shows details). The sizes table uses `th scope` and `aria-sort`. Toggles are `role="switch"` and their note is linked with `aria-describedby`.
  - Build progress is a `progressbar` plus a polite live region, throttled to one message per phase.
  - The motion setting stops the meter animating.
- **Keys:**
  - `Ctrl+7` opens the screen; `Ctrl+Shift+B` builds the selected medium (added to `menuModel.ts`, so the Keyboard shortcuts window lists it).
  - Tabs are a `tablist` with arrow keys. `Delete` on a list row removes it after confirmation.
  - Escape closes `write-floppy`, and nothing in that window activates on Enter until both confirmations match.
- **i18n:** every string goes in `messages.ts` under `media.*`:
  - `nav.media`, `menu.viewMedia`
  - `media.title`, `media.intro`, `media.add.floppy`, `media.add.cd`, `media.remove.confirm`
  - `media.kind.floppy1440`, `media.kind.cdrom`
  - `media.meter.used`, `.common`, `.free`, `.reserve`, `.withoutSharing`, `.aria`
  - `media.tab.map`, `.sizes`, `.targets`, `.label`, `.autorun`
  - `media.label.count`, `media.label.rule`, `media.serial.auto`, `media.dir.rule`
  - `media.toggle.frees`, `media.toggle.needs`, `media.toggle.doesNotFit`
  - `media.warn.<code>`
  - `media.build.<phase>`, `media.build.cancel`, `media.build.reveal`, `media.build.copySha`, `media.build.copied`
  - `media.test.dosbox`, `media.test.win31`, `media.test.win95`
  - `media.write.*`, `media.burn.open`, `media.release.open`
  - `preview.media.runFrom`, `preview.media.host.*`
  - Plurals for "N disks" and "N warnings" use `tn`. Messages from the engine pass through unchanged, like flow-validation messages. `bun run check:i18n` and the en-XA and ar-XB pseudo-locales: the meter and tree must mirror correctly in RTL.
- **Tests:**
  - **Rust:** `li-media` plan tests over a three-target Planboard media fixture:
    - shared entries get target mask 7 and 4 KiB chunks;
    - `delta_if_toggled` equals the real difference between two builds (exactness test);
    - label, serial and 8.3 rules; spanning places default-off components last;
    - `does_not_fit` lists the overrun per file;
    - a CLI test shows `media plan --json` equals the MCP `get_media_layout` result.
  - **`li-studio`:** `set_media` undo and redo; the MCP tools list (including the existing name-list test); `build_media` and `get_media_build` with stand-in runtimes.
  - **Contract and bindings:** `api_contract.rs` (automatic) and bindings drift.
  - **Tauri shell:** the new window label in both capability files (`scripts/dev-automation.test.ts` enforces it) and the `windows.rs` spec tests.
  - **Vitest:** `mediaScreen.test.tsx` against a fake `Api` with `mediaFixture.ts`, built from the media plan's section 3 Quill numbers. It covers: add and remove; a toggle updates free space; a label over 11 characters shows the rule; spanning view; build log and errors; keyboard on the tree.
  - **End to end with the Tauri MCP:** `driver_session`, `webview_screenshot` of `main` on each tab in en-CA, en-XA and ar-XB, plus the `write-floppy` window; `webview_keyboard` walks the tree.
- **Docs:**
  - `SPEC.md` §12: `media plan` and `build --media`.
  - `SPEC.md` §13: the tools and the window's screen list, which says "six screens" today.
  - `docs/studio.md`: a "Media screen" section and the job/event model.
  - `docs/decisions.md`: entries for where `li-media` lives, deferring physical writes, and progress by polling for MCP.

## 8. Stacked PRs (each sized for one Sonnet implementer)

| PR | Branch / depends on | Content | Acceptance |
|---|---|---|---|
| S0 | `docs/studio-media`, main | SPEC §12/§13, `docs/studio.md`, decision entries, wireframes as prose | markdownlint |
| S1 | `feat/media-types`, main | `crates/li-media` skeleton: the wire types with `ts`, `MediaWarning` codes, `validate` (label, serial, 8.3, over M1's `check_label` once merged; a local copy until then); bindings generated | Bindings drift clean; validate unit tests |
| S2 | `feat/studio-media-screen`, S1 | Route, menu, `Ctrl+7`, icon, store, `Api` methods with demo and fake implementations, list with add/remove, empty state | Vitest: navigate, add, remove, undo through the fake; i18n check |
| S3 | `feat/studio-media-map`, S2 | `CapacityMeter`, `DiskMap` tree, `SizesTable`, spanning two-column view (fixture) | Vitest a11y roles and keys; screenshots in demo mode |
| S4 | `feat/studio-media-edit`, S3 | Targets and folders, label/serial with the counter, exclude toggles with deltas, overflow/reserve, README and autorun previews, warnings linking to tabs | Vitest: toggle changes free space from the fixture; label rule; RTL screenshot |
| S5 | `feat/media-plan`, **M3 + M5** (+ S1) | Real `li-media::plan`/`edit`/`list` and `SizeCache`; `li-studio/media.rs`, `DocKey::Media`; Tauri commands; MCP `list_media`, `get_media_layout`, `estimate_media`, `set_media`; `liminst media plan` | Exactness test; MCP list test; CLI and MCP JSON equal; Studio shows real Planboard media |
| S6 | `feat/studio-media-build`, S5 | Build jobs, `studio://media-build` events, cancel, results with SHA-256, reveal, open ISO; Preferences runtime paths; MCP `build_media` and `get_media_build`; Runner text updated | Build Planboard floppy from the Studio; the image is byte-equal to the CLI's; a too-large payload shows `does_not_fit` |
| S7 | `feat/studio-media-cd`, S6 + **M7** | CD kind fully on: ISO sectors in the plan, Joliet label, real `AUTORUN.INF` preview | ISO built from the Studio is byte-equal to the CLI's |
| S8 | `feat/studio-media-emulators`, S6 | DOSBox Staging and DOSBox-X paths, `test_media_in_emulator` (DOS floppy as A:, ISO as D:), checkout harness detection for Windows 3.x and 95 | Manual DOSBox run logged; buttons hidden when tools are missing |
| S9a | `feat/dryrun-media`, S5 | "Run from" and "Started on" controls, `MediaContext` in `run_preview`, the `insert-disk` step drawn from the plan with stub text | Vitest + Rust preview test |
| S9b | `feat/dryrun-media-text`, S9a + **M6/M8** | `li-retro::media_text` shared with the runtimes; redirect and set-id steps | Host golden: Studio text equals the runtime's |
| S10 | `feat/studio-media-span`, S4 + **M8** | Spanning live: volume boundaries, disk-2 labels, default-on-disk-2 warning | Plan test with the `common-span` fixture |
| S11 (deferred) | `feat/studio-write-floppy`, S6 | `write-floppy` child window, Linux device listing, `pkexec dd`, read-back check | Tested with a loop-device stand-in; never picks a disk; both confirmations required |

**Order**:

- S0, S1, then S2–S4 can all start now, alongside M3 and M4, against `mediaFixture.ts` and the demo API.
- S5 needs M3 and M5 (M5 needs M1 and M4).
- S6 follows S5. S7, S9b and S10 wait for M7, M6 and M8.

## 9. Decisions (recommended default in bold) and risks

1. **New `li-media` crate, and change M5 to use it**, or keep the logic in `li-cli` and have Studio depend on it.
2. Screen name: **"Media"**, or "Release".
3. Toggles **write immediately and can be undone** (the Studio's usual way), or work on a draft with an Apply button. Agents get the draft way anyway through `estimate_media`.
4. Physical floppy writing: **defer (S11), Linux only first**.
5. Burning: **reveal the ISO and open it with the default app**; no burner built in.
6. Emulators: **DOSBox Staging from Studio for DOS; the Windows harness only when a checkout is found**.
7. Runtimes: **Preferences paths plus checkout discovery; Studio never runs `build.sh` for you**, and shows estimated sizes with a warning instead.
8. MCP build progress: **ask again with `get_media_build`**, not streaming.

**Risks**:

- **Plan speed [U]:** compiling and compressing about 3 MB for three targets on every toggle. `SizeCache` and running off the UI thread should keep it under a second, but this is not measured.
- **Cluster rounding:** deltas are exact for packed bytes but must be rounded per file in the image layout, or the "frees N" note will be off by a cluster.
- **Plan drift:** S5 is blocked if M5 lands its logic in `li-cli`. Agree decision 1 before M5 starts.
- **Format churn:** wire types follow the M3 model; a `FORMAT` bump or model change in M3 means regenerating the bindings.
- **Physical floppy write:** this is a destructive action needing elevated rights. Keep it deferred, device-filtered and double-confirmed.
- **Windows harness:** it relies on a local base image and a checkout, so most installed Studios will only offer DOSBox.

**Not verified:**

- What M3–M8 will actually look like; I worked from the plan only.
- How long a plan takes.
- `xdg-open` handling of ISOs.
- The DOSBox command lines.
- Whether Windows 95 under DOSBox-X can read the floppy and CD.

I also could not drive the running app: the Tauri MCP server, which the end-to-end tests in section 7 rely on, failed to connect in this session. Retry it before those tests.

### Critical files for implementation

- /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-studio/src/mcp.rs (with a new `crates/li-studio/src/media.rs`, built after the `tauri.rs` pattern)
- /home/scott/source/liminal-hq-labs/liminal-installer/apps/studio/src-tauri/src/lib.rs (and `windows.rs` for the `write-floppy` window)
- /home/scott/source/liminal-hq-labs/liminal-installer/apps/studio/src/app/router.tsx (with `screens.ts` and `src/chrome/menuModel.ts`)
- /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-cli/src/dos.rs (the runtime lookup and build code that moves to `crates/li-media`)
- /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-package/src/dos.rs (M1's `clusters_needed`, `check_label`, `floppy_1440_labelled`, on branch `feat/floppy-subdirs`)
