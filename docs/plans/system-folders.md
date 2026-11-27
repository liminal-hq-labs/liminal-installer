# Plan: install a file into the Windows folder or the system folder

The retro Windows runtimes install only below `{install}`. Quill's installers need to place the Quill Ghostwriter screen saver, `QUILLGW.SCR` on Windows 3.x and `AGQUILL.SCR` on Windows 95 and later, where Display Properties (Control Panel's Desktop on Windows 3.x) lists it. This plan adds the smallest feature that does that: two destinations, `{windows}` and `{system}`, for a file on a `win16` or `win32-classic` target, with a backup of anything already there and a clean uninstall.

**Markers.** [V] means checked in the code or files at the path given, on `origin/main` at `2d7b765`. [U] means not checked, and the implementer must check it. Nothing was built or run for this plan.

## Table of contents

- [1. What exists today](#1-what-exists-today)
- [2. Design](#2-design)
- [3. Text for SPEC.md and decisions.md](#3-text-for-specmd-and-decisionsmd)
- [4. Decisions for the owner](#4-decisions-for-the-owner)
- [5. Risks](#5-risks)
- [6. Work packages](#6-work-packages)
- [7. Sonnet briefs](#7-sonnet-briefs)

## 1. What exists today

- **Destinations [V].** `li-retro-build/src/payload.rs` `dos_rel` (line 27) and `win_rel` (line 51) refuse any `to` that does not start with `{install}/` ("the DOS runtime installs into one folder"). `FileEntry.dest` (`li-retro/src/model.rs` line 90) is a path below the install directory.
- **Joining [V].** `install.rs` `join(dir, rel)` (line 70) already passes a `rel` that holds `:` through unchanged, so a full path in a receipt `F` record is removed by the existing uninstall with no change. The Win32 runtime already writes `D` and `S` records by full path.
- **Overwrites [V].** `extract` (`install.rs` line 194) truncates an existing file and records it only when it did not exist; the module comment says a file that was there "is overwritten and, if the install is rolled back, left as the new copy". Nothing is backed up today.
- **A folder token [V].** `default_dir` for Win32 is `%PROGRAMFILES%\<name>` (`li-retro-build/src/lib.rs` line 317), resolved in `app.rs` `seed` (line 273) through `Shell::program_files`. That is the precedent for a folder the runtime asks the machine for.
- **Compiled format [V].** `FORMAT` is 4 and `FORMAT_MIN` 2 (`model.rs` lines 20 and 25); `Project::validate` refuses a feature its claimed format cannot hold (line 382). The decoder (`decode.rs` `file`, line 174) reads `component`, `dest`, `stored`, `size`, `crc`.
- **Shell seam [V].** `li_retro::platform::Shell` (`platform.rs` line 132) has `program_files`, `start_menu_programs` and the uninstaller hooks; DOS has no shell (`shell()` is `None`). `HostPlatform` in `sim.rs` implements `Shell` for the host tests.
- **Win16 [V].** `dosio::windows_dir()` (`retro/win16/src/bin/lisetup/dosio.rs` line 306) calls `GetWindowsDirectory` (KERNEL 134, `tools/imports.tbl` line 45) into a 144-byte stack buffer. `GetSystemDirectory` is not imported. `dosio::copy_file` (line 258) copies through a 1 KiB heap buffer. The data segment has 628 bytes of static room left (`docs/retro-win16.md` line 109); the heap peak is 35 552 of 40 960 bytes for Lt. Fizz (line 97). Code segments have a 40 000-byte budget each, and the packer opens another segment when one fills (`res/lisetup.segments.json`).
- **Win32 [V].** `win32/tools/imports.allow` has `CopyFileA` but neither `GetWindowsDirectoryA` nor `GetSystemDirectoryA`; both are Windows 95 exports [U: confirm against the reference's Windows 95 column, as the file's header says].
- **Engine [V].** `li-engine/src/machine.rs` `Dirs` (line 22) and the `Resolver`'s `NAMES` (line 138) know `install`, `start_menu`, `desktop`, `user_data`, `system_data` and `temp`; an unknown `{name}` is `EngineError::Placeholder`. `liminst preview` and the Studio's dry run plan against `SimMachine`, so a project with `{windows}` cannot be previewed today.
- **Checks [V].** `li-engine/src/check.rs` holds the component checks (`required-default-off` at line 153, `script-on-retro` at line 307).
- **Studio [V].** `ComponentInfo` (`li-studio/src/studio.rs` line 62) carries `required`, `default`, `targets` and a file count; `ProjectScreen.tsx` line 145 prints them.
- **Pack [V].** A Win16 pack stores each file under its destination (`stored = dest` with `/`), a Win32 pack under `F0000001` and on with the destination as the long name (`payload.rs` line 206). Short names may hold `%` (`docs/lpk-format.md`, "Short names").
- **Quill [V].** `installer/components.toml` on Quill's `main` (`4c71ee2`) has targets `dos-x86`, `win16-x86` and `windows-x86`, the `default = false` file-type components and no screen saver yet. `installer/tools/stage.sh` fills `dist/{target}/` and has a `--stand-ins` mode. `installer/liminst.rev` pins the installer commit.

## 2. Design

### 2.1 The two destinations

A file rule may name `{windows}/` or `{system}/` where it named `{install}/`:

```toml
[[component]]
id      = "screensaver"
name    = "Screen saver"
default = false
targets = ["win16-x86", "windows-x86"]
files   = [{ from = "dist/screensaver/FIZZ.SCR", to = "{windows}/" }]
```

| Placeholder | Windows 3.x (`win16`) | Windows 95, 98, Me (`win32-classic`) | Windows NT and later (`win32-classic`) | DOS (`tui-retro`) |
|---|---|---|---|---|
| `{windows}` | `GetWindowsDirectory` (`C:\WINDOWS`) | `GetWindowsDirectoryA` (`C:\WINDOWS`) | `GetWindowsDirectoryA` (`C:\Windows`) | refused |
| `{system}` | `GetSystemDirectory` (`C:\WINDOWS\SYSTEM`) | `GetSystemDirectoryA` (`C:\WINDOWS\SYSTEM`) | `GetSystemDirectoryA` (`System32`; a 32-bit program on 64-bit Windows lands in `SysWOW64`) | refused |

Table 1. Where each destination resolves.

Rules, all checked by `liminst check` and enforced by the compiler:

- Only a file directly in the folder. `{windows}/SUB/X` is refused, so the runtime never makes or removes a folder in the Windows tree.
- Only `win16` and `win32-classic` targets. DOS has no Windows folder, and the modern engine targets have no real-machine answer yet.
- Shortcuts and file types still name a program under `{install}`; the existing compile error stays.
- Win16 names stay 8.3 (the existing `dos_rel` rule); Win32 names follow `win_rel`.
- The Windows folder, not the system folder, is the one to use for a screen saver: every Windows lists `.SCR` files there [U: Display Properties on Windows 95, XP and 10, Control Panel Desktop on 3.11], while `SysWOW64` is a doubtful place on 64-bit Windows [U].

### 2.2 Compiler and compiled format

- **Tokens in `dest`.** The compiler writes a rooted file's `dest` as `%WINDIR%\NAME` or `%SYSDIR%\NAME`, following `%PROGRAMFILES%` in `default_dir`. The decoder does not change, and `stored` stays `dest` with `/` on Win16 (`%WINDIR%/QUILLGW.SCR` is a valid 8.3 path: `%` is a short-name character and `%WINDIR%` is eight of them) and `F000000n` on Win32. A rooted file and an install-folder file of the same name cannot collide in the pack. `%` sorts before letters, so the destination order `Project::validate` checks still holds.
- **Reserved names.** An `{install}` path whose first part is `%WINDIR%` or `%SYSDIR%` (any case) is refused, as `SETUP.PRJ` is.
- **Format 5.** `FORMAT` becomes 5 and `FORMAT_MIN` stays 2. `Project::validate` refuses a project of format 4 or older that has a `dest` starting with `%`, and a `%` token other than the two, or one with a `\` after the file name. A runtime that predates 5 refuses a 5 project as it does today ("unsupported format version"), and the program and its pack come from one build (D86).
- **Flavours.** `Flavour::Dos` refuses a rooted `to` with "`{windows}` and `{system}` have no meaning on DOS" (the check catches it first). `Flavour::Win16` and `Flavour::Win32` accept it.
- **Alternative not taken.** A `root: u8` field on `FileEntry`. It is more explicit but changes the decoder and needs generated stored names to avoid collisions on Win16; the token costs neither. The swap is local to `payload.rs`, `model.rs`, `decode.rs` and `install.rs` and only cheap before WP-SYS-1 lands.

### 2.3 Install: placing, conflicts, backups and rollback

`install.rs` gains `target_path(p, dir, dest) -> Result<String, Error>`: `%WINDIR%\X` becomes `<windows_dir>\X`, `%SYSDIR%\X` becomes `<system_dir>\X` (from the shell, without a trailing backslash), anything else is `join(dir, dest)`. A rooted file on a machine whose shell answers `None` (DOS, or a shell that cannot ask) is `Error::Io("this machine has no Windows folder")`. `make_dirs` is not called for a rooted file's parent.

For each rooted file, before it is extracted:

1. **Ours already.** When the receipt already in `dir` (the `before` receipt the file types use) has an `F` record with the same full path (ASCII case ignored), the file is ours from an earlier install: it is overwritten, and the earlier `O` record for that path, if any, is carried into the new receipt.
2. **Someone else's.** When the file exists and is not ours, it is copied with `Shell::copy_file` to `BACKUP\WIN\<name>` or `BACKUP\SYS\<name>` below the install directory (`make_dirs` makes `BACKUP` and its child and journals them), then extracted over. The pair is noted for the receipt and for rollback.
3. **Absent.** Extracted as any file, and journalled as created.

Rollback (cancel or failure) puts each backup back **before** it removes the files it made: `Journal` gains `restores: Vec<(String, String)>` (backup, original), and `rollback` copies each backup over its original, last first, then removes `j.files` (which lists the backup copies, since they were created) and `j.dirs`. A rooted file that was ours is left as the new copy, as an install-folder file is today.

`Progress.file` shows the full resolved path for a rooted file, so the copying page says `C:\WINDOWS\QUILLGW.SCR` rather than a token. `bytes_needed` still counts every file against the install drive; a Windows folder on another drive is not measured (see Risks).

The runtime says what it backed up on its log channel (`/LOG` on Win16, the dump on Win32: `backup: C:\WINDOWS\QUILLGW.SCR -> BACKUP\WIN\QUILLGW.SCR`), and nothing on a page: no file is lost, so the person is not asked (owner decision 1).

### 2.4 Receipt

- A rooted file is an `F` record by full path: `F C:\WINDOWS\QUILLGW.SCR 51234 1A2B3C4D`. `Receipt::parse` already reads it (`rsplitn(3, ' ')`), and `join` passes it through.
- A replaced file is a new letter, `O original|backup`: the full path and the copy below the install directory, escaped as the `A` record is (`%`, `|`, CR, LF). `Receipt` gains `overwritten: Vec<(String, String)>`. A reader that does not know `O` ignores it (SPEC §9), so it restores nothing, which is the safe failure.
- The receipt stays `LIR 1`.

### 2.5 Uninstall

1. The `F` records are removed as today, rooted ones included (full paths). A removal that fails stops the uninstall with the receipt kept, as for any file today, so a second run finishes it.
2. Then each `O` record, last first: copy the backup over the original with `Shell::copy_file`, then remove the backup. A missing backup is skipped.
3. Then `BACKUP\WIN`, `BACKUP\SYS` and `BACKUP` are removed when empty, whether or not the receipt has `D` records for them (a reinstall finds them existing and does not record them).
4. The rest as today.

The uninstall does not check that the file in the Windows folder is still the one it installed: an `F` file is removed by name, as everywhere else in the receipt (owner decision 7). When the person has chosen the saver as their screen saver, Windows is left naming a file that is gone and shows none; the runtime does not edit `SYSTEM.INI` or `HKCU\Control Panel\Desktop` (owner decision 5).

### 2.6 `.LPK` format

No change, and no golden file changes. The rooted names are ordinary short and long names under the existing rules (`%` is a short-name character; long names refuse only `\ : * ? " < > |` and control characters). The compatibility rule ("a writer writes the lowest minor version that can express the pack") is met by writing 1.0 as now. `SHARED` (0x0004) is not set: the file belongs to one product. `crates/li-lpk/tests/golden/` is not touched, and `LI_UPDATE_GOLDEN=1` is not run. `liminst lpk unpack` of a Win16 pack makes a folder named `%WINDIR%`, which is harmless and shows where the file goes.

### 2.7 Engine, `liminst check` and preview

- `Dirs` gains `windows: Option<PathBuf>` and `system: Option<PathBuf>`; `Dirs::get` answers `"windows"` and `"system"` from them, and `NAMES` lists both so `unresolve` gives `{windows}/AGQUILL.SCR` back in a plan. `SimMachine` sets `/sim/windows` and `/sim/windows/system` (and makes both); `RealMachine` sets `None` (`sandboxed` sets `root/windows` and `root/windows/system`), so a real Linux run of such a project fails with the existing `Placeholder` error, which the check prevents first.
- New diagnostics in `check.rs`, both errors, and `build` refuses through the compiler:

| Code | When | Message |
|---|---|---|
| `system-folder-unsupported` | A file rule's `to` starts with `{windows}` or `{system}` in a component that applies to a target whose renderer is not `win16` or `win32-classic` | "component `c` installs `to` into the Windows folder, which target `t` (`renderer`) does not have; limit the component's `targets` to the Windows 3.x and Windows 95 targets" |
| `system-folder-subfolder` | Such a `to` has a folder after the placeholder (`{windows}/SUB/`) | "component `c`: only a file directly in `{windows}` or `{system}` can be installed, not one in a folder below it" |

Table 2. New `check` diagnostics.

### 2.8 Studio

`ComponentInfo` gains `system_folders: Vec<String>` (`"windows"`, `"system"`, in that order, each once) from the components' file rules; ts-rs regenerates `packages/protocol/src/generated/ComponentInfo.ts`. The Project screen adds " · installs into the Windows folder" or " · installs into the Windows system folder" to the component's line (`project.writesWindows`, `project.writesSystem` in `messages.ts`). The dry run already shows the plan, which now resolves, as `{windows}/AGQUILL.SCR`. No new screen, setting or MCP tool. The Media screen's demo data and `li-media` are not touched: the planner is not built, and when it is, a rooted file counts against its target's pack like any other.

### 2.9 DOS

No runtime change: the compiler refuses a rooted file for `Flavour::Dos` and the check refuses it first. The DOS `SETUP.EXE` links the new `install.rs` code, so it grows by about the same as the host build of `li-retro` [U: measure in `make dos`]; its `shell()` is `None`, so `target_path` errors for a rooted file, which no compiled DOS project can hold.

### 2.10 Budgets

| What | Budget | Expected cost |
|---|---|---|
| Win16 data segment statics | 628 bytes free | 0: `system_dir` uses a 144-byte stack buffer, as `windows_dir` does |
| Win16 heap peak | 40 960; Lt. Fizz 35 552 | About 1.3 KiB during a backup (the 1 KiB copy buffer and paths), freed after; measure with `/LOG` |
| Win16 code segments | 40 000 each, more segments on demand | About 1.5 to 2.5 KB across `li-retro` and `lisetup` [U]; a fifth segment is acceptable but must be reported |
| Win16 imports | `imports.tbl` | One: `KERNEL 135 GetSystemDirectory pw w` [U: check the ordinal with `tools/ne_dump.py` on Windows 3.11's `KRNL386.EXE`, as D88 did] |
| Win32 imports | `imports.allow`, Windows 95 exports only | Two: `kernel32.dll!GetWindowsDirectoryA`, `kernel32.dll!GetSystemDirectoryA` |
| Win32 and DOS `SETUP.EXE` | No size gate | Report the size change in the PR |
| Quill's floppy | At least 16 KiB free (release-media plan) | Both `.SCR` files [U: sizes not known]; a saver that does not fit is left off the floppy with `[[media]] exclude` once media exist, else the component is CD and zip only |

Table 3. Budgets.

## 3. Text for SPEC.md and decisions.md

WP-SYS-1 pastes the §1, §8 and §9 text and D95; WP-SYS-2 pastes the §4 paragraph, which names its check codes. Adjust only line references.

**SPEC §1, the paths bullet**, replacing its second sentence:

> Install-time paths use placeholders: `{install}`, `{user_data}`, `{system_data}`, `{start_menu}`, `{desktop}`, `{temp}`, and on the Windows 3.x and Windows 95 targets `{windows}` and `{system}` (§4).

**SPEC §4**, a new paragraph after the `default = false` paragraph:

> A file may go to `{windows}/` or `{system}/` in place of `{install}/`: the machine's Windows folder or its system folder, as Windows reports them, which is where Display Properties finds a screen saver (`.SCR`). Only a file directly in one of those folders, and only on a `win16` or `win32-classic` target: `check` reports `system-folder-unsupported` for a component that applies to any other target (DOS has no Windows folder) and `system-folder-subfolder` for a path in a folder below one, and `build` refuses both. Shortcuts and file types still name a program under `{install}`. Choosing the saver in Display Properties is left to the person.

**SPEC §8**, at the end of the retro paragraph:

> The Windows 3.1 and Win32 runtimes copy a `{windows}` or `{system}` file to the folder `GetWindowsDirectory` or `GetSystemDirectory` names (`C:\WINDOWS\SYSTEM` on Windows 3.x, 95, 98 and Me; `System32` on Windows NT and later, which a 32-bit program on 64-bit Windows reaches as `SysWOW64`). A file already there that no earlier install of this product put there is copied to `BACKUP\WIN\` or `BACKUP\SYS\` in the install folder before it is replaced, and is put back when the install is rolled back or removed.

**SPEC §9**, after the `A` record sentences:

> A file outside the install folder is an `F` record by full path, and a file it replaced is `O path|backup` (the full path, then the copy below the install directory, escaped as in `A`). The uninstall removes the `F` file, then copies the backup back, removes the copy and removes `BACKUP` when it is empty; a second install over the first keeps the first's `O` record and does not back up its own file.

**decisions.md**, the next free number (D95 on `main` today; a merge may renumber it):

> | D95 | A file may be installed into the Windows folder or the system folder | `{windows}/` and `{system}/` join `{install}/` as file destinations on `win16` and `win32-classic` targets, for a file directly in the folder; `check` refuses them elsewhere (`system-folder-unsupported`) and below a subfolder (`system-folder-subfolder`). The compiler writes the destination as `%WINDIR%\NAME` or `%SYSDIR%\NAME`, the compiled format is 5, and the runtime resolves the token through `Shell::windows_dir` and `Shell::system_dir` (`GetWindowsDirectory` and `GetSystemDirectory`, KERNEL 134 and 135 on Windows 3.x, the `A` functions on Win32). A file already there that is not this product's is copied to `BACKUP\WIN\` or `BACKUP\SYS\` in the install folder first and recorded as `O path\|backup`; rollback and uninstall copy it back. The receipt records the file by full path | A screen saver is listed only from the Windows or system folder, and Quill ships one as an option. A token in the destination follows `%PROGRAMFILES%` in `default_dir`, leaves the decoder and the pack format as they are and cannot collide with an install-folder file in a Win16 pack; a `root` field was the alternative. Backing up rather than asking means a silent install and a person both lose nothing, and three runtimes need no new page. Refusing subfolders keeps the runtime from making or removing folders in the Windows tree |

## 4. Decisions for the owner

Each has a default this plan builds on; the swap point says where to change it.

| # | Question | Default taken | Where to swap |
|---|---|---|---|
| 1 | A file of the same name is already in the Windows folder: what happens? | Back it up to `BACKUP\WIN\` in the install folder, replace it, and put it back on rollback and uninstall; log it, ask nothing | `install.rs` `place_rooted` (WP-SYS-1). Alternatives: refuse the install with a message naming the file, or a Replace, Keep and Cancel page (a new built-in page in three runtimes, much larger) |
| 2 | Which folder does Quill's saver go in? | `{windows}` for both `QUILLGW.SCR` and `AGQUILL.SCR` | Quill's `installer/components.toml` (WP-SYS-8): `{system}` is one word per component |
| 3 | A rooted file on DOS | Refused at `check` (error) and by the compiler | `check.rs` and `payload.rs` (WP-SYS-2, WP-SYS-3): a warning and a skipped file instead, so one component can serve DOS too |
| 4 | Files in a folder below `{windows}` | Refused | `check.rs` and `payload.rs`; allowing them needs `D` records and folder removal in the Windows tree |
| 5 | Make the saver the active screen saver? | No; the person chooses it in Display Properties | A later operation (`SCRNSAVE.EXE` in `SYSTEM.INI` `[boot]` on 3.x, `HKCU\Control Panel\Desktop` `SCRNSAVE.EXE` on 95), with its own undo |
| 6 | Who builds `QUILLGW.SCR` (Windows 3.x)? | Quill's own build, staged by `stage.sh` from `QUILLGW_SCR`; `AGQUILL.SCR` from afterglow-savers through `AGQUILL_SCR`; stand-ins until both exist [U] | `installer/tools/stage.sh` in Quill (WP-SYS-8) |
| 7 | The person replaced the saver after the install: does uninstall remove it? | Yes, by name, as every `F` file | `install.rs` `uninstall`: a size check before removing a rooted file (streamed CRC is not possible on the Win16 heap without a new read API) |
| 8 | What carries the end-to-end tests? | Lt. Fizz gains an unticked `screensaver` component (`FIZZ.SCR`, a stand-in) for `win16-x86` and `windows-x86` | `examples/lt-fizz/installer/components.toml` (WP-SYS-3); a test-only fixture project would keep the example unchanged |
| 9 | The saver does not fit Quill's floppy | CD and zip only, through `[[media]] exclude` once media are built; until then it ships wherever the target does | Quill's `installer.toml` media entries |

Table 4. Owner decisions.

## 5. Risks

- **Where Display Properties looks [U].** The listing folders above are from memory. Check on the Windows dev VM (Windows 10), the Windows 95 test bed (`docs/win95-testbed.md`) and Windows 3.11 in DOSBox-X before Quill ships it. If a version lists only the system folder, owner decision 2 swaps.
- **WOW64 [U].** `{system}` from a 32-bit `SETUP.EXE` on 64-bit Windows is `SysWOW64`; the 64-bit Screen Saver dialog may not list it. This is why the Windows folder is the default.
- **Elevation.** Writing to `C:\Windows` on Vista and later needs administrator rights, as `C:\Program Files` already does; `SETUP.EXE` relies on Windows' installer detection to ask [U: confirm a UAC prompt appears for the Win32 `SETUP.EXE`].
- **Free space.** Only the install drive is measured; a Windows folder on another, full drive fails during copying and is rolled back. Rare enough to accept.
- **Win16 heap.** The backup copy's 1 KiB buffer adds to the peak; Quill's peak is higher than Lt. Fizz's [U]. Measure with `/LOG` on 3.11.
- **Win16 code size.** A fifth code segment grows `SETUP.EXE` and its load time slightly. Report it; it does not break anything.
- **Network Windows installs.** A shared Windows 3.x folder can be a UNC path with no `:`; `join` treats only a `:` path as absolute. `target_path` must build the full path itself and never pass a rooted path through `join` [mitigated in WP-SYS-1].
- **The ordinal.** KERNEL 135 for `GetSystemDirectory` is from Wine's `krnl386.exe16.spec` [U] until read from Windows 3.11's `KRNL386.EXE`.
- **Format bump.** A Quill build that pins an older runtime with a newer pack fails with "unsupported format version", as with formats 3 and 4. Quill pins one installer commit, so this holds.
- **Wine paths.** Wine's Windows folder is `C:\windows` (lower case) and a 32-bit program's system folder is `drive_c/windows/syswow64` in a 64-bit prefix. The e2e checks must map receipt paths through `drive_c` case-insensitively, and the Win32 receipt checker in `wine-e2e.sh` joins every `F` name to the install folder today, which a full path breaks.

## 6. Work packages

Build order: **WP-SYS-1** and **WP-SYS-2** in parallel; then **WP-SYS-3** and **WP-SYS-6** in parallel; then **WP-SYS-4** and **WP-SYS-5** in parallel; then **WP-SYS-8** in the Quill repository. WP-SYS-7 (DOS) is folded into WP-SYS-2 and WP-SYS-3. That is at most three builders at once.

### WP-SYS-1 — The runtime core in `li-retro`, with SPEC and D95

- **Branch:** `feat/system-folders-core`, from `main`.
- **Finish line:** a hand-built format 5 `Project` with `%WINDIR%` and `%SYSDIR%` files installs on `HostPlatform` into `C:\WINDOWS` and `C:\WINDOWS\SYSTEM`, backs up a file that was there, rolls back to the original on cancel and on a forced failure, uninstalls to the original state, and reinstalls over itself without a second backup. SPEC §1, §8, §9 and D95 are in.
- **Files owned:** `crates/li-retro/src/{model.rs,install.rs,platform.rs,sim.rs}`, `crates/li-retro/tests/` (a new `system_folders.rs`), SPEC §1, §8 and §9, `docs/decisions.md`. SPEC §4's paragraph is WP-SYS-2's.
- **Must not touch:** `decode.rs` (the token needs no decoder change), `crates/li-lpk/`, the runtimes (`retro/win16`, `win32`, `dos`), `li-retro-build`, `li-engine`, `docs/lpk-format.md`, golden files.
- **Tests:** `cargo nextest run -p li-retro --locked`; `cargo clippy -p li-retro --all-targets -- -D warnings`; `cargo fmt --all -- --check`; the `no_std` build of `li-retro` that `make validate` covers.
- **Budgets:** no new statics; every new allocation is a path `String`.

### WP-SYS-2 — Engine placeholders and `liminst check`

- **Branch:** `feat/system-folders-check`, from `main`.
- **Finish line:** `Dirs` and `Resolver` know `{windows}` and `{system}`; `SimMachine` and `RealMachine::sandboxed` give them folders and `RealMachine::new` gives `None`; `check` reports `system-folder-unsupported` and `system-folder-subfolder`; `liminst preview` of a project with a `{windows}` file on `win16-x86` prints the copy as `{windows}/…`.
- **Files owned:** `crates/li-engine/src/{machine.rs,sim.rs,real.rs,check.rs}`, the tests under `crates/li-engine/tests/` and `crates/li-cli/tests/`, and SPEC §4's new paragraph.
- **Must not touch:** `li-retro`, `li-retro-build`, `li-studio`, SPEC §1, §8 and §9 and `docs/decisions.md` (WP-SYS-1's).
- **Tests:** `cargo nextest run -p li-engine -p li-cli --locked`; a check test per code (a DOS component and a `{windows}/SUB/` rule), and a resolve and unresolve round trip.
- **Budgets:** none.

### WP-SYS-3 — The compiler, the Lt. Fizz example and the host end-to-end tests

- **Branch:** `feat/system-folders-compile`, on WP-SYS-1 and WP-SYS-2.
- **Finish line:** `dos_rel` and `win_rel` accept `{windows}/NAME` and `{system}/NAME` for `Flavour::Win16` and `Flavour::Win32` and write `%WINDIR%\NAME` or `%SYSDIR%\NAME`; `Flavour::Dos` refuses them; the reserved first parts are refused; `FORMAT` 5 projects come out. Lt. Fizz has the `screensaver` component; the host tests install it unticked (absent) and ticked (present, with a seeded conflict) on the Win16 and Win32 flavours and uninstall cleanly. The Win16 and Win32 page goldens include the new checklist item.
- **Files owned:** `crates/li-retro-build/src/payload.rs`, `crates/li-retro-build/tests/` (a new `system_folders.rs`, and the goldens under `tests/golden/win16` and `tests/golden/win32`), `examples/lt-fizz/installer/components.toml`, `examples/lt-fizz/installer/dist/screensaver/FIZZ.SCR` (a short text stand-in), `docs/retro-dos.md` (one sentence: DOS refuses the two placeholders).
- **Must not touch:** `li-retro` beyond a test helper, the runtimes, `li-lpk` and its goldens, the DOS goldens (the component does not apply to `dos-x86`, so `components.txt` and the other DOS pages must not change; if they do, the component's `targets` is wrong).
- **Tests:** `cargo nextest run -p li-retro -p li-retro-build -p li-cli --locked`; `tests/placeholders.rs`, `tests/optional.rs`, `tests/win16.rs` and `tests/win32.rs` still pass; `liminst check -C examples/lt-fizz` is clean.
- **Budgets:** none.

### WP-SYS-4 — Windows 3.1 runtime

- **Branch:** `feat/win16-system-folders`, on WP-SYS-3.
- **Finish line:** `Win16Platform` answers `windows_dir`, `system_dir` and `copy_file`; `GetSystemDirectory` is imported; `e2e-wine.sh` installs Lt. Fizz with the screen saver ticked over a seeded `C:\windows\FIZZ.SCR`, finds the new file, the backup and the `O` record, uninstalls and finds the seeded file back and no `BACKUP`; `e2e-w31.sh` does the same on Windows 3.11; `docs/retro-win16.md` describes it with the measured heap peak and code size.
- **Files owned:** `retro/win16/tools/imports.tbl`, `retro/win16/src/imports.rs` (regenerated by `tools/gen_imports.py`, never by hand), `retro/win16/src/bin/lisetup/{dosio.rs,plat.rs}`, `retro/win16/tools/{e2e-wine.sh,e2e-w31.sh}`, `docs/retro-win16.md`.
- **Must not touch:** `li-retro` (raise a core bug with WP-SYS-1's owner), the packer (`tools/ne_pack.py`), `res/lisetup.segments.json` budgets, the Win32 runtime.
- **Tests:** `make win16` (host tests, `check.sh`, the Wine install and uninstall); `retro/win16/tools/e2e-w31.sh` on Windows 3.11 in DOSBox-X; the ordinal read with `tools/ne_dump.py` from 3.11's `KRNL386.EXE` and named in `imports.tbl`'s header.
- **Budgets:** 628 bytes of statics untouched; heap peak with the saver ticked under 40 960 and reported; segment report pasted in the PR.

### WP-SYS-5 — Win32 runtime

- **Branch:** `feat/win32-system-folders`, on WP-SYS-3.
- **Finish line:** `Win32` answers `windows_dir` (`GetWindowsDirectoryA`), `system_dir` (`GetSystemDirectoryA`) and `copy_file` (`CopyFileA` with `bFailIfExists` 0); both imports are on `imports.allow`; `wine-e2e.sh` installs Lt. Fizz with the screen saver ticked (a `/PLAY` toggle on the components page) over a seeded `drive_c/windows/FIZZ.SCR`, checks the file, the backup and the receipt (its CRC checker maps `C:\` to `drive_c`), uninstalls and finds the seeded file back; `docs/retro-win32.md` says so.
- **Files owned:** `win32/setup/src/{sys.rs,platform.rs}`, `win32/tools/{imports.allow,wine-e2e.sh}`, `docs/retro-win32.md`, `crates/li-retro-build/tests/golden/win32/` only if the ticked run's pages differ from WP-SYS-3's.
- **Must not touch:** `li-retro`, the Win16 runtime, `win32.json`, the Pentium scan.
- **Tests:** `make win32` (`check.sh` with the import, Pentium and PE checks, the host tests, the Wine run); `WIN32_REQUIRE_TOOLS=1`.
- **Budgets:** two imports, both Windows 95 exports; `SETUP.EXE` size change reported.

### WP-SYS-6 — Studio

- **Branch:** `feat/studio-system-folders`, on WP-SYS-2.
- **Finish line:** `ComponentInfo.system_folders` is filled and generated; the Project screen shows "installs into the Windows folder" or "installs into the Windows system folder"; the demo data and the UI tests carry the field; a dry run of a `win16-x86` target with a `{windows}` file shows the copy.
- **Files owned:** `crates/li-studio/src/studio.rs`, `packages/protocol/src/generated/ComponentInfo.ts` (generated), `apps/studio/src/screens/ProjectScreen.tsx`, `apps/studio/src/i18n/messages.ts`, `apps/studio/src/lib/demoApi.ts`, the test fixtures that build a `ComponentInfo`.
- **Must not touch:** `packages/chrome`, `src/theme/tokens.css`, `li-media`, the Media screen.
- **Tests:** `cargo nextest run -p li-studio --locked`; `bash scripts/check-bindings-drift.sh`; `bun run format:check && bun run typecheck && bun run test:js && bun run check:i18n`; a look at `?locale=en-XA` and `?locale=ar-XB`.
- **Budgets:** none.

### WP-SYS-8 — Quill's components (Quill repository)

- **Branch:** `feat/installer-ghostwriter` in `~/source/experiments/quill16`, after WP-SYS-4 and WP-SYS-5 merge.
- **Finish line:** `installer/components.toml` has `ghostwriter16` and `ghostwriter32` (below), `stage.sh` stages `QUILLGW.SCR` and `AGQUILL.SCR` (and stand-ins), `liminst.rev` names the installer commit with WP-SYS-4 and WP-SYS-5, `liminst check` is clean on stand-ins, and Quill's `e2e-w31.sh` and `e2e-w95.sh` tick the saver once and check `C:\WINDOWS`.
- **Files owned:** `installer/components.toml`, `installer/tools/stage.sh`, `installer/liminst.rev`, `installer/tools/e2e-w31.sh`, `installer/tools/e2e-w95.sh`, Quill's own docs where they list the components.
- **Must not touch:** anything outside `installer/` and its docs; the afterglow-savers repository.
- **Tests:** `installer/tools/check.sh`; the two e2e scripts with the real or stand-in saver.

```toml
# Quill Ghostwriter, the screen saver: optional and unticked. It goes in the Windows folder, where Display Properties (Control
# Panel's Desktop on Windows 3.x) lists it. A file of the same name already there is kept in the install folder's BACKUP and put back
# on uninstall. Choosing it as the screen saver is left to the person.
[[component]]
id      = "ghostwriter16"
name    = "Quill Ghostwriter screen saver"
default = false
targets = ["win16-x86"]
files   = [{ from = "dist/{target}/QUILLGW.SCR", to = "{windows}/" }]

[[component]]
id      = "ghostwriter32"
name    = "Quill Ghostwriter screen saver"
default = false
targets = ["windows-x86"]
files   = [{ from = "dist/{target}/AGQUILL.SCR", to = "{windows}/" }]
```

## 7. Sonnet briefs

Paste one brief per builder. Each assumes the repository rules in `AGENTS.md` and `CLAUDE.md`.

### Brief: WP-SYS-1 (in full)

> You are a Sonnet builder on Liminal Installer (`~/source/liminal-hq-labs/liminal-installer`, remote `liminal-hq-labs/liminal-installer`). The main checkout belongs to the orchestrator: make a worktree at `~/source/worktrees/liminal-installer/system-folders-core` on a new branch `feat/system-folders-core` from `origin/main`, and work only there. Read `AGENTS.md`, `SPEC.md` §1, §4, §8 and §9, `docs/decisions.md` D86 to D93, and `docs/plans/system-folders.md` sections 2.1 to 2.5 and 3, which are your specification.
>
> **Goal.** Teach the retro runtime core (`crates/li-retro`) to install a file into the Windows folder or the system folder. The compiler is not your job: build test projects by hand in Rust.
>
> **Do this.**
>
> 1. `model.rs`: `FORMAT` to 5 (keep `FORMAT_MIN` 2) and extend its doc comment. Add `pub const WINDOWS_TOKEN: &str = "%WINDIR%";` and `pub const SYSTEM_TOKEN: &str = "%SYSDIR%";` with doc comments. In `Project::validate`, refuse: a `dest` that starts with `%` in a project of format 4 or older ("a file outside the install folder needs format version 5"); a `dest` that starts with `%` but is not `%WINDIR%\NAME` or `%SYSDIR%\NAME` with no further `\`. Keep the existing destination-order check.
> 2. `platform.rs`: add to `Shell`, with doc comments and defaults: `fn windows_dir(&mut self) -> Option<String> { None }`, `fn system_dir(&mut self) -> Option<String> { None }` (each without a trailing backslash) and `fn copy_file(&mut self, _from: &str, _to: &str) -> Result<(), Error>` whose default returns `Err(Error::Io(...))`.
> 3. `install.rs`:
>    - `fn target_path<P: Platform>(p: &mut P, dir: &str, dest: &str) -> Result<String, Error>`: `%WINDIR%\X` gives `<windows_dir>\X`, `%SYSDIR%\X` gives `<system_dir>\X`, built by string concatenation (never through `join`, so a UNC Windows folder works); any other `dest` is `join(dir, dest)`. No shell or `None` is `Error::Io("this machine has no Windows folder for <dest>")`. Add `fn is_rooted(dest: &str) -> bool`.
>    - In `do_install`'s file loop: for a rooted file use `target_path`, skip `make_dirs` for its parent, set `pr.file` to the full path, and decide as section 2.3 says: ours already (the `before` receipt's `files` hold the same full path, ASCII case ignored: overwrite, carry over its `O` record), someone else's (exists: `make_dirs` for `BACKUP\WIN` or `BACKUP\SYS` below `dir`, `Shell::copy_file` to `BACKUP\WIN\<name>` or `BACKUP\SYS\<name>`, push the backup to `j.files`, push `(backup_full, original)` to a new `Journal.restores`, note `(original, backup_rel)` for the receipt), or absent (as any file). Record the `F` line with the full path. Change `install` so `before` is the whole earlier `Receipt` (its `file_types` and `files` and the new `overwritten`), not only the file types.
>    - `rollback`: first copy each `restores` backup over its original (last first, errors ignored), then remove files and folders as now. Keep the call order in `install` (rollback, then groups, then file types).
>    - Receipt: a new record `O original|backup` written after the `F` records, escaped with the existing `escape` and parsed with `unescape`; `Receipt` gains `pub overwritten: Vec<(String, String)>`. Update the module documentation's record list.
>    - `uninstall`: after the `F`/`I`/`B` loop, for each `overwritten` entry last first: when the backup exists, `copy_file` it over the original and remove the backup; then try `remove_dir` on `BACKUP\WIN`, `BACKUP\SYS` and `BACKUP` below `dir` (errors ignored), before the `D` records.
>    - Add `fn notice(&mut self, _text: &str) {}` to `Platform` (a log line for tests and `/LOG`, never drawn; `Win16Platform` already has an inherent `note`, hence the other name) and call it with `backup: <original> -> <backup_rel>` for each backup. The runtimes wire it in WP-SYS-4 and WP-SYS-5.
> 4. `sim.rs`: `HostPlatform`'s `Shell` answers `windows_dir` `C:\WINDOWS` and `system_dir` `C:\WINDOWS\SYSTEM` when a new builder flag is set (`with_windows()`, off by default, so DOS-like hosts keep `None`), makes both folders on the host, and implements `copy_file` with `read_file`, `create`, `write` and `close`. Record `notice` lines in a `Vec<String>` the tests can read.
> 5. Tests in `crates/li-retro/tests/system_folders.rs` (a hand-built format 5 `Project` with one install-folder file and one `%WINDIR%` and one `%SYSDIR%` file, packed with `li_lpk::writer::pack` as the existing tests do; copy their setup): fresh install and uninstall leave the Windows folders as they were; a seeded `C:\WINDOWS\A.SCR` is backed up, replaced, listed as `O`, and restored on uninstall with no `BACKUP` left; cancel during copying and a forced write failure both restore the seeded file and leave no backup; reinstall over the first install writes no second backup and keeps the first `O`; a format 4 project with a `%` destination is refused by `validate`; a platform without `with_windows()` fails the install with the "no Windows folder" error and rolls back.
> 6. Docs, in the same commit as the behaviour: paste the SPEC §1, §8 and §9 text and the decisions.md row from section 3 of the plan (D95, or the next free number if `main` has moved on), and update the `install.rs` module comment. SPEC §4's paragraph belongs to WP-SYS-2; leave it.
>
> **Must not touch.** `decode.rs`, `crates/li-lpk`, `crates/li-retro-build`, `crates/li-engine`, `retro/`, `win32/`, `dos/`, `docs/lpk-format.md`, any golden file. If you find you need one of them, stop and report why.
>
> **Check before you commit.** `cargo fmt --all -- --check`, `cargo clippy -p li-retro --all-targets --locked -- -D warnings`, `cargo nextest run -p li-retro -p li-retro-build --locked` (the compiler's tests must still pass on format 5), `bash scripts/check-headers.sh`, and markdownlint in Docker on the two docs. Report the commands and results exactly.
>
> **Git.** Stage explicit paths (never `git add -A`). One commit, `feat(retro): install a file into the Windows or system folder`, body in markdown with bold labels, hard-wrapped at about 72, written to a file and committed with `git commit -F`, ending with `Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>` (your own model). Push the branch and open a PR: capitalized imperative title, `## Summary` bullets with bold lead-ins, `## Test plan` with `- [x]` items and a last unchecked "Not verified:" line. Do not merge. Remove the worktree only after the orchestrator says the branch is merged.
>
> **Report.** The PR URL, the tests run, anything in the plan you changed and why, and any `OWNER DECISION: question | default taken | where to swap` lines.

### Brief: WP-SYS-2

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/system-folders-check`, branch `feat/system-folders-check` from `origin/main`. Specification: `docs/plans/system-folders.md` sections 2.1 and 2.7. In `crates/li-engine/src/machine.rs` add `windows: Option<PathBuf>` and `system: Option<PathBuf>` to `Dirs`, answer `"windows"` and `"system"` in `Dirs::get` (through `as_deref`), and add both to `NAMES`; update every `Dirs { … }` (`sim.rs` `/sim/windows` and `/sim/windows/system`, made like the others; `real.rs` `None` in `new`, `root/windows` and `root/windows/system` in `sandboxed`; the tests). In `check.rs` add the errors `system-folder-unsupported` (a file rule `to` starting `{windows}` or `{system}` in a component that applies to a target whose renderer is not `win16` or `win32-classic`) and `system-folder-subfolder` (a `/` after the placeholder's `/` that is not the last character), with the messages in Table 2. Tests: one per code in `crates/li-engine/tests/project_and_flow.rs` style, a resolve and unresolve round trip in `machine.rs`, and a `liminst preview` test in `li-cli` that a `win16-x86` project prints the copy as `{windows}/…`. Paste the SPEC §4 paragraph from section 3 of the plan (it names your two codes); WP-SYS-1 owns §1, §8, §9 and D95. Do not touch `li-retro`, `li-retro-build` or `li-studio`. Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo nextest run -p li-engine -p li-cli --locked`. Commit `feat(engine): know the Windows and system folders and check where they are used` with the house body and `Co-Authored-By` trailer for your model via `git commit -F`; push; open the PR in the house format; do not merge. Report the PR URL, the tests and any `OWNER DECISION:` lines.

### Brief: WP-SYS-3

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/system-folders-compile`, branch `feat/system-folders-compile` from `origin/main` after WP-SYS-1 and WP-SYS-2 have merged (rebase onto them if they merge while you work; never merge `main` in). Specification: `docs/plans/system-folders.md` sections 2.1, 2.2 and 2.9. In `crates/li-retro-build/src/payload.rs`, give `dos_rel` and `win_rel` a flavour (or add rooted variants used by `build`'s `rel_of`) so `{windows}/NAME` and `{system}/NAME` become `%WINDIR%\NAME` and `%SYSDIR%\NAME` for Win16 (8.3 upper case) and Win32 (case kept), a folder after the placeholder is refused, `Flavour::Dos` refuses both with "`{windows}` and `{system}` have no meaning on DOS", and an `{install}` path whose first part is `%WINDIR%` or `%SYSDIR%` is refused as reserved. Shortcut targets and `open_with` keep the `{install}` rule. Add Lt. Fizz's `screensaver` component exactly as in section 2.1, with `examples/lt-fizz/installer/dist/screensaver/FIZZ.SCR` holding one line of stand-in text (CR LF). Regenerate the Win16 and Win32 page goldens the existing tests use (their update switches are in the test files) and check by eye that only the components page gained the new unticked item; the DOS goldens must not change. Add `crates/li-retro-build/tests/system_folders.rs`: compile Lt. Fizz for `win16-x86` and `windows-x86`, run it on `HostPlatform::with_windows()` unticked (no `FIZZ.SCR` anywhere) and with `/COMPONENTS=…,screensaver` over a seeded `C:\WINDOWS\FIZZ.SCR` (backup, `O` record, restore on uninstall), and assert a DOS compile of a project with a rooted file fails. Add one sentence to `docs/retro-dos.md`. Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo nextest run --workspace --locked`, `liminst check -C examples/lt-fizz`. Commit `feat(retro-build): compile files for the Windows and system folders` via `git commit -F` with the house body and your model's trailer; push; PR in the house format; do not merge. Report the PR URL, the golden diff summary and any `OWNER DECISION:` lines.

### Brief: WP-SYS-4

> Sonnet builder, Liminal Installer, Windows 3.1 runtime. Worktree `~/source/worktrees/liminal-installer/win16-system-folders`, branch `feat/win16-system-folders` from `origin/main` after WP-SYS-3 merged. Read `docs/retro-win16.md`, D77, D78, D88 and `docs/plans/system-folders.md` sections 2.3 to 2.5 and 2.10. Add `KERNEL 135 GetSystemDirectory pw w` to `retro/win16/tools/imports.tbl` after reading the ordinal from Windows 3.11's `KRNL386.EXE` with `tools/ne_dump.py` (say so in the file's header, as for 58 and 59), regenerate `src/imports.rs` with `tools/gen_imports.py` (never edit it by hand), add `dosio::system_dir()` beside `windows_dir()` with the same 144-byte stack buffer and no new statics, and implement `Shell::windows_dir`, `system_dir` and `copy_file` (through `dosio::copy_file`) and `Platform::notice` (to the `/LOG` file, through the existing `note`) in `plat.rs`. Extend `tools/e2e-wine.sh` for Lt. Fizz: seed `$WINEPREFIX/drive_c/windows/FIZZ.SCR` with known bytes, install with the screen saver ticked (a `/PLAY` `toggle` on the components page, or `/COMPONENTS=`), assert the receipt's `F C:\windows\FIZZ.SCR` (match the folder case-insensitively), the `O` record, `BACKUP/WIN/FIZZ.SCR` equal to the seeded bytes and the log's `backup:` line; uninstall and assert the seeded bytes are back and `BACKUP` is gone. Do the same in `tools/e2e-w31.sh` for `C:\WINDOWS` on Windows 3.11. Update `docs/retro-win16.md` with the import, the measured heap peak with the saver ticked and the segment report. Do not touch `li-retro`, the packer, the segment budgets or the Win32 runtime. Run `make win16` and `retro/win16/tools/e2e-w31.sh`; paste the segment report and heap peak in the PR. Commit `feat(win16): install into the Windows and system folders` via `git commit -F` with the house body and your model's trailer; push; PR; do not merge. Report the PR URL, the ordinal evidence, the heap peak and any `OWNER DECISION:` lines.

### Brief: WP-SYS-5

> Sonnet builder, Liminal Installer, Win32 runtime. Worktree `~/source/worktrees/liminal-installer/win32-system-folders`, branch `feat/win32-system-folders` from `origin/main` after WP-SYS-3 merged. Read `docs/retro-win32.md`, D62, D66, D87 and `docs/plans/system-folders.md` sections 2.3 to 2.5 and 2.10. Declare `GetWindowsDirectoryA` and `GetSystemDirectoryA` in `win32/setup/src/sys.rs` the way the other kernel32 imports are (`raw-dylib`, ANSI only), add both to `win32/tools/imports.allow` in order, and implement `Shell::windows_dir`, `system_dir` (`MAX_PATH` buffer, trailing backslash trimmed) and `copy_file` (`CopyFileA` with `bFailIfExists` 0, an `Error::Io` naming both paths on failure) and `Platform::notice` (a `notice:` line in the `/DUMP` file) in `platform.rs`. Extend `win32/tools/wine-e2e.sh` for Lt. Fizz: seed `$WINEPREFIX/drive_c/windows/FIZZ.SCR`, tick the screen saver in the `/PLAY` script, make the receipt checker map an `F` name that starts with a drive (`C:\`) to `$WINEPREFIX/drive_c/…` case-insensitively instead of joining it to the install folder, assert the file, the `O` record and `BACKUP/WIN/FIZZ.SCR`, uninstall and assert the seeded bytes are back and `BACKUP` is gone; keep Planboard's run as it is. If the ticked run changes the page dump, update `crates/li-retro-build/tests/golden/win32/lt-fizz.pages.txt` from the host test, not by hand. Update `docs/retro-win32.md`. Do not touch `li-retro`, the Win16 runtime, `win32.json` or the Pentium scan. Run `WIN32_REQUIRE_TOOLS=1 make win32` and report the `SETUP.EXE` size before and after. Commit `feat(win32): install into the Windows and system folders` via `git commit -F` with the house body and your model's trailer; push; PR; do not merge. Report the PR URL and any `OWNER DECISION:` lines.

### Brief: WP-SYS-6

> Sonnet builder, Liminal Installer Studio. Worktree `~/source/worktrees/liminal-installer/studio-system-folders`, branch `feat/studio-system-folders` from `origin/main` after WP-SYS-2 merged. Read `AGENTS.md` (i18n, CSS Modules, ts-rs) and `docs/plans/system-folders.md` section 2.8. Add `system_folders: Vec<String>` to `ComponentInfo` in `crates/li-studio/src/studio.rs`, filled from the component's file rules (`"windows"` then `"system"`, each once), regenerate the bindings with `bash scripts/generate-protocol-bindings.sh` and commit the generated file. In `apps/studio/src/screens/ProjectScreen.tsx` append ` · ` and `t('project.writesWindows')` ("installs into the Windows folder") or `t('project.writesSystem')` ("installs into the Windows system folder") to the component's line, with both keys in `src/i18n/messages.ts` in sentence case; give the demo data in `src/lib/demoApi.ts` one component with `system_folders: ['windows']`, and add the field to every test fixture that builds a `ComponentInfo`. Add a `li-studio` test that Lt. Fizz's `screensaver` component reports `["windows"]` (if WP-SYS-3 has not merged, use a fixture project) and a UI test that the text shows. Do not touch `packages/chrome`, the tokens or the Media screen. Run `cargo nextest run -p li-studio --locked`, `bash scripts/check-bindings-drift.sh`, `bun run format:check`, `bun run typecheck`, `bun run test:js`, `bun run check:i18n`, and look at `?locale=en-XA` and `?locale=ar-XB`. Commit `feat(studio): say when a component installs into the Windows folder` via `git commit -F` with the house body and your model's trailer; push; PR; do not merge. Report the PR URL and any `OWNER DECISION:` lines.

### Brief: WP-SYS-8 (Quill)

> Sonnet builder, Quill (`~/source/experiments/quill16`, its own `AGENTS.md`). Worktree `~/source/worktrees/quill16/installer-ghostwriter`, branch `feat/installer-ghostwriter` from `origin/main`, after WP-SYS-4 and WP-SYS-5 have merged in Liminal Installer. Add the two components in `docs/plans/system-folders.md` section 6 (WP-SYS-8) to `installer/components.toml`, after the file-type components. In `installer/tools/stage.sh` copy `QUILLGW.SCR` into `dist/win16-x86/` and `AGQUILL.SCR` into `dist/windows-x86/` from `QUILLGW_SCR` and `AGQUILL_SCR` (document both in the usage comment), and add both to the `--stand-ins` list; when neither the variable nor a built file exists, fail with a message that names the variable. Set `installer/liminst.rev` to the Liminal Installer commit that holds WP-SYS-4 and WP-SYS-5. Extend `installer/tools/e2e-w31.sh` and `e2e-w95.sh` with one run that ticks the saver and checks `C:\WINDOWS\QUILLGW.SCR` or `C:\WINDOWS\AGQUILL.SCR` after install and its absence after uninstall. Run `installer/tools/check.sh` on stand-ins and the two e2e scripts; report the floppy's free bytes before and after. Commit `feat(installer): offer the Ghostwriter screen saver as an option` via `git commit -F` with the house body and your model's trailer; push; PR in Quill's house format. Report the PR URL, the free bytes and any `OWNER DECISION:` lines.
