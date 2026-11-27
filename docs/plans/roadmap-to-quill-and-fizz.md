# Plan: the road to Quill's installer proving the chain, then Lieutenant Fizz's

Quill's installer is the first real installer (decided 2026-11-27): it is small, it covers DOS, Windows 3.x and Windows 95, and each of its needs becomes a general installer feature that Lieutenant Fizz then gets for free. This plan orders the work from `main` at `083b99a` to the point where Quill's installer has proved the whole chain (project, Studio, compiler, packs, media, three runtimes, lifecycle, release assets, evidence), and then to a real Lieutenant Fizz installer. It is a roadmap over the existing plans, not a replacement for them: where [release-media-plan.md](release-media-plan.md), [studio-media-screen-plan.md](studio-media-screen-plan.md), [system-folders.md](system-folders.md), [win16-headroom.md](win16-headroom.md) or [shared-libraries.md](shared-libraries.md) already specify a piece, the work package here points at it and adds only what changed.

**Markers.** [V] checked in the code, the docs or a repository at the path given; [M] measured for this plan (sizes from Python `zlib` raw deflate at level 9 in independent chunks plus the 12-byte chunk headers, which is how the media plan measured, so expect a few per cent either way against `li-lpk`'s `miniz_oxide`); [U] not checked, and the implementer must check it. Nothing was built for this plan.

## Table of contents

- [1. Where things stand](#1-where-things-stand)
- [2. The finish line](#2-the-finish-line)
- [3. What changed since the media plan: Quill's programs doubled](#3-what-changed-since-the-media-plan-quills-programs-doubled)
- [4. Capabilities](#4-capabilities)
- [5. Budgets](#5-budgets)
- [6. The serial: `QD05-0010` and the FAT volume serial](#6-the-serial-qd05-0010-and-the-fat-volume-serial)
- [7. Modify, repair and update on the retro runtimes](#7-modify-repair-and-update-on-the-retro-runtimes)
- [8. Release assets, signing keys and signed feeds](#8-release-assets-signing-keys-and-signed-feeds)
- [9. Lieutenant Fizz: what it needs](#9-lieutenant-fizz-what-it-needs)
- [10. Test beds and evidence](#10-test-beds-and-evidence)
- [11. What the owner can test on real machines](#11-what-the-owner-can-test-on-real-machines)
- [12. Decisions](#12-decisions)
- [13. Owner questions](#13-owner-questions)
- [14. Risks](#14-risks)
- [15. Work packages](#15-work-packages)
- [16. Sonnet briefs](#16-sonnet-briefs)

## 1. Where things stand

| Capability | State on `main` (`083b99a`) | Source |
|---|---|---|
| Optional components that start unticked, operations gated by component, `/S` and `/COMPONENTS=` | Merged (D89, D90) | [V] decisions.md |
| File type operation: Win32 `HKCR`, Win16 `WIN.INI` `[Extensions]`, previous owner restored | Merged (D86 to D88), tested under Wine and on Windows 3.11 with Planboard's `.pbd` | [V] decisions.md, retro-win16.md Table 3 |
| `{windows}` and `{system}` destinations, backups, `O` records (WP-SYS-1 to WP-SYS-6) | Merged (D95, D97 to D100) | [V] |
| Win16 leaner allocator, 36 KiB arena (WP-W16-1) | Merged (D96): data 45,144 bytes, about 4,000 free | [V] retro-win16.md Table 2 |
| Win16 gates, data diet, shallower stack, packer trims (WP-W16-2 to WP-W16-5) | Planned, not started | [V] win16-headroom.md |
| Floppy writer with folders, label and serial (media plan M1) | Merged (D80) | [V] |
| `COMMON.LPK` convention and golden, `Reader::begin_for` (M2) | Merged (D81, D82); no runtime reads a second pack | [V] |
| `li-media` wire types, `MediaWarning`, `validate` (S1) | Merged (D83); nothing depends on `li-media` yet, and `installer.toml` does not parse `[[media]]` | [V] `crates/*/Cargo.toml`, `li-model/src/project.rs` |
| Studio Media screen on demo data (S2 to S4) | Merged (D84) | [V] |
| `[[media]]` in the model, dedup into `COMMON.LPK`, `others` (M3) | Not built | [V] grep |
| Runtimes read `COMMON.LPK` (M4), `liminst build --media` (M5), redirect (M6), ISO and CD (M7), spanning (M8) | Not built | [V] grep |
| Studio's real media planner (S5), builds (S6), CD (S7) | Not built | [V] |
| Modify, repair, update, launch on DOS, Win16, Win32 | Not built: install and uninstall only | [V] retro-dos.md, retro-win16.md, retro-win32.md |
| Release: tag, runtimes and `liminst` as assets | No tag, no release; `release.yml` builds `liminst` and the engine runner but no retro runtime | [V] `git tag`, `gh release list`, `release.yml` |
| Feed signing | `liminst feed add --sign-key` is implemented (`li-cli/src/feed.rs`); `docs/releasing.md` still says it is not | [V] |
| Real Windows machine for the engine runner | The real machine's registry is a stand-in (a JSON file); nothing in the Tauri path has run on Windows | [V] tauri-apps.md |
| Quill's installer | Three per-OS floppies (INS-3), `.QLD` by default, `.RTF` and `.TXT` as `default = false` components; pinned to `1ab614f`, before D89, so the two optional types still start ticked; no Ghostwriter components yet (WP-SYS-8) | [V] Quill `origin/main` `a004a70` |
| Quill print set | Open in Quill (pull request 190, INS-23 to INS-25): labels for the DOS, Windows 3.x, Windows 95 and all-in-one disks, printed serial `QD05-0010` for 0.1.0 | [V] Quill `origin/feat/print-set` |

Table 1. State at the start of this plan. Open in this repository: pull request 59 (optional components on Windows 3.1), which this plan does not touch.

## 2. The finish line

**Quill's installer fully proves the chain** when all of the following hold, each with evidence in the repository that produced it:

1. Quill's `installer/` is checked and previewed by `liminst` and edited in the Studio with no hand step: its media are planned by the Studio's real planner, and the sizes the Studio shows equal `SIZES.txt` byte for byte.
2. `installer/tools/build-media.sh` in Quill builds, from one command and a pinned installer **release** (not a commit), the three per-OS floppies, the all-in-one CD (`COMMON.LPK` shared by three targets, `AUTORUN.INF`) and the two-disk all-in-one floppy set, with the label and serial of section 6, the size gate and `SHA256SUMS`.
3. From those images, in emulators: DOS installs from `A:\SETUP` and from the CD; Windows 3.11 installs from `A:\WIN31\SETUP` and from the CD; Windows 95 installs from `A:\WIN95\SETUP` and by AutoPlay; each uninstalls to the state before; `.QLD` is associated by default and `.RTF` restores the previous owner when ticked; the Ghostwriter saver lands in the Windows folder when ticked and is listed by Display Properties; the wrong-OS cases redirect; the two-disk set asks for disk 2 only when a disk-2 component is ticked; modify, repair and update run from the media on all three.
4. The Win16 gates of the headroom plan pass on every Quill run (data at least 1,024 bytes free, heap margin at least 4,096, stack margin at least 2,048).
5. The installer has a tagged release whose assets are `liminst`, the three retro runtimes and a signed `SHA256SUMS`, and Quill's build verifies the signature before it uses them.
6. Wine, Windows 3.11 in DOSBox-X, Windows 95 in DOSBox-X and ReactOS each have a passing run recorded, and the owner's real-machine runs (section 11) are recorded in `docs/real-machines.md`, or listed there as not done.

Lieutenant Fizz then needs the Windows and release pieces in section 9, which Quill does not exercise.

## 3. What changed since the media plan: Quill's programs doubled

The media plan measured Quill's programs at 197 KB (DOS), 327 KB (Windows 3.x) and 361 KB (Windows 95) and found the all-in-one floppy fitting with the Canadian dictionary and about 171 KB free. The programs staged on 2026-11-27 (a Quill worktree's `installer/dist/`, after the zoom, print and picture work) are much larger [M]:

| File | Raw | Packed, 32 KiB | Packed, 4 KiB | Media plan's packed figure |
|---|---:|---:|---:|---:|
| `QUILLDOS.EXE` | 438,272 | 262,618 | 284,734 | 112,885 (32 KiB) |
| `QUILL.EXE` | 425,424 | 198,848 | 213,731 | 157,507 (4 KiB) |
| `QUILL32.EXE` | 441,856 | 269,264 | 290,712 | 216,035 (32 KiB) |
| `AGQUILL.SCR` (afterglow-savers, i586) | 171,008 | 88,588 | 96,980 | not counted |
| `EN_CA.DIC` | 300,086 | 201,050 | 210,547 | 210,547 (4 KiB) |
| Win16 `SETUP.EXE` | 251,223 | | | 206,179 |

Table 2. Quill's payload now. The Win32 `SETUP.EXE` is 130,048 bytes (D99); the DOS one about 146,000 at `3f615e5` [V, cached build] and taken as 150,000 below. `SETUP.PRJ` is budgeted at 16,000 per target [U], as the media plan did. `QUILLGW.SCR`, the Windows 3.x saver, is not built yet [U].

| Layout (1,457,664 bytes of data area) | Used | Free | Fits? |
|---|---:|---:|---|
| DOS floppy (`QUILL DOS`), three dictionaries | 1,058,816 | 398,848 | Yes |
| Windows 3.x floppy (`QUILL W31`), three dictionaries | 1,142,272 | 315,392 | Yes, before `QUILLGW.SCR` |
| Windows 95 floppy (`QUILL W95`), three dictionaries and `AGQUILL.SCR` | 1,136,140 | 321,524 | Yes |
| All-in-one, `COMMON.LPK` at 4 KiB, no dictionary | 1,361,408 | 96,256 | Yes, 80 KB after the 16 KiB reserve |
| All-in-one, `COMMON.LPK` at 4 KiB, `EN_CA` only | 1,571,840 | −114,176 | **No** |
| The same with WP-W16-5's file trim (about −41 KB) | | about −73,000 | No |
| All-in-one, three dictionaries | 1,990,656 | −532,992 | No |
| Two-disk all-in-one: disk 2 holds `COMMON.L02` with the three dictionaries (about 630 KB) | | about +800,000 on disk 2 | Yes, with spanning |
| CD, everything | about 2.6 MB | | Yes |

Table 3. Fit at today's sizes [M, rounded to 512-byte clusters with two folder clusters and a 2 KB root `README.TXT`].

So:

- **The all-in-one floppy is a two-disk set or nothing.** With any dictionary it is over by 70 to 115 KB; without one it fits with 80 KB to spare, which is enough for disk 1 of a spanned set (programs, samples, help) with the dictionaries on disk 2. That moves spanning (media plan M8) onto the path of the all-in-one floppy, and it makes the per-OS floppies (which already exist and which the print set already labels) the floppy release for 0.1.0 (D115).
- **The CD carries the shared-pack proof.** Space is no object there, but it is the one medium that puts three targets and one `COMMON.LPK` side by side without spanning, so it proves the runtime common-pack reader first.
- **The savers fit only the per-OS disks.** `AGQUILL.SCR` and `QUILLGW.SCR` are left off the two-disk set with `exclude` unless disk 2 has room (it has: about 800 KB, so they may ride on disk 2 as unticked components, which is also the honest test of "disk 2 is asked for only when a disk-2 component is ticked").

## 4. Capabilities

Each capability names its finish line, what it needs, the effect on each runtime, the tests and the evidence. Budgets are in section 5 and the packages in section 15.

### 4.1 Media in the project model (`[[media]]`)

- **Finish line.** `installer.toml` parses `[[media]]` and `[[media.target]]` (SPEC §13's fields plus `serial_prefix` and `catalogue`, D114); `liminst check` reports every media rule (label, serial, 8.3 folder used once, known retro target, `exclude` naming a known component, `autorun` naming a Win32 target on a CD only, 224 root entries); the automatic serial rule is in `li-media` with test vectors.
- **Needs.** Nothing: unblocked now.
- **Runtimes.** None.
- **Tests.** Unit tests in `li-model` and `li-media`; a `liminst check` test on the Lt. Fizz example with a floppy and a CD medium; bindings drift.
- **Evidence.** The test names in the pull request.

### 4.2 Compiling a medium: dedup, `COMMON.LPK`, `others`, the text table

- **Finish line.** `li-media` compiles every target of a medium with the existing per-target compiler, groups identical files (size, CRC-32, bytes) used by two or more targets into `COMMON.LPK` (`Cnnnnnnn` names, union target mask, the smallest consumer's chunk size, D81), and each target's compiled project records `CommonRef { path, set_id, len }`, `FileEntry.pack`, `others` (the other targets' flavour and path) and a `texts` table for the new runtime messages (D116, D117: compiled format 6).
- **Needs.** 4.1.
- **Runtimes.** None yet: a format 6 project is refused by today's runtimes, as every bump has been. `FORMAT_MIN` stays 2.
- **Tests.** Host: a three-target Lt. Fizz medium gives one common entry per shared file at 4 KiB with mask 7; each project names the right set id and relative path; `common = "none"` gives byte-identical packs to today's `-t` builds; golden decoder test of format 6 by hand and by serde (D72's cross-check).

### 4.3 The runtimes read `COMMON.LPK`

- **Finish line.** Media plan M4: `Platform` opens a second pack by `PackId`; `li_retro::pack::Packs` shares one decoded-chunk window; the common pack is opened lazily at `commit`; a missing file or a wrong set id stops before any write with the message from the project's text table; uninstall never needs it.
- **Needs.** 4.2; WP-W16-2's gates merged first, so the heap and stack cost is measured by the gate rather than by hand.
- **DOS.** A second DOS handle (function 3D00h). **Win16.** A second `_lopen`; heap gains the common directory and one `Reader`. **Win32.** A second `CreateFileA`.
- **Tests.** Sim: install from two packs is byte-equal to a one-pack install; missing and wrong `COMMON.LPK` fail before a write; uninstall with `COMMON.LPK` deleted passes. DOSBox Staging, Wine and Windows 3.11 runs from a shared Lt. Fizz tree; the Win16 `/LOG` heap and stack lines read by the gate.

### 4.4 `liminst build --media`: floppy, zip, `SIZES.txt`, gate, README

- **Finish line.** Media plan M5, written in `li-media` (D83): the loose tree, the `floppy-1440` image (label, serial from D114), the zip, `SIZES.txt` (each pack's entries raw and packed, free bytes and the margin), the `reserve` gate with a non-zero exit and the per-file overrun, a generated `README.TXT` (one paragraph per target, the catalogue line) unless `readme` is set, and `extra` files. `liminst build -t` is unchanged.
- **Needs.** 4.2 and runtimes found as today (`--dos-runtime` and the others). End-to-end runs need 4.3.
- **Tests.** CLI test with stand-in runtimes: the image's tree, label and serial (`li_package::dos::read_floppy` and `boot_label_and_serial`, mtools when present); an oversized payload exits 1 naming the largest files; `SIZES.txt` equals the plan's numbers.

### 4.5 The ISO writer and CD output

- **Finish line.** Media plan M7: `li-package::iso` writes ISO 9660 level 1 plus a Joliet supplementary descriptor, L and M path tables, 2,048-byte sectors and `SOURCE_DATE_EPOCH` timestamps, in pure Rust; the `cdrom` kind builds the tree, `AUTORUN.INF` (`open=WIN95\SETUP.EXE`, `icon=WIN95\SETUP.EXE,0`, CRLF) and `<slug>-<v>-cd.iso`.
- **Needs.** The writer needs nothing (unblocked now); the `cdrom` kind needs 4.4.
- **Runtimes.** None: all three open their packs read-only already [V, media plan 0.3].
- **Tests.** Round trip and `xorriso -indev X.iso -find` and `isoinfo -l -J` when present; DOSBox Staging `imgmount d X.iso -t iso` and a Planboard install from `D:\SETUP`; Windows 3.11 in DOSBox-X with its own MSCDEX; Windows 95 bed with `-i ISO` and an AutoPlay screenshot.

### 4.6 The Studio's real media planner

- **Finish line.** Studio plan S5: `li_media::plan`, `edit` and `list` with a `SizeCache`; `DocKey::Media`; the Tauri commands; the MCP tools `list_media`, `get_media_layout`, `estimate_media`, `set_media`; `liminst media plan <id> [--json]`. The Media screen shows Quill's real media, and its numbers equal `SIZES.txt`.
- **Needs.** 4.2 and 4.4 (the plan and the build share the sizing code, so S5 lands after M5 to avoid two owners of `li-media`'s sizing).
- **Tests.** The exactness test (plan versus build on the same input); CLI and MCP JSON equal; Vitest with the real `Api` in the Tauri shell; a screenshot of Quill's media in the running Studio.

### 4.7 The wrong-OS redirect

- **Finish line.** Media plan M6 and 2.5: DOS calls INT 2Fh 1600h and 4680h and, under Windows, shows a page that names the right `SETUP` and offers D to install the DOS version or Esc; Win16 under Windows 95 or later (`GetVersion` 3.95 or more) asks to start `WIN95\SETUP.EXE` and starts it with `WinExec`; the text comes from the project (D116).
- **Needs.** 4.2 (for `others` and `texts`). It does not need 4.3.
- **DOS.** The INT 2Fh calls through `dpmi.rs` [U: the real-mode interrupt path]. **Win16.** `WinExec` (KERNEL 166 [U], read with `ne_dump.py`). **Win32.** Nothing.
- **Tests.** DOSBox Staging: no redirect. A DOS box in Windows 3.11 (DOSBox-X): the page, then D installs. Windows 95 bed: `A:\WIN31\SETUP` starts `A:\WIN95\SETUP`.

### 4.8 Disk spanning

- **Finish line.** Media plan M8: `open_volume(n)` asks for disk `n` (DOS a text page, Win16 `MessageBox` Retry and Cancel, Win32 `MessageBoxA`), refuses a wrong disk and asks again, handles cancel as a rollback; `overflow = "span"` writes one image per disk with labels `<label>` and `<label> 2`; the compiler orders unticked components (and then the largest files) last, so disk 2 is asked for only when a disk-2 component is ticked; the Studio's span view (S10) is live.
- **Needs.** 4.3, 4.4.
- **Tests.** Sim disk swap; DOSBox Staging `imgmount a d1.img d2.img -t floppy` with Ctrl-F4; Windows 3.11 in DOSBox-X with `imgmount` and the swap key; Windows 95 bed [U: floppy swap in DOSBox-X under Windows 95's own driver]; the `common-span` golden.

### 4.9 Modify, repair and update on the retro runtimes

- **Finish line.** Section 7: `SETUP` from the media finds an installed copy and offers Modify, Repair and Remove for the same version, Update for an older one, and refuses a newer one; all three runtimes; `/M` and `/R` switches; repair re-checks size and CRC-32 by streaming.
- **Needs.** 4.3 (repair and update read `COMMON.LPK`), WP-W16-4 merged (data room), ideally WP-W16-3.
- **Tests.** Host first (the sim with a damaged file, a deleted file, a component added, one removed, an older receipt, a newer receipt), then Wine, Windows 3.11 and DOSBox Staging, then Quill.

### 4.10 Release assets, signing keys, signed feeds

- **Finish line.** Section 8: `release.yml` builds the three retro runtimes and publishes them with `liminst` and a signed `SHA256SUMS`; the release key exists in the owner's custody; Quill's `liminst.sh` takes a pinned release and verifies it; a tagged `v0.1.0` (the owner's go).
- **Needs.** Nothing in code; the tag needs the owner.

### 4.11 Quill's side

- **Finish line.** In Quill: the installer pin moved to today's `main` and the Ghostwriter components (WP-SYS-8, extended here); `build-media.sh` in place of `build-disks.sh` with the media entries, the serial and the labels of section 6; the e2e scripts from the final images; a release workflow (media plan Q1 to Q7).
- **Needs.** The installer pieces above, pinned in order.

### 4.12 Evidence beyond Wine and DOSBox-X

- **Finish line.** A ReactOS run of the Win32 `SETUP.EXE` (install, Start menu, Add/Remove Programs, file type, uninstall) in a throwaway copy of the Afterglow image under the builder's own headless QEMU; `docs/real-machines.md` with the owner's checklist and a results table (D122).
- **Needs.** reactos-liminal's image and guest driver [V: `scripts/guest/guest.py` is the savers' WP-PACK-2 driver; the installer has no ReactOS harness yet].

## 5. Budgets

### 5.1 The Win16 data segment

About 4,000 bytes free now (65,536 less 45,144 of data and 16,384 of stack and local heap) [V]. The headroom rule (win16-headroom.md section 5, pasted by WP-W16-2): nothing merges into `retro/win16` or `li-retro` with less than 1,024 free, and relief lands before the features that spend it.

| Step | Data spent (+) or freed (−) | Free after, about |
|---|---:|---:|
| Today | | 4,000 |
| WP-W16-2 gates | +200 at most | 3,800 |
| WP-W16-4 data diet | −800 to −1,530 | 4,600 to 5,330 |
| WP-INS-6 common-pack reader | +400 at most | 4,200 to 4,930 |
| WP-INS-10 redirect (text in the project, D116) | +250 at most | 3,950 to 4,680 |
| WP-INS-13 spanning | +250 at most | 3,700 to 4,430 |
| WP-INS-15 maintenance on Win16 | +900 at most | 2,800 to 3,530 |
| WP-W16-3 stack to 6 KiB | about −2,048 [U] | 4,850 to 5,580 |

Table 4. Win16 data ledger. Without D116 the redirect, set-id and disk-prompt messages and the maintenance pages would cost 1.5 to 2.5 KB of read-only data on top; with it, their text lives in `SETUP.PRJ`, which is read into the heap.

### 5.2 The Win16 heap

The 36 KiB arena peaks at 29,688 bytes for Lt. Fizz and 30,488 with the saver (D96, D98) [V]. Quill's peak on the current runtime has not been measured [U]: it was 36,168 of 40,960 under the old allocator (Quill INS-5), which the new one should bring to about 29 to 31 KB. The gate asks for 4,096 of margin. Expected additions: the common directory and second `Reader` (about 600 bytes for Quill's ten or so shared entries), the text table (about 1 KB with every message), maintenance (the receipt is read line by line, never held; the component diff is a few hundred bytes). If Quill's margin falls under the gate, the fallback is WP-W16-6's one pool (an owner go), not a smaller feature.

### 5.3 Win16 code and the other runtimes

- **Win16 code** is 178,851 bytes in five segments, each under its 40,000 budget [V]. Each feature here adds 1 to 4 KB [U]; a sixth segment is acceptable and must be reported. WP-W16-5 (copies of small functions, the data segment's zero tail out of the file) takes about 7 KB of code and 41 KB of file off.
- **DOS** `SETUP.EXE` has no gate; report the size. Memory is not a constraint (DPMI).
- **Win32** `SETUP.EXE` has no gate; report the size; every new import must be on `win32/tools/imports.allow` and exported by Windows 95 OSR2's DLLs (checked as D99 did).

### 5.4 Floppies

| Disk | Free now | Reserve | What can still go on it |
|---|---:|---:|---|
| `QUILL DOS` | about 399 KB | 16 KiB (raised from Quill's 64 KB minimum only if the owner wants) | Program growth |
| `QUILL W31` | about 315 KB | 16 KiB | `QUILLGW.SCR` [U] and growth |
| `QUILL W95` | about 322 KB with `AGQUILL.SCR` | 16 KiB | Growth |
| All-in-one disk 1 | about 96 KB with no dictionary | 16 KiB | About 80 KB of growth across three programs, which is two months of Quill at the recent rate; WP-W16-5 adds 41 KB |
| All-in-one disk 2 | about 800 KB | 16 KiB | The dictionaries and the savers |

Table 5. Floppy budgets [M]. The two programs that grew most (`QUILLDOS.EXE` and `QUILL32.EXE`) are Quill's to watch; the installer's size gate fails the build rather than writing a disk that does not fit.

## 6. The serial: `QD05-0010` and the FAT volume serial

The owner decided the serial is `QD05-0010` (Quill INS-25): the catalogue prefix `QD05`, then the version as one, two and one hexadecimal digits (Fizz's scheme), so 0.1.0 is `QD05-0010`. The media plan wrote `QD01-MMmP`, and `build-disks.sh` sets no serial at all (mtools keeps the one `liminst` wrote, `4C49-5253`, `LEGACY_SERIAL`) [V].

A FAT volume serial is 32 bits shown as eight hexadecimal digits; `Q` is not one, so `VOL` can never print `QD05-0010` [V: `li_media::check_serial`]. Fizz's prefix `FE01` happens to be hexadecimal; Quill's is not. The reconciliation (D114):

- **The printed number is the catalogue number.** Labels, booklet and README say `QD05-0010`. It is text, so the installer carries it as `catalogue` and writes it into the generated `README.TXT` (`Catalogue number QD05-0010`).
- **The FAT serial follows the same rule with a hexadecimal prefix.** `serial = "auto"` is `serial_prefix` (four hexadecimal digits) then the version as major (one digit), minor (two) and patch (one). Quill sets `serial_prefix = "0D05"`, so `VOL A:` prints `0D05-0010` for 0.1.0: the same eight characters with `Q` read as `0`. Fizz's own `FE01` reproduces `make-release.sh` exactly, which is the test vector that keeps the two schemes one rule. A version with a major or patch over 15 or a minor over 255 is an error, as in Fizz.
- **One serial per medium, the same on every disk of a set.** A spanned set's disk 2 has the same serial and the label `<label> 2` (`QUILL 2`); the set id inside the packs, not the serial, tells the runtime the disk is right.
- **Labels.** The per-OS floppies keep `QUILL DOS`, `QUILL W31` and `QUILL W95` (INS-3 and the print set); the all-in-one set is `QUILL` and `QUILL 2`; the CD's ISO volume id is `QUILL` and its Joliet label `Quill 0.1.0`.
- **Where Quill changes.** `build-disks.sh` (until `build-media.sh` replaces it in WP-INS-11) sets the serial with `mlabel -N 0D050010`, computed from the version with the same rule, so the per-OS disks carry it before media exist.

## 7. Modify, repair and update on the retro runtimes

SPEC §9 lists modify, repair and update for every runtime; the retro runtimes do install and uninstall only. Quill needs them because a person with the disk should be able to add the US dictionary, put back a deleted `QUILL.EXE` or install 0.2.0 over 0.1.0 without losing their choices. The design (D118, D119):

- **From the media, not from the kept uninstaller.** Adding a component or restoring a file needs the payload, and the payload is on the disk. `SETUP` run from the media looks for an installed copy: Win32 reads `InstallLocation` from the product's Add/Remove Programs key; DOS and Win16 look in the project's default folder, or the folder named by `/D=` [U: whether a `WIN.INI` line naming the folder is worth its operation; the default is no]. `UNINST.EXE` stays remove-only, and the Win32 Uninstall key gains `NoModify` and `NoRepair` set to 1, so Add/Remove Programs offers only Remove.
- **What it shows.** The same version installed: a maintenance page (Modify, Repair, Remove). An older version: an Update page that keeps the receipt's `components=` and goes straight to the ready page. A newer version: a message and exit code 3, nothing written (D54's "the version on the machine wins").
- **Modify** is the component page ticked from the receipt. Added components install their files and operations as a fresh install would; removed components remove the receipt records of the files and operations that the compiled project assigns to them (the project already knows each file's and operation's component, D90), so the receipt format does not change. Required components cannot be unticked.
- **Repair** checks every `F` record by size and CRC-32, reading the installed file in 1 KiB pieces through a new `Platform::read_chunk` (stack buffer, no heap), and restores the missing or changed ones from the packs; shortcuts, groups and file types are applied again where they are missing (`ReplaceItem` and the "still ours" rule make that safe); `O` backups are never touched.
- **Update** installs the new version over the old: files no longer shipped are removed, `O` and `A` records keep what the first install replaced (already the rule), and the receipt is written to a temporary name and renamed, so a failure leaves the old receipt.
- **Pages.** The maintenance and update pages are compiled from base layouts (`maintenance.layout.json`, `update.layout.json`) that a theme may override, and their text is in the project (D116); the compiled format is 7.
- **Switches.** `/M` opens the maintenance page; `/R` repairs; both combine with `/S`. A silent repair that finds nothing to do exits 0.
- **Studio.** The engine already has the modify and repair flows for the host runners; the Dry-run gains a "Run against an installed copy" switch for retro targets once the runtime pages exist (a later Studio package, not on the critical path).

## 8. Release assets, signing keys and signed feeds

- **Runtimes as assets (D120).** `release.yml` gains a `retro-runtimes` job on `ubuntu-24.04` in the shared image that `make dos`, `make win16` and `make win32` already use in CI [U: one image with DJGPP's packer inputs, `lld-link`, LLVM and Python], which builds the three `SETUP.EXE` files and writes `liminal-retro-runtimes-<v>.zip` (`dos/SETUP.EXE`, `win16/SETUP.EXE`, `win32/SETUP.EXE`, the provenance files and a manifest with each SHA-256 and the commit). The publish job adds it beside the `liminst` archives, and signs `SHA256SUMS` with the release key into `SHA256SUMS.sig` (D13's detached Ed25519, by `liminst` itself) when the `RELEASE_SIGN_KEY` secret exists.
- **Keys (D121).** One release key for this repository, made by the owner with `liminst keygen` on the owner's machine, kept offline (a password manager or an encrypted drive, as Fizz's upload key is), with the public half committed as `keys/release.pub`; the private half reaches CI only as `RELEASE_SIGN_KEY`. Each product that publishes a feed has its own feed key (D6, D13), pinned in that product's installer at build time; Lieutenant Fizz is the first. A lost or rotated key is a new public key in a new release and a new pin in each product, never a key change inside a feed.
- **Consumers.** Quill's `liminst.sh` gains `installer/liminst.version`: when set, it downloads the release's `liminst` and runtimes, checks `SHA256SUMS.sig` against a copy of `release.pub` it carries, then `SHA256SUMS`, and only then uses them; `liminst.rev` stays the fallback for unreleased work.
- **Signed feeds** are already implemented in `liminst feed add --sign-key` [V]; what is missing is the action's `sign-key` path being documented as working, a hosted feed, and an update run against it on a real machine (section 9). `docs/releasing.md` is corrected in WP-INS-5.
- **Platform signing.** Authenticode and notarisation stay optional (D10); with no certificate, Windows SmartScreen warns on the first run of an unsigned `SETUP.EXE` or runner, as it does for Fizz's own packages today [V: Fizz `docs/APP.md`, Releases]. Owner question 7.

## 9. Lieutenant Fizz: what it needs

Read from `~/source/liminal-hq/lieutenant-fizz` (`88e9bc7`, its `docs/APP.md` Releases section and `release.yml`) and `~/source/experiments/fizz-dos` (`gh release view v0.5.0`), read only [V].

**Its release today.**

| Channel | What ships | Built by |
|---|---|---|
| Windows x64 and arm64 | `Lieutenant-Fizz_<v>_<arch>_en-US.msi`, `Lieutenant-Fizz_<v>_<arch>-setup.exe` (Tauri's NSIS), unsigned | `tauri build --bundles msi,nsis` on a Windows runner |
| Linux x86_64 and aarch64 | `.deb`, `.rpm`, AppImage, unsigned | `tauri-ci-desktop` image, the org's AppImage workflow |
| macOS | `Lieutenant-Fizz_<v>_universal.dmg`, ad-hoc signed, not notarised | `tauri build --target universal-apple-darwin` |
| Android | Signed APK and AAB, Play internal-track draft | `tauri-ci-mobile` |
| DOS (separate game, `ScottMorris/lieutenant-fizz-dos`) | `fizz-dos-<v>-floppy.img`, a FreeDOS boot floppy, a DOSBox bundle, `disk.zip` (387,383 bytes), a JS-DOS bundle, pre-release | `tools/make-release.sh`: `mkfs.fat -n LABEL -i FE01MmmP`, `mcopy`, an 8 KiB save reserve |

Table 6. Lieutenant Fizz's release channels. The game has no Tauri updater configured and no Windows 3.x or Windows 95 build; the Liminal Installer's Lt. Fizz example uses stand-ins for every target.

**What a real Fizz installer needs, beyond what Quill proves.**

| Need | Why | Package |
|---|---|---|
| The engine's real machine on Windows: the registry, the elevation prompt, the shortcut's AppUserModelID | Nothing in the Tauri path has run on Windows; the registry is a JSON stand-in [V] | WP-INS-19 |
| A one-file Windows setup for a `web` target | `liminst build` stages a folder (`liminal-runner.exe`, `payload.lip`, the project, `manifest.json`) [V]; a person downloads one `.exe` | WP-INS-20 |
| The Tauri runner built for Windows in CI | `runner.yml` checks it on Linux only [V] | WP-INS-20 |
| A signed, hosted feed and an update run against it | `[updates] mode = "liminal"` (Fizz has no Tauri updater, D54) needs a feed the runner trusts | WP-INS-21 |
| An installer release that Fizz's workflow can pin | The composite action installs `liminst` from a release (`docs/releasing.md`) [V] | WP-INS-5 |
| The game's build inputs as plain files | The installer copies files; Fizz's workflow produces bundles. `tauri build --no-bundle` gives the main binary with its assets embedded [U: whether Fizz has `resources` or sidecars to stage beside it] | WP-INS-22 |
| A DOS target from the fizz-dos release | `disk.zip` holds the game's files; the DOS installer floppy is `SETUP.EXE`, `SETUP.LPK` and the game (fits easily) | WP-INS-22 |
| The decision of which channels the Liminal Installer owns | Windows arm64 has no runner target; macOS and Android have their own stores and packages | Owner question 8, D123 |

Table 7. Fizz's needs. Fizz then reuses, unchanged, everything Quill proves: the media writer for its DOS floppy, the label and serial rule (`FE01`), the Windows and system folder rule for its planned screen saver (`FIZZ.SCR`, Fizz pull request 276), optional components and the release assets.

**What blocks Lieutenant Fizz** (in order): Quill's chain first (the owner's order); a real Windows machine backend (WP-INS-19); a one-file Windows setup (WP-INS-20); an installer release (WP-INS-5 and the owner's tag); a signed hosted feed (WP-INS-21, and the owner's feed key and host); the game's input contract (WP-INS-22, in the Fizz repository, with the owner's choice of channels).

## 10. Test beds and evidence

| Bed | Runs | Who | Notes |
|---|---|---|---|
| Host (`cargo nextest`, the sim) | Everything first | Agents | Goldens for pages, receipts, packs |
| Wine 11 on a private Xvfb | Win32 and Win16 `SETUP.EXE` | Agents | `win32/tools/wine-e2e.sh`, `retro/win16/tools/e2e-wine.sh`; one known flaky Planboard uninstall (run twice) |
| Windows for Workgroups 3.11 in DOSBox-X | Win16 | Agents | Image `quill16-dosbox-x:local`, tree `~/.cache/quill16-play/w31-base` (read only), `e2e-w31.sh`; floppy and ISO through `imgmount` in `[autoexec]` |
| Windows 95 OSR2.5 in DOSBox-X | Win32, the Win16 hand-off | Agents | `scripts/win95/run-win95.sh`, plain and IE4 bases; `-a IMG` and `-i ISO` are to add [U: Windows 95's own floppy and IDE CD drivers under DOSBox-X] |
| DOSBox Staging | DOS | Agents | Floppy and ISO `imgmount`, Ctrl-F4 swap |
| ReactOS 0.4.16 under QEMU | Win32 | Agents | A throwaway copy of the Afterglow image, `snapshot=on`, the builder's own QEMU (WP-INS-16) |
| Windows 11 VM (`WinDev2407Eval`) | The engine runner and Fizz | Agents, under the VM notes | Snapshot first, never build Rust in the guest, restore after |
| Windows Me VM | Win32 | Agents only when the owner says it is free | It freezes; the fix is unconfirmed |
| Real machines | Everything | The owner | Section 11 |

Table 8. Where each thing is tested. Every Wine, DOSBox-X and QEMU run uses a private Xvfb (or none), keeps its own process id and kills only that; never `pkill` or `killall` an emulator or Xvfb by name.

**Evidence rule (D122).** A runtime's doc says "verified on" an environment only with a recorded run there: the command, the date, the commit and the result (and a screenshot for anything visual). `docs/real-machines.md` holds the owner's runs; until a row is filled, that environment is "not run", never assumed.

## 11. What the owner can test on real machines

Agents cannot reach real hardware or a Windows 98 install. Each item says what to run and what to look for; the disks come from the latest `build-media.sh` (or a pull request's artefact), and the results go in `docs/real-machines.md`.

1. **Windows 98 SE (a real PC or the owner's own VM).** `A:\WIN95\SETUP` from `QUILL W95`: every page, Browse, the Start menu group, Add/Remove Programs, double-clicking a `.QLD`, ticking `.RTF` then uninstalling (WordPad gets it back), ticking the saver and finding Quill Ghostwriter in Display Properties. A Windows 98 DOSBox-X tree handed to the agents would turn this into an agent test too.
2. **Windows 95 on a real Pentium.** The same, plus the CD: AutoPlay opens `WIN95\SETUP`, and `A:\WIN31\SETUP` offers to start the Windows 95 setup.
3. **Windows 3.1 retail (not 3.11) on a real 386 or 486.** `A:\WIN31\SETUP` from `QUILL W31`: the Program Manager group and its Uninstall item, File Manager opening a `.QLD`, Control Panel's Desktop listing the saver, the uninstall from the group, and the ESP fix (D74) holding on real silicon (a dozen runs; any fault matters).
4. **MS-DOS 6.22 on a real machine.** `A:\SETUP` from `QUILL DOS`, the `QUILLDOS.BAT` launcher, `SETUP /U`; then `A:\SETUP` inside a Windows 3.1 DOS box (the redirect page, and D to install anyway); the CD through MSCDEX (`D:\SETUP`).
5. **Real floppies.** Write each image with `dd` to a 1.44 MB disk, then `DIR A:` and `VOL A:` show `QUILL W31` and `0D05-0010`; read every file back. For the two-disk set: the prompt for disk 2 appears only when a disk-2 component is ticked, a wrong disk is refused, and cancel rolls back.
6. **A burned CD-R.** The ISO on a real drive in DOS (MSCDEX), Windows 3.1 and Windows 95 (Joliet names, AutoPlay).
7. **Windows 10 or 11, 64-bit.** Quill's Win32 `SETUP.EXE`: the UAC prompt, the saver in the Screen Saver dialog from `C:\Windows` (not `SysWOW64`), the `.QLD` association, uninstall from Settings.
8. **Lieutenant Fizz on Windows 11 x64.** The one-file setup: SmartScreen's warning, the fizz-modern pages in light and dark, WebView2 present and absent, Start menu and Settings' Apps list, an update from the feed, uninstall.
9. **Windows Me.** If the VM is unfrozen: the Lt. Fizz procedure in `docs/retro-win32.md`, then Quill.
10. **A 286 or Standard mode.** Only to record the refusal message; the floor is a 386 (D46).

## 12. Decisions

Ids D114 to D124 are reserved by this plan. Main has D100; D101 to D109 are left for work in flight (the headroom rule of WP-W16-2 takes the next free one); D110 to D113 are the shared-libraries plan's. A merge may renumber. Each package pastes its rows into `docs/decisions.md` in the same commit as the behaviour.

| # | Decision | Choice | Why | Pasted by |
|---|---|---|---|---|
| D114 | A medium's automatic serial | `serial = "auto"` is `serial_prefix` (four hexadecimal digits, default `4C49`) then the version's major as one hexadecimal digit, minor as two and patch as one; a version that does not fit is an error. `catalogue` is optional text (at most 16 characters, `{code}` replaced by the four version digits) that the generated `README.TXT` prints. Quill uses `0D05` and `QD05-{code}`; Fizz's `FE01` gives `make-release.sh`'s serial | A FAT serial is hexadecimal and Quill's printed `QD05` is not; one rule with a prefix serves both products and keeps the version readable in `VOL` | WP-INS-1 |
| D115 | What proves the chain for Quill | Three per-OS floppies and the all-in-one CD for 0.1.0; the all-in-one floppy is a two-disk set once spanning lands, with the dictionaries on disk 2 | At today's sizes the all-in-one floppy is 70 to 115 KB over with one dictionary and 80 KB under with none (section 3) | WP-INS-11 (and Quill's own INS decision) |
| D116 | New runtime text comes from the project | Messages and pages added from here on (the wrong-set and missing-pack messages, the redirect page, the disk prompt, the maintenance and update pages) are compiled into a `texts` table and screens in `SETUP.PRJ`, read into the heap, not linked into the runtime's read-only data | The Win16 data segment has about 4,000 bytes; text in the project costs heap at run time and nothing in the segment, and a theme can reword it | WP-INS-4 |
| D117 | Compiled format 6 | `CommonRef { path, set_id, len }`, `FileEntry.pack` (0 own, 1 common), `others` and `texts`; `FORMAT` 6, `FORMAT_MIN` 2; a format 5 decoder refuses 6; `path` is relative to the folder of `SETUP.EXE` and the compiler refuses a drive, a leading `\` or more than one `..` | The media plan's M3 shape, with the text table of D116 in the same bump so there is one bump, not two | WP-INS-4 |
| D118 | Maintenance runs from the media | `SETUP` from the media finds an installed copy (Win32 `InstallLocation`; DOS and Win16 the default folder or `/D=`) and offers Modify, Repair and Remove for the same version, Update for an older one, and exit 3 for a newer one; `UNINST.EXE` stays remove-only and the Uninstall key has `NoModify` and `NoRepair`; `/M` and `/R`; format 7 carries the pages | Adding and restoring need the payload, which is on the disk; a kept uninstaller that offered Modify would have to ask for the disk anyway | WP-INS-14 |
| D119 | Repair streams the check | Each `F` record is checked by size and CRC-32 read in 1 KiB pieces through `Platform::read_chunk` (a stack buffer); missing and changed files are restored from the packs; shortcuts, groups and file types are applied again where missing; `O` backups are left alone | The Win16 heap cannot hold a file, and a size check alone misses a damaged file | WP-INS-14 |
| D120 | The retro runtimes are release assets | A tag publishes `liminal-retro-runtimes-<v>.zip` (the three `SETUP.EXE`, provenance, a manifest) beside `liminst`, and `SHA256SUMS.sig` over `SHA256SUMS` when `RELEASE_SIGN_KEY` is set; products pin a release by version and verify the signature before use | A product that builds the runtimes from a commit rebuilds a toolchain nobody else has; a signed release is what the action and Quill can trust | WP-INS-5 |
| D121 | Release and feed keys | One release key per repository, made and kept offline by the owner, public half in `keys/release.pub`, private half only as a CI secret; one feed key per product, pinned at build time; rotation is a new pin in a new build | D6 and D13 already say per-feed keys and detached Ed25519; this names who holds them and how they change | WP-INS-5 |
| D122 | Evidence names its environment | A doc says a runtime is verified on an environment only with a recorded run (command, date, commit, result); ReactOS under QEMU joins Wine and the DOSBox-X beds for Win32; real-machine results live in `docs/real-machines.md` and come only from the owner | Every run so far is in an emulator, and an unrecorded "works on Windows 98" cannot be checked | WP-INS-16 |
| D123 | Which Fizz channels the Liminal Installer owns | Windows x64 (`windows-x86_64:web`, fizz-modern, native mode from Tauri's `--no-bundle` binary) and DOS (`dos-x86` from the fizz-dos release); Windows arm64, Linux packages, macOS and Android stay on Tauri's own; a Linux `.run` is a later step | The installer's value is the themed install and the receipt on Windows and DOS; the other channels already work and have stores or package managers | WP-INS-22 |
| D124 | A one-file Windows setup for `web` targets | `liminst build` for a Windows `web` target also writes `<slug>-<v>-setup.exe`: the runner with the staged folder appended as a stored `.LPK` and a trailer, which the runner reads from its own file into a temporary folder before it starts | A person downloads one file; an appended pack needs no new format and no installer for the installer | WP-INS-20 |

Table 9. Decisions this plan reserves.

## 13. Owner questions

Each has the default this plan builds on.

| # | Question | Default taken | Where to swap |
|---:|---|---|---|
| 1 | The all-in-one floppy no longer fits with a dictionary: what does 0.1.0 ship on floppy? | The three per-OS floppies (already labelled by the print set) and the CD; the two-disk all-in-one set after spanning | Quill's `installer.toml` media entries and `build-media.sh` (WP-INS-11); D115 |
| 2 | The FAT serial for the catalogue number `QD05-0010` | `0D05-0010` (`serial_prefix = "0D05"`), with `QD05-0010` printed and in `README.TXT` | `serial_prefix` in Quill's media entries (WP-INS-11); D114 |
| 3 | Does the CD share files through `COMMON.LPK`, or store each target's files (`common = "none"`)? | Share: it is the first proof of the common-pack reader | `common` on the CD entry in Quill |
| 4 | Split the dictionaries for the two-disk set? | `dicts` (`EN_CA`, ticked) and `dicts-more` (`EN_US`, `EN_GB`, unticked) on every medium, as the media plan proposed | Quill's `components.toml` (WP-INS-11) |
| 5 | Maintenance from the media only, or also a Modify button in Add/Remove Programs that asks for the disk? | Media only; `NoModify` and `NoRepair` | D118; `win32/setup` Uninstall key (WP-INS-15) |
| 6 | Release key: who makes it and when? | The owner makes it with `liminst keygen` before the first tag and sets `RELEASE_SIGN_KEY`; until then releases publish an unsigned `SHA256SUMS` and Quill keeps pinning a commit | D121; `release.yml` (WP-INS-5) |
| 7 | Windows code signing (Authenticode) for `SETUP.EXE`, the runner and Fizz's setup? | None; SmartScreen's warning is documented, as Fizz does today | D10's secrets in `release.yml` |
| 8 | Which Fizz channels does the Liminal Installer own? | Windows x64 and DOS; arm64, Linux packages, macOS and Android stay Tauri's | D123; Fizz's `installer/installer.toml` (WP-INS-22) |
| 9 | Fizz on Windows: native mode (the installer owns the files) or wrap (run Tauri's NSIS setup silently)? | Native | `[tauri] windows` in Fizz's project |
| 10 | Where is Fizz's feed hosted? | GitHub Pages of the Fizz repository (it already has `pages.yml`), artefacts on the GitHub release | WP-INS-21, WP-INS-22 |
| 11 | When is `liminal-installer` v0.1.0 tagged? | When WP-INS-5 is merged and the Quill per-OS disks and CD pass from it; the owner tags | Section 15 order |
| 12 | DOS and Win16 find an installed copy only in the default folder or `/D=`; also record the folder in `WIN.INI`? | No `WIN.INI` line | D118; a later operation |
| 13 | Raise the floppy reserve from 16 KiB? | 16 KiB on every medium; Quill's per-OS script keeps its 64 KB check until `build-media.sh` replaces it | `reserve` on each medium |

Table 10. Owner questions.

## 14. Risks

- **Quill keeps growing.** The two-disk set's disk 1 has about 80 KB of room after the reserve; three programs that grew 60 to 140 KB each in a month would fill it. The size gate catches it; the levers are WP-W16-5's file trim, moving the samples to disk 2, or a third disk.
- **The Win16 heap with Quill [U].** Quill's peak on the new allocator is unmeasured, and the common directory, the text table and maintenance all add to it. WP-INS-3 measures it first; WP-W16-6 is the fallback.
- **Format churn.** Formats 6 and 7 must land with matching runtimes; Quill pins one release, so a pack and its programs always agree. Two bumps in flight at once would collide: WP-INS-4 owns 6 and WP-INS-14 owns 7, and they are sequential.
- **Emulator media [U].** Windows 95 reading a floppy and an IDE CD under DOSBox-X with its own drivers; if not, the tree is copied onto `D:` and the run says so.
- **The real-mode INT 2Fh path on DOS [U].** The DPMI interrupt call in `dpmi.rs` has not been used for INT 2Fh.
- **`WinExec`'s ordinal [U]** until read from Windows 3.11's `KRNL386.EXE`.
- **Repair on a changed file the person edited.** Repair restores Quill's own files only (`F` records), never documents; a person who replaced `QUILL.HLP` gets the shipped one back. Stated on the page.
- **One CI image for three runtimes [U].** If no shared image builds all three, WP-INS-5 splits into one job per runtime, each with its own image.
- **Windows real machine.** The engine's real registry and elevation have never run; WP-INS-19 is the first time, and the Windows 11 VM's rules (snapshot, no builds in the guest) slow it.
- **Labs and Liminal HQ.** Fizz lives in `liminal-hq`, the installer in `liminal-hq-labs`; pinning a Labs release from a product's release workflow is allowed but is the kind of dependency the Labs charter asks to graduate before it is relied on [U: the charter's wording].

## 15. Work packages

**Order.** At most three builders at once (two planners). Waves overlap where the files allow.

| Wave | Packages | Repository | Needs |
|---:|---|---|---|
| 0 | **WP-INS-1** media in the model and check · **WP-INS-2** ISO writer · **WP-W16-2** gates (headroom plan) | installer | — |
| 0 | **WP-INS-3** Quill pin and Ghostwriter (absorbs WP-SYS-8) | Quill | — |
| 1 | **WP-INS-4** compile a medium (format 6) · **WP-W16-4** data diet · **WP-INS-5** release assets and keys | installer | INS-1; W16-2 for W16-4 |
| 2 | **WP-INS-6** runtimes read `COMMON.LPK` · **WP-INS-7** `build --media` floppy and zip | installer | INS-4; W16-2 |
| 3 | **WP-INS-8** CD output · **WP-INS-9** Studio media planner · **WP-INS-10** redirect | installer | INS-2 and INS-7; INS-7; INS-4 and W16-4 |
| 4 | **WP-INS-11** Quill media build · **WP-INS-12** Quill e2e from the images | Quill | INS-6, INS-7, INS-8 (INS-10 for the redirect runs) |
| 4 | **WP-INS-13** spanning | installer | INS-6, INS-7 |
| 5 | **WP-INS-14** maintenance core and DOS · **WP-W16-3** stack | installer | INS-6, W16-4 |
| 6 | **WP-INS-15** maintenance on Win16 and Win32 · **WP-INS-16** ReactOS bed and real-machine kit | installer | INS-14 |
| 7 | **WP-INS-17** Quill release workflow, then the owner's tags | Quill | INS-5 tagged, INS-11 to INS-15 |
| 8 | **WP-INS-19** real Windows machine · **WP-INS-20** one-file Windows setup · **WP-INS-21** signed feed end to end | installer | INS-5 |
| 9 | **WP-INS-22** Fizz's installer project · **WP-INS-23** Fizz evidence | Fizz | INS-19 to INS-21 |

Table 11. Build order. WP-INS-18 (Studio builds media, S6, and the dry run of a medium, S9) is off the critical path and starts whenever a builder is free after WP-INS-9. WP-W16-5 (packer, in rust-win16) can run at any time and is wanted before WP-INS-13 for the 41 KB it frees. Quill's installer **proves the chain** at the end of wave 7; Fizz follows.

The Win16 columns below are the most a package may spend; a package states its measured bytes in its pull request, and the gate decides.

### WP-INS-1 — Media in the project model and in `liminst check`

- **Branch:** `feat/media-model` from `origin/main`.
- **Finish line:** section 4.1; `[[media]]` and `[[media.target]]` parse into `InstallerFile.media`; `li_media::check_project` reports every rule; `li_media::auto_serial` with the vectors of section 6; the Lt. Fizz example has a floppy and a CD medium and `liminst check` is clean; SPEC §3 and §13 and D114 in.
- **Files owned:** `crates/li-model/src/{project.rs,lib.rs}`, a new `crates/li-model/src/media.rs`, `crates/li-media/src/{types.rs,validate.rs,warning.rs,lib.rs}`, new `crates/li-media/src/{check.rs,serial.rs}`, `crates/li-media/Cargo.toml`, `crates/li-cli/src/check.rs` and its tests, `crates/li-cli/Cargo.toml`, `examples/lt-fizz/installer/installer.toml`, the regenerated `packages/protocol/src/generated/`, `SPEC.md` §3 and §13, `docs/decisions.md`.
- **Must not touch:** `li-retro`, `li-retro-build`, `li-lpk`, `li-package`, `li-engine` (the engine cannot depend on `li-media`, which will depend on `li-retro-build`), the runtimes, `apps/studio/src` beyond what regenerated bindings force.
- **Depends:** nothing.
- **Budgets:** none at run time.
- **Tests:** unit tests per rule; `liminst check` on the example; bindings drift; `make validate`.
- **Evidence:** the test names and `liminst check` output in the pull request.
- **Parallel with:** WP-INS-2, WP-W16-2, WP-INS-3.

### WP-INS-2 — An ISO 9660 and Joliet writer

- **Branch:** `feat/iso-writer`.
- **Finish line:** `li_package::iso::write(volume_id, joliet_label, files, epoch) -> Vec<u8>` and `read_iso` for tests: level 1 names (`NAME.EXT;1`, upper case, 8.3), a Joliet supplementary descriptor (UCS-2 big-endian, at most 64 characters), L and M path tables, folders, 2,048-byte sectors, fixed timestamps; deterministic bytes; refuses a name that is not 8.3 for level 1.
- **Files owned:** new `crates/li-package/src/iso.rs`, `crates/li-package/src/lib.rs` (the module line), its tests and golden image under `crates/li-package/tests/`.
- **Must not touch:** `dos.rs`, `li-media`, `li-cli`.
- **Depends:** nothing.
- **Tests:** round trip; a pinned SHA-256; `xorriso -indev X.iso -find` and `isoinfo -l -J` when installed (skipped with a note otherwise); DOSBox Staging `imgmount d X.iso -t iso` listing the tree (one manual run, logged).
- **Evidence:** the `isoinfo` listing in the pull request.
- **Parallel with:** WP-INS-1, WP-W16-2.

### WP-INS-3 — Quill: today's installer, and the Ghostwriter components (Quill repository)

- **Branch:** `feat/installer-ghostwriter` in Quill (WP-SYS-8's branch; this package replaces that brief).
- **Finish line:** WP-SYS-8's finish line, plus: `installer/liminst.rev` at the installer's `main` after WP-SYS-5 (`083b99a` or later), so `.RTF` and `.TXT` start unticked; the stale comment in `components.toml` and INS-2's "no unticked-component field yet" corrected; `build-disks.sh` sets the serial `0D05-MmmP` with `mlabel -N` (section 6); a recorded Win16 heap peak, data free and stack figure for a Quill install and uninstall on Windows 3.11 with today's runtime (the `/LOG` lines), and each per-OS disk's free bytes.
- **Files owned:** Quill's `installer/components.toml`, `installer/liminst.rev`, `installer/tools/{stage.sh,build-disks.sh,e2e-w31.sh,e2e-w95.sh}`, `docs/installer.md`, `docs/decisions.md` (INS-2 amended, INS-26 for the serial).
- **Must not touch:** anything outside `installer/` and those docs; afterglow-savers; the print set's branch.
- **Depends:** nothing (WP-SYS-4 and WP-SYS-5 are merged).
- **Budgets:** Win16 heap margin at least 4,096 on Quill's install; report it either way.
- **Tests:** `installer/tools/check.sh`; `e2e-w31.sh` and `e2e-w95.sh`, default run and a run with `.RTF` and the saver ticked; `verify-disks.sh`.
- **Evidence:** the heap peak table, the free bytes, `VOL` output from mtools (`minfo`), screenshots of the saver in Control Panel's Desktop and Display Properties.
- **Parallel with:** wave 0.

### WP-INS-4 — Compile a medium: `COMMON.LPK`, `others`, the text table (format 6)

- **Branch:** `feat/media-compile`, on WP-INS-1.
- **Finish line:** section 4.2; `li_media::compile(&Project, media_id, &RuntimeSet) -> CompiledMedium` (each target's `Compiled`, the `COMMON.LPK` bytes, the set ids); `li-retro`'s model and hand-written decoder read format 6 (D116, D117); the text table holds every message the later packages need (the set-id and missing-pack messages, the redirect page, the disk prompt), worded in Canadian English and overridable from a theme's `texts.toml` [U: or `installer.toml`; the builder proposes, the default is the theme]; DOS, Win16 and Win32 goldens unchanged for `-t` builds.
- **Files owned:** `crates/li-media/src/compile.rs` (new), `crates/li-retro-build/src/{lib.rs,payload.rs}`, `crates/li-retro/src/{model.rs,decode.rs}` and their tests, `crates/li-retro-build/tests/`, `SPEC.md` §8 and §13, `docs/retro-dos.md` (compiled project section), `docs/decisions.md` (D116, D117).
- **Must not touch:** `li-retro/src/{install.rs,pack.rs,platform.rs,app.rs}` (WP-INS-6), the runtimes, `li-lpk`'s format.
- **Depends:** WP-INS-1.
- **Budgets:** Win16 data 0 (decoder code only; the decoder must not add statics); code +1 KB at most in `li-retro`.
- **Tests:** section 4.2's list; the D72 cross-check on format 6.
- **Parallel with:** WP-W16-4 (different files: `cp437.rs` and the `trim` sites are WP-W16-4's; if both touch `app.rs`, WP-W16-4 goes first), WP-INS-5.

### WP-INS-5 — Release assets, the release key and a signed `SHA256SUMS`

- **Branch:** `ci/release-runtimes`.
- **Finish line:** section 8: the `retro-runtimes` job, the zip and manifest, `SHA256SUMS.sig` when the secret exists, `liminst sign` or the existing signing code reused for a file [U: whether `liminst` has a generic sign command; add `liminst sign <file> --key` if not], `keys/` with a placeholder note until the owner adds `release.pub`; `docs/releasing.md` corrected (feed signing works) and extended; D120, D121; a dispatch run that uploads the artefact.
- **Files owned:** `.github/workflows/release.yml`, `scripts/ci-stage.sh` or a new `scripts/ci-retro-runtimes.sh`, `crates/li-cli/src/{cli.rs,keygen.rs}` (a `sign` subcommand only if missing), `docs/releasing.md`, `docs/decisions.md`, `keys/README.md`.
- **Must not touch:** the runtimes' sources, `ci.yml`'s jobs (reuse their images).
- **Depends:** nothing; publishing needs the owner's tag.
- **Tests:** `actionlint` and `shellcheck` in Docker; a `workflow_dispatch` run on the branch (the owner's go to dispatch, per the house rule); a local run of the script.
- **Parallel with:** anything.

### WP-INS-6 — The runtimes read `COMMON.LPK`

- **Branch:** `feat/runtime-common-pack`, on WP-INS-4 and WP-W16-2.
- **Finish line:** section 4.3 (media plan M4).
- **Files owned:** `crates/li-retro/src/{pack.rs,platform.rs,install.rs,app.rs,sim.rs}`, `dos/setup/src/platform.rs`, `retro/win16/src/bin/lisetup/plat.rs`, `win32/setup/src/platform.rs`, the e2e scripts' shared-tree mode, `docs/retro-*.md`.
- **Must not touch:** `li-lpk` (use `Reader::begin_for`, D82), the packer, `heap.rs`.
- **Depends:** WP-INS-4, WP-W16-2.
- **Budgets:** Win16 data +400 at most; heap +1,000 at most at the peak; code +3 KB at most.
- **Tests:** section 4.3; `make dos`, `make win16`, `make win32`; `e2e-w31.sh --runs 3` from a shared tree.
- **Evidence:** the Win16 `/LOG` heap and stack lines for Lt. Fizz and, if it builds, Quill.

### WP-INS-7 — `liminst build --media`: floppy, zip, `SIZES.txt`, gate, README

- **Branch:** `feat/media-build`, on WP-INS-4.
- **Finish line:** section 4.4 (media plan M5, in `li-media`).
- **Files owned:** `crates/li-media/src/{build.rs,size.rs}` (new), `crates/li-media/Cargo.toml`, `crates/li-cli/src/{build.rs,cli.rs}` (the `--media` flag), `crates/li-cli/tests/`, SPEC §12 and §13, `docs/retro-dos.md` (the floppy section).
- **Must not touch:** `li-package/src/dos.rs` beyond calling it, `li-retro`.
- **Depends:** WP-INS-4.
- **Tests:** section 4.4; then, once WP-INS-6 is in, a DOSBox Staging install from the image.

### WP-INS-8 — CD output and `AUTORUN.INF`

- **Branch:** `feat/media-cdrom`, on WP-INS-2 and WP-INS-7.
- **Finish line:** section 4.5; the `cdrom` kind writes the ISO, `AUTORUN.INF` and the Joliet label `<name> <v>`; `scripts/win95/run-win95.sh` gains `-i ISO`.
- **Files owned:** `crates/li-media/src/build.rs` (the `cdrom` arm), `scripts/win95/run-win95.sh`, `docs/win95-testbed.md`.
- **Tests:** section 4.5.
- **Evidence:** the AutoPlay screenshot on the Windows 95 bed; the DOSBox Staging and Windows 3.11 installs from the ISO.

### WP-INS-9 — The Studio's real media planner

- **Branch:** `feat/media-plan`, on WP-INS-7.
- **Finish line:** section 4.6 (Studio plan S5).
- **Files owned:** `crates/li-media/src/{plan.rs,edit.rs}`, `crates/li-studio/src/media.rs` (new) and `DocKey`, `apps/studio/src-tauri/src/lib.rs` (the commands), the MCP tool registry, `apps/studio/src/lib/` (the real `Api`), `crates/li-cli/src/` (`media plan`), SPEC §13, `docs/studio.md`.
- **Must not touch:** `li-media/src/build.rs` beyond sharing `size.rs`, the Media screen's components except to drop fixture-only paths.
- **Tests:** section 4.6; `bun run typecheck && bun run test:js && bun run check:i18n`.
- **Evidence:** a screenshot of Quill's floppy and CD in the running Studio (the Tauri MCP server, or the browser demo with the real backend through `liminst studio --mcp`).

### WP-INS-10 — The wrong-OS redirect

- **Branch:** `feat/media-redirect`, on WP-INS-4 and WP-W16-4.
- **Finish line:** section 4.7.
- **Files owned:** `crates/li-retro/src/app.rs` (the redirect step), `dos/setup/src/platform.rs` and `dos/rt` (INT 2Fh), `retro/win16/tools/imports.tbl` and `src/imports.rs` (`WinExec`), `retro/win16/src/bin/lisetup/plat.rs`, the e2e scripts.
- **Budgets:** Win16 data +250 at most; code +2 KB at most.
- **Tests:** section 4.7.

### WP-INS-11 — Quill's media build (Quill repository)

- **Branch:** `build/media` in Quill, on a pinned installer with WP-INS-6 to WP-INS-8.
- **Finish line:** media plan Q1 and Q2 with D115: `installer.toml` has the three per-OS floppy media, the CD and the two-disk all-in-one (`overflow = "span"`, left as `error` and unused until WP-INS-13 is pinned); `dicts` and `dicts-more` (owner question 4); `serial_prefix = "0D05"`, `catalogue = "QD05-{code}"`, the labels of section 6; `installer/tools/build-media.sh` replaces `build-disks.sh` and makes `QUILL-DOS.IMG`, `QUILL-W31.IMG`, `QUILL-W95.IMG`, `QUILL-<v>.ISO`, the zips, `SIZES.txt` and `SHA256SUMS`, with the Fizz-style image checks (media plan section 5).
- **Files owned:** Quill's `installer/installer.toml`, `installer/components.toml`, `installer/tools/{build-media.sh,verify-disks.sh,stage.sh}`, `installer/tools/build-disks.sh` (removed), `installer/liminst.rev`, `docs/installer.md`, `docs/decisions.md`.
- **Tests:** `installer/tools/check.sh`; `build-media.sh` on real binaries; `verify-disks.sh` on every image.
- **Evidence:** `SIZES.txt` committed to the pull request body.

### WP-INS-12 — Quill end to end from the images (Quill repository)

- **Branch:** `test/install-media` in Quill, on WP-INS-11.
- **Finish line:** finish line item 3 for the per-OS floppies and the CD: Quill's `e2e-dos.sh`, `e2e-w31.sh` and `e2e-w95.sh` take `IMG=` or `ISO=`; the default and ticked runs for associations and the saver; the redirect cases once WP-INS-10 is pinned.
- **Files owned:** Quill's `installer/tools/e2e-*.sh`, `installer/tools/shots.sh`, `docs/installer.md`, `docs/media/install-*.png`.
- **Evidence:** screenshots and the logs' page lines.

### WP-INS-13 — Disk spanning

- **Branch:** `feat/runtime-spanning`, on WP-INS-6 and WP-INS-7 (and WP-W16-5's refresh if it is in).
- **Finish line:** section 4.8.
- **Files owned:** `crates/li-retro/src/pack.rs` (`open_volume`), the three platforms' prompt, `crates/li-media/src/build.rs` (the span arm and ordering), `crates/li-retro-build/src/payload.rs` (stream order), `crates/li-lpk/tests/golden/` (`common-span.lpk`, `.l02`, under the compatibility rule with `LI_UPDATE_GOLDEN=1`), the Studio's span view (S10), `docs/retro-*.md`.
- **Budgets:** Win16 data +250 at most; code +2 KB at most.
- **Tests:** section 4.8.

### WP-INS-14 — Modify, repair and update: the core and DOS

- **Branch:** `feat/retro-maintenance`, on WP-INS-6 and WP-W16-4.
- **Finish line:** section 7 in `li-retro` and the DOS runtime: finding the installed copy, the three outcomes, modify, repair (with `Platform::read_chunk`), update, the receipt rename, `/M` and `/R`, the two base layouts, format 7; host goldens for every case; DOSBox Staging install, damage a file, `SETUP /R`, modify to add the samples, update from a 0.1.1 build of the example.
- **Files owned:** `crates/li-retro/src/{app.rs,install.rs,platform.rs,sim.rs,model.rs,decode.rs}`, new `crates/li-retro/src/maint.rs`, `crates/li-retro-build/src/{screens.rs,lib.rs}`, the base layouts in `examples/*/installer/layouts/` and the shared layout folder, `dos/setup/src/platform.rs`, SPEC §9, `docs/retro-dos.md`, D118 and D119.
- **Must not touch:** `retro/win16`, `win32` (WP-INS-15), `li-lpk`.
- **Budgets:** none on Win16 yet (the Win16 platform answers "not supported" until WP-INS-15); the core must add no `static`.
- **Tests:** `cargo nextest run -p li-retro -p li-retro-build --locked`; `make dos`.

### WP-INS-15 — Modify, repair and update on Windows 3.1 and Windows 95

- **Branch:** `feat/retro-maintenance-windows`, on WP-INS-14.
- **Finish line:** the Win16 and Win32 platforms answer section 7 (`read_chunk` through `dosio` and `ReadFile`, `InstallLocation` from the registry, `NoModify` and `NoRepair`); Wine, Windows 3.11 and the Windows 95 bed runs of the WP-INS-14 cases.
- **Files owned:** `retro/win16/src/bin/lisetup/{plat.rs,ui.rs,dosio.rs}`, `win32/setup/src/{platform.rs,gui.rs}`, the e2e scripts, `docs/retro-win16.md`, `docs/retro-win32.md`.
- **Budgets:** Win16 data +900 at most; heap margin at least 4,096 on every run; code +4 KB at most; Win32 imports on the allowlist only.

### WP-INS-16 — ReactOS as a Win32 test bed, and the real-machine kit

- **Branch:** `test/reactos-bed`.
- **Finish line:** `scripts/reactos/run-reactos.sh` boots a throwaway copy (`snapshot=on`) of the Afterglow image from reactos-liminal (read only, by path from an environment variable), puts a floppy image or ISO in, drives `SETUP.EXE /PLAY` through QMP, reads the `/DUMP` file back from a second FAT disk and checks it like `wine-e2e.sh`; Lt. Fizz and Planboard pass; `docs/real-machines.md` holds section 11 as a checklist and an empty results table; D122.
- **Files owned:** `scripts/reactos/` (new), `docs/real-machines.md` (new), `docs/retro-win32.md` (the verification list), `docs/decisions.md`.
- **Tests:** two runs of each example; shellcheck in Docker.

### WP-INS-17 — Quill's release workflow (Quill repository)

- **Branch:** `ci/release` in Quill, after WP-INS-5 has a tagged installer release and WP-INS-11 to WP-INS-15 are pinned.
- **Finish line:** media plan Q6 and Q7, with `installer/liminst.version` (section 8) in place of `liminst.rev`, the signature check, `CHANGELOG.md`, the Cargo version bump, the README "Get it" paragraph naming each asset, the labels and the catalogue number; a dispatch with `v0.1.0-test` uploads the artefacts.
- **Evidence:** the dispatch run's artefact list; the owner tags.

### WP-INS-18 — The Studio builds media (off the critical path)

Studio plan S6, S7 and S9a: build jobs, results with SHA-256, reveal, the dry run of a medium. Starts after WP-INS-9 when a builder is free.

### WP-INS-19 — The engine's real machine on Windows

- **Branch:** `feat/windows-real-machine`.
- **Finish line:** `li-engine`'s real machine on Windows reads and writes the registry (`HKCU`, `HKLM` and the WOW64 views) through the `windows` crate behind `cfg(windows)`, detects elevation and asks for it through a relaunch with `runas` for a machine scope, and sets the shortcut's AppUserModelID; the `tauri-notes` example installs, updates, repairs and uninstalls on the Windows 11 VM in native and wrap modes, WebView2 present and absent, with the receipt and the registry read back.
- **Files owned:** `crates/li-engine/src/real.rs` and a new `real_windows.rs`, `crates/li-engine/Cargo.toml`, `docs/tauri-apps.md`, `docs/tauri-windows-spike.md` (results).
- **Must not touch:** the sim's behaviour (the same tests must pass on both).
- **Tests:** `cargo nextest` on the host; a Windows build in CI (`release.yml`'s Windows job, or a new `ci.yml` job); the VM runs, under the VM notes.

### WP-INS-20 — A one-file Windows setup, and the runner built for Windows in CI

- **Branch:** `feat/web-setup-exe`, on WP-INS-19.
- **Finish line:** D124: `liminst build` for a Windows `web` target writes `<slug>-<v>-setup.exe`; the runner finds its appended pack, unpacks it to a temporary folder, runs, and cleans up; `runner.yml` (or `release.yml`) builds `liminal-runner.exe` for `windows-x86_64` and the release carries it; Fizz-modern's pages run from the one file on the Windows 11 VM.
- **Files owned:** `crates/li-cli/src/web.rs`, `crates/li-render-web/`, `apps/runner/src-tauri/` (startup), `.github/workflows/runner.yml`, `docs/web-runner.md`.

### WP-INS-21 — A signed feed, end to end

- **Branch:** `test/signed-feed`, on WP-INS-19.
- **Finish line:** the `tauri-notes` example's feed is signed with a test key, served from a local folder and from an `http` URL, and an update from 1.0.0 to 1.0.1 runs on Linux and on the Windows 11 VM; a feed signed with another key, a rolled-back feed and a changed artefact are each refused; the action's `sign-key` input is shown working in `docs/releasing.md`.
- **Files owned:** `examples/tauri-notes/installer/feed/`, `crates/li-runner/tests/`, `.github/actions/installer-action/`, `docs/releasing.md`.

### WP-INS-22 — Lieutenant Fizz's installer project (Fizz repository)

- **Branch:** `feat/liminal-installer` in `liminal-hq/lieutenant-fizz`, after the owner answers questions 8 to 10.
- **Finish line:** `installer/` in the game repository, made with `liminst init` and `liminst import tauri apps/player/src-tauri/tauri.conf.json`; targets `windows-x86_64:web` (fizz-modern, copied from the installer's example with its provenance) and `dos-x86` (from the fizz-dos release's `disk.zip`, the fizz-dos theme, a `floppy-1440` medium with `serial_prefix = "FE01"`); `[updates] mode = "liminal"` with the feed on GitHub Pages and a pinned feed key; a `liminal-installer` job in `release.yml` that uses the installer action pinned to an installer release, builds `Lieutenant-Fizz_<v>_x64-liminal-setup.exe` and the DOS floppy, signs and updates the feed, and attaches them beside the existing assets.
- **Files owned:** the game's `installer/`, `.github/workflows/release.yml` (the new job only), `docs/APP.md` (Releases).
- **Must not touch:** the game, its existing release jobs and assets.

### WP-INS-23 — Lieutenant Fizz evidence

The Windows 11 VM: install, update from the feed, repair, uninstall, with screenshots of every fizz-modern page in light and dark; DOSBox Staging from the DOS floppy; then the owner's real-machine items 8 and 4 in section 11.

## 16. Sonnet briefs

### Brief: WP-INS-1 (in full)

> Sonnet builder, Liminal Installer (`~/source/liminal-hq-labs/liminal-installer`, remote `liminal-hq-labs/liminal-installer`). The main checkout belongs to the orchestrator: run `git fetch`, then make a worktree at `~/source/worktrees/liminal-installer/media-model` on a new branch `feat/media-model` from `origin/main`, and work only there. Read `AGENTS.md`, `SPEC.md` §3 and §13 (The Media screen), `docs/decisions.md` D80 to D85, `docs/plans/release-media-plan.md` section 2.1, `docs/plans/studio-media-screen-plan.md` section 5, and `docs/plans/roadmap-to-quill-and-fizz.md` sections 4.1, 6 and 12 (D114), which are your specification. Today `installer.toml` cannot hold a `[[media]]` entry: the `li-media` crate has the wire types and `validate`, but nothing depends on it and `li-model` does not parse media. You make media part of the project and of `liminst check`. No compiler, runtime or build output changes.
>
> **Do this.**
>
> 1. **Model** (`crates/li-model`). Add `src/media.rs` with the TOML shape of a medium: `Media { id, kind, label, serial, serial_prefix, catalogue, common, exclude, overflow, reserve, readme, extra, autorun, targets }` and `MediaTarget { target, dir }`, with the defaults SPEC §13 gives (`serial = "auto"`, `common = "auto"`, `overflow = "error"`, `reserve = 16384`, empty lists) and the TOML names `[[media]]` and `[[media.target]]` (so the field is `#[serde(rename = "target")] targets`). Add `#[serde(default, rename = "media")] pub media: Vec<Media>` to `InstallerFile`. Move `MediaKind`, `CommonMode`, `Overflow` and `MediaTarget` from `li-media/src/types.rs` into `li-model::media` behind the same `ts` feature and re-export them from `li-media`, so there is one definition; the generated TypeScript in `packages/protocol` must keep the same type names and shapes (regenerate with `scripts/generate-protocol-bindings.sh` and check `git diff` shows only additions for the two new fields). Add `serial_prefix: Option<String>` and `catalogue: Option<String>` to `MediaInfo` and `MediaEdit` as well, and teach `MediaEdit::apply` them. An existing project without `[[media]]` must load exactly as before.
> 2. **Serial** (`crates/li-media/src/serial.rs`, new). `pub fn version_code(version: &str) -> Result<u16, MediaRuleError>` takes `MAJOR.MINOR.PATCH` (a suffix after `-` or `+` is ignored, as Fizz's `make-release.sh` does) and gives `major << 12 | minor << 4 | patch`, refusing a major or patch over 15 or a minor over 255 with a new `MediaRuleError::VersionDoesNotFit`. `pub fn auto_serial(prefix: Option<&str>, version: &str) -> Result<u32, MediaRuleError>` is `prefix << 16 | version_code`; the prefix is four hexadecimal digits (a new `BadSerialPrefix` otherwise) and defaults to `4C49`. `pub fn catalogue_text(template: &str, version: &str)` replaces `{code}` with the four digits in upper case. Test vectors: `auto_serial(Some("0D05"), "0.1.0") == 0x0D05_0010`; `auto_serial(Some("FE01"), "1.112.3") == 0xFE01_1703` (Fizz's own example in `~/source/experiments/fizz-dos/tools/make-release.sh`, read only); `"0.1.0-test"` as `"0.1.0"`; `"16.0.0"`, `"0.256.0"`, `"0.0.16"` refused; `catalogue_text("QD05-{code}", "0.1.0") == "QD05-0010"`; `format_serial` of the first is `0D05-0010`.
> 3. **Checks** (`crates/li-media/src/check.rs`, new). `pub fn check_project(installer: &InstallerFile, components: &[Component]) -> Vec<MediaWarning>` (take what you need from `li-model`; `li-media` gains a `li-model` dependency, and must not depend on `li-engine`). For each medium, in this order: an id used twice (error); `validate`'s label, serial and folder rules (reuse them); `serial_prefix` four hexadecimal digits; `catalogue` at most 16 characters after substitution, ASCII printable; every `target` is a target of the project whose renderer is `tui-retro`, `win16` or `win32-classic` (error, new code `media_target`), and no target listed twice; `dir` unique in the medium (the existing `dir_not_8_3`); `exclude` names a component that exists (error, new code `media_exclude`); `autorun` only on a `cdrom` and naming a `win32-classic` target of the medium (`autorun_target`); a floppy's root would hold at most 224 entries counting each target's `SETUP.EXE` and `SETUP.LPK`, the folders, `COMMON.LPK`, the README and `extra` (`does_not_fit`, with a message that says root entries, not bytes); `readme` and `extra` paths exist below `installer/` (error, new code `media_file`). Add the new codes to `MediaWarningCode` and `ALL`, to SPEC Table 9 and to the Studio's message catalogue only if the Studio's i18n check requires it (it lists codes; if it does, add the `en-CA` text and let the pseudo-locales regenerate). The version for the serial is the project's literal version; when it is `{ from = "git-tag" }`, skip the serial check with no warning (the build will have a version).
> 4. **`liminst check`** (`crates/li-cli/src/check.rs`). Run `li_media::check_project` after the existing checks and print its warnings as diagnostics with the code, the medium's id as the location and the message; an error fails the check as other errors do. Add `li-media` to `li-cli`'s dependencies.
> 5. **Example.** In `examples/lt-fizz/installer/installer.toml` add a `floppy` medium (`kind = "floppy-1440"`, label `LT FIZZ`, `serial_prefix = "FE01"`, `catalogue = "FE01-{code}"`, `dos-x86` at the root, `win16-x86` in `WIN31`, `windows-x86` in `WIN95`) and a `cd` medium with the same targets and `autorun = "windows-x86"`. `liminst check -C examples/lt-fizz` must be clean, and every existing golden and Studio test must still pass (the Studio demo embeds parts of this example: run its tests).
> 6. **Docs** in the same commit: SPEC §3 (a sentence and a short `[[media]]` example under `installer.toml`) and §13 (the two new fields, the serial rule and the new codes in Table 9); D114 from the plan's Table 9 pasted into `docs/decisions.md` (D114, or the next free number at or above it if `main` has moved; a merge may renumber); `docs/plans/README.md` is not yours.
>
> **Must not touch.** `crates/li-retro`, `crates/li-retro-build`, `crates/li-lpk` and its golden files, `crates/li-package`, `crates/li-engine` (including `check.rs`), `dos/`, `retro/`, `win32/`, `apps/studio/src` beyond what regenerated bindings or the message catalogue force. If you need one of them, stop and report why.
>
> **Budgets.** No runtime change: the DOS, Win16 and Win32 `SETUP.EXE` and every golden stay byte-identical (say so, with the test that shows it).
>
> **Check before you commit.** `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo nextest run --workspace --locked --profile ci`; `bash scripts/check-bindings-drift.sh`; `bun run format:check`, `bun run typecheck && bun run test:js && bun run check:i18n`; `bash scripts/check-headers.sh`; markdownlint in Docker on `SPEC.md` and `docs/decisions.md`. Or `make validate`, which runs most of these. Report the commands and results exactly.
>
> **Git.** Stage explicit paths (never `git add -A` or `git commit -a`). One commit, `feat(media): read media from installer.toml and check them`, body in markdown with bold labels and no headings, hard-wrapped at 100, written to a file and committed with `git commit -F`, ending with `Co-Authored-By:` and your own model's name. Push the branch and open a pull request with the labels `enhancement`: a capitalized imperative title, `## Summary` bullets with bold lead-ins, `## Test plan` with `- [x]` items naming the commands and a last unchecked "Not verified:" line. Do not merge. Clean up with plain commands and full literal paths, one per command (no globs), and remove the worktree's `target/` when done if disk is short (`df -h /home` must stay above 20 GB).
>
> **Report.** The pull request URL, the tests run, the binding diff summary, anything in the plan you changed and why, and any `OWNER DECISION: question | default taken | where to swap` lines.

### Brief: WP-INS-2

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/iso-writer`, branch `feat/iso-writer` from `origin/main`. Read `AGENTS.md`, `docs/plans/release-media-plan.md` section 2.4 (ISO writer) and `docs/plans/roadmap-to-quill-and-fizz.md` WP-INS-2. Add `crates/li-package/src/iso.rs`: `pub fn write(volume_id: &str, joliet: &str, files: &[(String, Vec<u8>)], epoch: i64) -> Result<Vec<u8>, IsoError>` for `/`-separated 8.3 paths with folders, ISO 9660 level 1 (`NAME.EXT;1`, d-characters, upper case), a primary and a Joliet supplementary volume descriptor (UCS-2 big-endian names, escape sequence `%/E`), L and M path tables for both, directory records sorted as the standard says, 2,048-byte sectors, a terminator, every timestamp from `epoch` in UTC, no Rock Ridge and no El Torito; refuse a name that is not 8.3, a volume id that is not d-characters or longer than 32, and a Joliet label longer than 16. Add `read_iso` for tests (walk the primary or the Joliet tree and return paths and bytes). Tests: round trip of a three-folder tree, a pinned SHA-256 of a small image, the refusals, and, when `xorriso` and `isoinfo` are installed, `xorriso -indev X.iso -find` and `isoinfo -l -J -i X.iso` listing the tree (skip with a printed note otherwise). One manual DOSBox Staging run (`imgmount d X.iso -t iso`, `dir d:\` of a folder) on a private Xvfb with its own PID, logged in the PR. Do not touch `dos.rs`, `li-media` or `li-cli`. Check `cargo fmt`, clippy with `-D warnings`, `cargo nextest run -p li-package --locked`, headers. Commit `feat(package): write ISO 9660 images with Joliet names`; Git, PR (label `enhancement`), cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-3 (Quill)

> Sonnet builder, Quill (`~/source/experiments/quill16`, its own `AGENTS.md`). Worktree `~/source/worktrees/quill16/installer-ghostwriter`, branch `feat/installer-ghostwriter` from `origin/main`. Do WP-SYS-8 as `~/source/liminal-hq-labs/liminal-installer/docs/plans/system-folders.md` section 6 and its brief say, plus WP-INS-3 of `docs/plans/roadmap-to-quill-and-fizz.md` in that repository (read both; read nothing else there except to understand). Set `installer/liminst.rev` to the installer's current `origin/main` commit (it must contain D89, D90 and D95 to D100); correct the `components.toml` comment and INS-2 that say the installer has no unticked-component field; in `build-disks.sh` set each disk's FAT serial after `mlabel` with `mlabel -N <8 hex digits>` computed from the version as `0D05` then major (one hex digit), minor (two) and patch (one), failing when the version does not fit, and add INS-26 for it (the printed catalogue number stays INS-25's `QD05-0010`). Run `installer/tools/check.sh` on stand-ins; then the real build and `e2e-w31.sh` and `e2e-w95.sh` with the defaults and with `.RTF` and the saver ticked, on a private Xvfb (`DISPLAY=:NN`, your own PID, `DOSBOX_IMAGE=quill16-dosbox-x:local`); record from the Windows 3.11 `/LOG` the heap peak and margin, and report each disk's free bytes and `minfo`'s serial. Commit `feat(installer): today's installer, the Ghostwriter screen saver and the disk serial`; Git, PR in Quill's house format, cleanup and report as in WP-INS-1's brief, with the heap table and free bytes in the report.

### Brief: WP-INS-4

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/media-compile`, branch `feat/media-compile` from `origin/main` after WP-INS-1 merged. Specification: `docs/plans/release-media-plan.md` sections 2.2 (dedup, chunk size, how each runtime finds it) and 2.5 (`others`), and `docs/plans/roadmap-to-quill-and-fizz.md` sections 4.2 and 12 (D116, D117) and WP-INS-4. Add `li_media::compile` over the existing per-target compile; write `COMMON.LPK` with `li-lpk`'s writer at the smallest consumer's chunk size; bump the compiled format to 6 with `CommonRef`, `FileEntry.pack`, `others` and the `texts` table (ids for `common-missing`, `common-wrong`, `redirect-dos-under-windows`, `redirect-win16-under-95`, `insert-disk`, `wrong-disk`, each with its `en-CA` text and `{path}`, `{label}`, `{disk}` tokens), read by `li-retro`'s hand-written decoder (no serde in the runtime, no new `static`), cross-checked against serde as D72 does. `-t` builds and every DOS, Win16 and Win32 golden must stay byte-identical; a project built for format 6 must be refused by a format 5 decoder with the existing error. Tests as section 4.2. Do not touch `install.rs`, `pack.rs`, `platform.rs`, `app.rs` or any runtime. Report the `li-retro` code delta. Commit `feat(media): compile a medium into its targets and a shared pack`; Git, PR, cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-5

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/release-runtimes`, branch `ci/release-runtimes` from `origin/main`. Read `docs/releasing.md`, `.github/workflows/release.yml` and `ci.yml` (the DOS, Win16 and Win32 jobs and their images), D10, D13 and section 8 of `docs/plans/roadmap-to-quill-and-fizz.md`. Add a `retro-runtimes` job to `release.yml` that builds the three `SETUP.EXE` files with the same commands and images CI uses, writes `liminal-retro-runtimes-<v>.zip` with a `MANIFEST.txt` (each file's SHA-256, size, the commit, the toolchain), and uploads it for the publish job; make the publish job sign `SHA256SUMS` into `SHA256SUMS.sig` with `liminst` when `RELEASE_SIGN_KEY` is set (add `liminst sign <file> --key <path>` writing `<file>.sig` if no such command exists, with a test that `liminst`'s own verify reads it), never printing the key and removing it in an `always()` step. Add `keys/README.md` saying the owner adds `release.pub`. Correct `docs/releasing.md` (feed signing is implemented) and document the assets, the key and how a product verifies; paste D120 and D121. Do not dispatch the workflow or tag anything: ask the owner in your report. Check `actionlint` and `shellcheck` in Docker, `cargo nextest run -p li-cli --locked`, headers, markdownlint. Commit `ci(release): publish the retro runtimes with a signed checksum list`; Git, PR (labels `ci`, `enhancement`), cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-6

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/runtime-common-pack`, branch `feat/runtime-common-pack` from `origin/main` after WP-INS-4 and WP-W16-2 merged. Specification: `docs/plans/release-media-plan.md` section 2.2 (Platform and `Pack` changes, when it is opened, receipt and uninstall) and media plan PR M4, and `docs/plans/roadmap-to-quill-and-fizz.md` section 4.3 and WP-INS-6. Use `Reader::begin_for` with each runtime's target bit (D82). The messages come from the project's `texts` (D116); add no `static` and no literal message to the runtimes. Tests: the sim cases of section 4.3; `make dos`, `make win16`, `make win32`; DOSBox Staging, `e2e-wine.sh` (twice for the known flake) and `e2e-w31.sh --runs 3` from a shared Lt. Fizz tree; if you can build Quill's media against your runtimes, one Quill run on 3.11. Budgets: Win16 data +400, heap +1,000 at the peak, code +3 KB at most, each stated with the packer's and `/LOG`'s lines; the WP-W16-2 gates must pass. Xvfb rules, Git, PR, cleanup and report as in WP-INS-1's and the headroom plan's WP-W16-1 brief; commit `feat(retro): read files shared by several targets from COMMON.LPK`.

### Brief: WP-INS-7

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/media-build`, branch `feat/media-build` from `origin/main` after WP-INS-4 merged. Specification: media plan sections 2.4 (outputs, floppy writer, size gate, README) and PR M5, the Studio plan section 5 (the build lives in `li-media`), and `docs/plans/roadmap-to-quill-and-fizz.md` sections 4.4 and 6. Add `liminst build --media <id>` (an alternative to `-t`) over `li_media::build`, writing `build/media-<id>/`, the image with the D114 serial and label, the zip, `SIZES.txt` and the generated `README.TXT` with the catalogue line; a non-zero exit on `does_not_fit` and `below_reserve` with the largest files named; `extra` copied to the root. Put the sizing in `li-media/src/size.rs` so WP-INS-9's planner reuses it. Tests as section 4.4, with stand-in runtimes. Do not touch `li-package/src/dos.rs` beyond calling it, or `li-retro`. Commit `feat(media): build a floppy and a zip from a medium`; Git, PR, cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-8

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/media-cdrom`, branch `feat/media-cdrom` from `origin/main` after WP-INS-2 and WP-INS-7 merged. Add the `cdrom` kind to `li_media::build` over `li_package::iso` (volume id from the label, Joliet label `<product name> <version>` cut to 16), `AUTORUN.INF` for the `autorun` target (media plan section 2.4) and `<slug>-<v>-cd.iso`; add `-i ISO` to `scripts/win95/run-win95.sh` (`imgmount d ISO -t iso -ide 2m` before `boot -l c`) and document it in `docs/win95-testbed.md`. Evidence: DOSBox Staging and Windows 3.11 (DOSBox-X's MSCDEX) installs of Lt. Fizz from the ISO, and an AutoPlay screenshot on the Windows 95 bed (if Windows 95 does not see the drive, say so and copy the tree to `D:` instead, as the media plan's section 5 note says). Xvfb rules, Git, PR, cleanup and report as in WP-INS-1's brief; commit `feat(media): build a CD image with an autorun file`.

### Brief: WP-INS-9

> Sonnet builder, Liminal Installer, Studio. Worktree `~/source/worktrees/liminal-installer/media-plan`, branch `feat/media-plan` from `origin/main` after WP-INS-7 merged. Do Studio plan PR S5 (`docs/plans/studio-media-screen-plan.md` sections 5 and 8) as `docs/plans/roadmap-to-quill-and-fizz.md` section 4.6 and WP-INS-9 narrow it: `li_media::plan`, `edit` (format-keeping `installer.toml` edit, as `li-studio/src/tauri.rs` does), `list`, a `SizeCache`, `DocKey::Media` replacing the demo's local key, the Tauri commands and MCP tools, `liminst media plan <id> [--json]`. The exactness test compares the plan with `build --media` on the same input. The Media screen's components change only where they read fixture-only data. Run the Studio's checks (`bun run typecheck && bun run test:js && bun run check:i18n`, `bun run format:check`), the Tauri shell's `cargo check` and clippy, and drive the running Studio with the Tauri MCP server for a screenshot of the Lt. Fizz media (if the server does not connect, say so and use `liminst studio --mcp` with the browser demo). Commit `feat(studio): plan media from the real project`; Git, PR (labels `enhancement`, `studio`), cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-10

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/media-redirect`, branch `feat/media-redirect` from `origin/main` after WP-INS-4 and WP-W16-4 merged. Do media plan section 2.5 and PR M6 as `docs/plans/roadmap-to-quill-and-fizz.md` section 4.7 narrows it, with every word from the project's `texts`. DOS: INT 2Fh AX=1600h then AX=4680h through `dos/rt`'s DPMI real-mode interrupt call (prove it on DOSBox Staging and in a Windows 3.11 DOS box). Win16: `GetVersion` and `WinExec` (read the ordinal from Windows 3.11's `KRNL386.EXE` with `retro/win16/tools/ne_dump.py`, add it to `imports.tbl`, regenerate `src/imports.rs`). Tests as section 4.7 plus the host goldens. Budgets: Win16 data +250, code +2 KB at most, stated. Xvfb rules, Git, PR, cleanup and report as in WP-INS-1's brief; commit `feat(retro): point a person at the setup for their system`.

### Brief: WP-INS-11 (Quill)

> Sonnet builder, Quill. Worktree `~/source/worktrees/quill16/build-media`, branch `build/media` from `origin/main`, after the installer has WP-INS-6 to WP-INS-8 merged. Specification: the installer's `docs/plans/release-media-plan.md` sections 4 and 5 (Q1, Q2) and `docs/plans/roadmap-to-quill-and-fizz.md` sections 3, 6 and WP-INS-11 (D115). Pin the installer; add the five media entries; split `dicts` and `dicts-more`; write `installer/tools/build-media.sh` and remove `build-disks.sh`; keep `verify-disks.sh` working on every image. Leave the two-disk entry at `overflow = "error"` with a comment until spanning is pinned, and expect its build to fail the size gate today with a dictionary: exclude `dicts` and `dicts-more` from it until then, and say so. Report `SIZES.txt` for every medium. Commit `build(installer): build Quill's floppies and CD from media entries`; Git, PR, cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-12 (Quill)

> Sonnet builder, Quill. Worktree `~/source/worktrees/quill16/install-media`, branch `test/install-media` after WP-INS-11 merged. Make `installer/tools/e2e-dos.sh`, `e2e-w31.sh` and `e2e-w95.sh` take `IMG=` or `ISO=` and install from the final images and the CD as the installer's media plan section 5 table says, with the default and ticked association and saver runs, and the redirect cases once WP-INS-10 is pinned. Private Xvfb with your own PID, `DOSBOX_IMAGE=quill16-dosbox-x:local`, never `pkill` by name. Screenshots per INS-4. Commit `test(installer): install Quill from its release images`; Git, PR, cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-13

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/runtime-spanning`, branch `feat/runtime-spanning` after WP-INS-6 and WP-INS-7 merged. Do media plan section 2.6 and PR M8 and Studio plan S10, as `docs/plans/roadmap-to-quill-and-fizz.md` section 4.8 narrows them, with the prompts' words from the project's `texts`. The `common-span` golden follows `docs/lpk-format.md`'s compatibility rule (`LI_UPDATE_GOLDEN=1` only for the new files). Tests as section 4.8. Budgets: Win16 data +250, code +2 KB at most. Xvfb rules, Git, PR, cleanup and report as in WP-INS-1's brief; commit `feat(retro): ask for the next disk of a set`.

### Brief: WP-INS-14

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/retro-maintenance`, branch `feat/retro-maintenance` after WP-INS-6 and WP-W16-4 merged. Specification: `docs/plans/roadmap-to-quill-and-fizz.md` section 7, D118, D119 and WP-INS-14. Build the core in a new `crates/li-retro/src/maint.rs` and the DOS platform's part; the Win16 and Win32 platforms answer "not supported" with the existing error path until WP-INS-15. Compiled format 7 adds the two pages; the base layouts go where the shared layouts live, with a theme override for Lt. Fizz's `fizz-dos` look. Host goldens for every case in section 4.9; DOSBox Staging runs of repair, modify and update. Add no `static` to the core. Commit `feat(retro): modify, repair and update from the setup disk`; Git, PR, cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-15

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/retro-maintenance-windows`, branch `feat/retro-maintenance-windows` after WP-INS-14 merged. Implement the Win16 and Win32 platform parts of section 7 (`read_chunk`, finding the installed copy, `NoModify` and `NoRepair`), and run the WP-INS-14 cases under Wine (twice), on Windows 3.11 (`e2e-w31.sh --runs 3`) and on the Windows 95 bed. Budgets: Win16 data +900, code +4 KB at most, heap margin at least 4,096 on every run, all stated; Win32 imports only from the allowlist, each confirmed in Windows 95 OSR2's exports. Xvfb rules, Git, PR, cleanup and report as in WP-INS-1's brief; commit `feat(win16,win32): modify, repair and update on Windows`.

### Brief: WP-INS-16

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/reactos-bed`, branch `test/reactos-bed`. Read `docs/win95-testbed.md`, `win32/tools/wine-e2e.sh`, and reactos-liminal's and afterglow-savers' guest drivers (read only; `scripts/guest/guest.py` in afterglow-savers is the pattern). Write `scripts/reactos/run-reactos.sh` as WP-INS-16 says, with the image path from `REACTOS_IMAGE`, QEMU with `snapshot=on`, KVM when available, `-display none`, QMP on a private socket and your own PID; never write the source image. Run Lt. Fizz and Planboard twice each. Write `docs/real-machines.md` from section 11 and paste D122. Commit `test(win32): install and remove on ReactOS`; Git, PR, cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-17 (Quill)

> Sonnet builder, Quill. After the installer's first tagged release and WP-INS-11 to WP-INS-15 pinned. Do the media plan's Q6 and Q7 with `installer/liminst.version` and the signature check of `docs/plans/roadmap-to-quill-and-fizz.md` section 8; a dispatch run with `v0.1.0-test` is the owner's go. Commit `ci(release): release Quill's disks and CD`; Git, PR, cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-19

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/windows-real-machine`, branch `feat/windows-real-machine`. Read `docs/tauri-apps.md` (build order item 3), `docs/tauri-windows-spike.md`, `crates/li-engine/src/{machine.rs,real.rs,sim.rs}` and the Windows VM notes in the owner's memory (snapshot first, never build Rust in the guest, restore after). Implement the registry, elevation and AppUserModelID behind `cfg(windows)` with the `windows` crate; build on a Windows CI runner (add the job) or cross-compile on the host, then copy the binaries in; run the `tauri-notes` example's install, update, repair and uninstall in native and wrap modes, WebView2 present and absent, and read the registry back. Commit `feat(engine): use the real registry on Windows`; Git, PR, cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-20

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/web-setup-exe`, branch `feat/web-setup-exe` after WP-INS-19 merged. Do D124 as WP-INS-20 says: the appended stored `.LPK` and trailer, the runner reading its own file, the temporary folder and its cleanup, the CI build of `liminal-runner.exe` for Windows; prove it with Lt. Fizz's `windows-x86_64:web` target on the Windows 11 VM with screenshots of each fizz-modern page. Commit `feat(web): one setup file for a Windows web target`; Git, PR, cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-21

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/signed-feed`, branch `test/signed-feed` after WP-INS-19 merged. Do WP-INS-21: a test feed key (never committed beyond the test fixture's own throwaway key, named as such), the update runs on Linux and the Windows 11 VM, the three refusals, and the action's `sign-key` documented. Commit `test(feeds): update from a signed feed and refuse a forged one`; Git, PR, cleanup and report as in WP-INS-1's brief.

### Brief: WP-INS-22 (Lieutenant Fizz)

> Not to be started before the owner answers questions 8 to 10. Sonnet builder, Lieutenant Fizz (`~/source/liminal-hq/lieutenant-fizz`, its own `AGENTS.md`). Worktree `~/source/worktrees/lieutenant-fizz/liminal-installer`, branch `feat/liminal-installer`. Do WP-INS-22 of the installer's `docs/plans/roadmap-to-quill-and-fizz.md`: `installer/` from `liminst init` and `liminst import tauri`, the two targets, the feed, and one new job in `release.yml` that leaves the existing jobs and assets alone. A dispatch run is the owner's go. Commit `feat(release): build a themed Windows setup and a DOS floppy with the Liminal Installer`; Git, PR in the game's house format, cleanup and report as in WP-INS-1's brief.
