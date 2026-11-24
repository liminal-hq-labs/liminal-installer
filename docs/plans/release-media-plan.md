# Plan: file associations, shared payload, an all-in-one floppy and an all-in-one CD for Quill

The all-in-one floppy fits, but only with the Canadian dictionary on it: about 171 KB is left free. With all three dictionaries it is about 248 KB over, even when the shared files are stored once. Without sharing it does not fit even with one dictionary. So the shared payload is the feature that makes the disk possible.

The design below makes it a general installer feature: a project defines a set of targets that share one disk or CD, and the files they have in common are stored once. The CD output uses the same mechanism and has room for all three dictionaries.

**Markers.** [V] means I checked it in the code or files at the path given. [U] means I did not, and the implementer must check it. Packed sizes come from Python zlib (raw deflate, level 9) in independent chunks, plus the 12-byte chunk headers and the pack's header, footer and directory. That matches li-lpk's packer (`writer.rs`: `compress_to_vec(raw, 9)`, falling back to stored) but miniz_oxide is not zlib, so expect a few per cent either way. I did not build `li-cli` or the runtimes: the margins are large enough that this does not change any fit/no-fit answer. Nothing was written to either repo.

## 0. What I found (beyond the earlier plan)

1. **Operations ignore the components the user ticked [V].**
   - `payload::build` pushes shortcut ops for every component that applies to the target (`li-retro-build/src/payload.rs` around line 150).
   - `do_install` then runs every op in `project.ops` without checking the selection (`li-retro/src/install.rs` around line 400).
   - `Op` has no component field (`model.rs:293`).
   - So an unticked "Quill Help" still gets its shortcut. A file type in an unticked component would still be registered. This must be fixed before the `.RTF`/`.TXT` options mean anything.
2. **Every component starts ticked, in four places [V]:**
   - `li-retro/src/app.rs:219`: `components.selected` is set to every component.
   - `li-retro-build/src/screens.rs:57`: `"checked": true` is built into the screens.
   - `li-engine/src/plan.rs:49`: no selection means all.
   - `li-studio/src/engine.rs:277` and `apps/studio/src/lib/demoApi.ts:285`.
3. **Each runtime finds its pack beside its own program [V].**
   - DOS: `dos/setup/src/platform.rs:41` (folder of the running program, else the current folder).
   - Win16: `lisetup/plat.rs:36` (`GetModuleFileName`).
   - Win32: `win32/setup/src/platform.rs:96` (`exe_dir()`, or `/LPK file`).
   - All three open it read-only: DOS function 3D00h, `_lopen(path, 0)`, and `CreateFileA` with `GENERIC_READ` and `FILE_SHARE_READ`. So a CD works, and `WIN31\SETUP.EXE` beside its own `SETUP.LPK` works today.
4. **The pack is looked up by name and the program reads one volume [V].**
   - `Pack::begin` looks entries up by name (`li-retro/src/pack.rs`).
   - `PlatformSource::open_volume` refuses any volume other than 1.
   - `Platform` has a single `data_len`/`data_read`.
5. **Uninstall never reads the payload [V].**
   - `app.rs`: Win32 uninstall decodes the project kept in `UNINST.DAT`. DOS and Win16 read only `SETUP.PRJ` from their own `SETUP.LPK`.
   - The receipt lists each file as `F dest size crc`.
   - So a shared pack is not needed to uninstall, as long as the project stays in each target's own pack.
6. **The floppy writer puts files in the root only [V].** `li-package/src/dos.rs` ("Files go in the root directory under 8.3 names"). Subdirectories are needed for `WIN31\` and `WIN95\`. There is no ISO support anywhere in the repo [V, grep]. `xorriso`, `genisoimage` and `mtools` are installed locally [V].
7. **The Win16 pack must use 4 KiB chunks [V].** `retro-win16.md:111`: 16 KiB chunks ran out of memory on 3.11, and 8 KiB left 3 KB. So any pack that the Windows 3.x installer reads must use 4 KiB chunks.
8. **Imports.**
   - Win16 already imports `GetVersion` (KERNEL 3) and `MessageBox` (USER 1) [V].
   - `WinExec` is not imported. It is KERNEL 166 from memory [U].
   - Win32 already imports `CreateProcessA` [V].
   - The DOS runtime has no INT 2Fh Windows check [V], but `dos/rt/src/dpmi.rs` exists for interrupt calls.
9. **Quill.**
   - `[workspace.package] version = "0.0.0"` [V], so the first release needs a bump.
   - There is no `CHANGELOG.md` [V].
   - CI is `ci.yml` only [V].
   - The roadmap still says `INSTALL.EXE`, "one platform is never split across two disks" and "all-in-one only if everything fits" (`docs/roadmap.md:112`) [V].
   - The last decisions are D74 in the installer repo and D90 in Quill [V].
10. **Fizz release practice [V]** (`/home/scott/source/experiments/fizz-dos/.github/workflows/release.yml`, `tools/make-release.sh`, `gh release view v0.5.0`):
    - It runs on `v*` tags and on manual dispatch with a `version` input. A manual run only uploads the files as a workflow artefact.
    - A separate `publish` job with `contents: write` runs:
      - `gh release create "$VERSION" --verify-tag --title … --notes-file NOTES.md --generate-notes`
      - with `--prerelease` for any `0.*` version, and never a draft (v0.5.0 is a pre-release, not a draft).
    - The notes are the changelog section, cut out by `awk '$0 ~ "^## " n " "'`, followed by the merged pull requests grouped by `.github/release.yml` labels.
    - The image is built with `mkfs.fat -C -F 12 -n LABEL -i SERIAL`, then `mcopy`. Times come from `SOURCE_DATE_EPOCH` with `TZ=UTC`.
    - The serial number encodes the version. The script checks the image is 1,474,560 bytes, compares every file back with `mtype | cmp`, enforces a free-space reserve and writes `SHA256SUMS`.
    - The README's "From a release" paragraph names each asset and the volume label.

## 1. File associations: `.QLD` by default, `.RTF` and `.TXT` as unticked options

### Model (`li-model`, `components.toml`)

- `Component.default: bool`, read with `#[serde(default = "yes")]`, so it is `true` when absent and existing projects do not change.
- `check` reports an error when a component is `required = true` and `default = false`.
- `FileTypeRule` stays as it is (`extension`, `name`, `open_with`). File types are already per component, so "per-component file-type rules" means only that each association gets its own component.
- `check` adds two rules:
  - an error when a Win16 target's extension is longer than 3 characters;
  - a warning (already present) when file types reach a DOS target.

Quill's `components.toml` gains:

| id | targets | default | file_types |
|---|---|---|---|
| `assoc-qld` "Open .QLD documents with Quill" | win16-x86, windows-x86 | true | `.qld` "Quill document" |
| `assoc-rtf` "Make Quill the program for .RTF files" | same | **false** | `.rtf` "Rich Text document" |
| `assoc-txt` "Make Quill the program for .TXT files" | same | **false** | `.txt` "Text document" |

These components have no files. `check` must accept a component with no files. Today it only warns on an empty project [V].

### Compiler (`li-retro-build`)

- `model::Component` gains a `default` flag.
- `screens.rs` writes `"checked": c.default`.
- Every `Op` that comes from a component gets `component: u16`. That covers `Shortcut`, `FileType` and later `Group`; `Mkdir` and `WriteIni` that are not tied to a component use `u16::MAX`.
- `Op::FileType { component, ext, prog_id, name, target }`, where `prog_id` is `<product id>.<ext>`, for example `hq.liminal.quill.rtf`.
- Fix the decoder: replace `_ => Op::Shortcut` with explicit tags, and refuse an unknown tag (the earlier L4 item).
- Bump `FORMAT`. The check is strict equality (`model.rs:356`) and the program and pack always come from the same pinned commit, so each bump is cheap.

### Runtime (`li-retro`)

- **Seeding:** `app.rs` selects `required || default`.
- **Gating:** `do_install` skips an op whose component is not selected.
- **`Shell` trait:**
  - `register_file_type(ext, prog_id, name, icon, command) -> Result<Option<String>>` returns the previous owner.
  - `unregister_file_type(ext, prog_id, previous)`.
- **Receipt:** a new record `A ext prog_id previous`. The previous value is everything after the second space, with `%` escapes for CR, LF and `%`. Older readers ignore unknown letters [V, SPEC §9].
- **Uninstall:** restore `previous` only if the extension still names our ProgID (Win32) or our command line (Win16). If the user has changed it since, leave it. Delete our ProgID key in either case.
- **Rollback:** a cancelled install restores the values in the journal in reverse order.

**Win32 (L5):**

- Install:
  - read `HKCR\.rtf` (default), for example `rtffile`;
  - set it to `hq.liminal.quill.rtf`;
  - create `hq.liminal.quill.rtf` with the name, `DefaultIcon` = `QUILL32.EXE,0` and `shell\open\command` = `"…\QUILL32.EXE" "%1"`;
  - call `SHChangeNotify(SHCNE_ASSOCCHANGED)`.
- Leave `Content Type` and the old ProgID untouched.
- Uninstall puts back `rtffile`.

**Win16 (L6):**

- Install:
  - `GetProfileString("Extensions","rtf")`, for example `C:\WINDOWS\WRITE.EXE ^.rtf`;
  - `WriteProfileString` with `C:\QUILL\QUILL.EXE ^.rtf`;
  - `SendMessage(HWND_BROADCAST, WM_WININICHANGE, 0, "Extensions")`.
- Uninstall writes the old string back, or `NULL` to delete the key if there was none.
- Never do this through `WriteIni` (earlier plan, [V]).

### Tests

- **Host, `li-retro-build/tests/{win16,win32}.rs`, using `examples/planboard`** with an unticked `.planboard` and `.txt` component:
  - Golden screens show that item unchecked.
  - A default run registers only the default type.
  - Ticking the option registers it, and the `A` record has the previous value (the sim is seeded with `txtfile`).
  - Uninstall restores it.
  - When "the user changed it since", uninstall leaves it.
  - Rollback after cancelling restores it.
  - Unticked help means no shortcut: a regression test for finding 0.1.
  - The decoder refuses an unknown tag.
- `li-render-web` and `li-studio` tests: `default = false` starts unchecked.
- **Wine:** `wine reg query 'HKCR\.txt'` before, after install and after uninstall.
- **3.11:** read `WIN.INI` back from the host after each run.
- **Quill Q5/Q4:** one run with the defaults (only `.qld`) and one with `.RTF` ticked through `/PLAY`. Assert that `QLD.REG` and `RTF.REG` show the change and that uninstall restores the previous value.

### Sequencing against L4, L5 and L6

L9 comes first. It adds `default`, the per-component ops, the explicit-tag decoder and a format bump. L4 (file-type model) is built on L9 and carries `component` from the start. L5 (Win32) is built on L4. L6 (Win16) is built on L4 and on L3 from the first wave (DDE and the profile imports).

If the first wave's L2 is still open when L9 starts, build L9 on L2: both touch `model.rs` and `decode.rs`.

## 2. Shared payload across targets (the main design: a general installer feature)

### 2.1 The concept: a `[[media]]` in `installer.toml`

A media entry is one disk or disc that holds several targets. Each target keeps its own `SETUP.EXE` and its own `SETUP.LPK`, which holds the compiled project plus the files only that target uses. Files that two or more of the media's targets install byte for byte are stored once in `COMMON.LPK`.

```toml
[[media]]
id = "floppy"
kind = "floppy-1440"            # or "cdrom"
label = "QUILL"                 # FAT: at most 11 characters; ISO: d-characters; Joliet: at most 16
serial = "auto"                 # floppy: derived from the version, as Fizz does
common = "auto"                 # "auto" | "none"
exclude = ["dicts-more"]        # components left off this media (they are not offered)
overflow = "error"              # "error" | "span" (needs runtime spanning, M8)
readme = "media/README.TXT"     # optional: generated when absent; {version} and {label} are filled in
extra = ["media/LICENCE.TXT"]   # loose files copied to the root
autorun = "windows-x86"         # cdrom only: writes AUTORUN.INF
[[media.target]]
target = "dos-x86"
dir = ""                        # the root: DOS users type A:\SETUP
[[media.target]]
target = "win16-x86"
dir = "WIN31"
[[media.target]]
target = "windows-x86"
dir = "WIN95"

[[media]]
id = "cd"
kind = "cdrom"
label = "QUILL"
autorun = "windows-x86"
# same targets; no exclude: all the dictionaries
```

Both media use the same project, components and target compiles. The only differences are:

- which components are excluded;
- each media's own `COMMON.LPK`. It has a different volume split and a different set id, which each target's compiled project records.

### 2.2 How the shared pack works

**Deduplication.**

- `common = "auto"` compiles every target of the media, then groups files by size, CRC and equal bytes.
- A group used by two or more targets goes into `COMMON.LPK` under an opaque 8.3 name: `C0000001`, `C0000002` and so on.
- Each target's `FileEntry` keeps its own `dest` (8.3 on DOS and Win16, a long name on Win32) and gains `pack: u8` (0 for its own pack, 1 for the common pack) and `stored = "C0000001"`.
- Because the names are content-addressed, it works even when the Win32 destination is a long name and the DOS one is 8.3.

**Chunk size.**

- `COMMON.LPK` uses the smallest window among its consumers, so 4 KiB (`WIN16_CHUNK_LOG2`) when the Windows 3.x installer shares it.
- Its directory entries carry the union of the consumers' target bits, for example `DOS|WIN16|WIN32 = 7`. That mask is already in the format; readers ignore it [V, `format.rs:321`].

**How each runtime finds it.**

- The compiled project gains `common: Option<CommonRef { path, set_id, len }>`.
- `path` is relative to the folder of `SETUP.EXE`: `COMMON.LPK` for the root target, `..\COMMON.LPK` for `WIN31\` and `WIN95\`.
- The compiler refuses a drive letter, a leading `\`, or more than one `..`.
- Each runtime resolves `path` with the same "folder of the running program" code it already uses for `SETUP.LPK` (finding 0.3). This adds no path-search heuristics.

**Platform and `Pack` changes.**

- `data_len(which: PackId)` and `data_read(which, offset, buf)`, plus `open_pack(which, path) -> Result<()>`. Implemented for:
  - DOS: a second handle;
  - Win16: a second `_lopen`;
  - Win32: a second `CreateFileA`;
  - the sim: a second byte vector.
- `li-retro::pack::Packs { own, common: Option<Pack> }` shares one decoded-chunk window, sized to the larger chunk size. On Win16 both are 4 KiB, so the heap gains only the common directory, a few hundred bytes, and one `Reader` struct.
- Check the heap peak on 3.11 against its 33.7 KiB [V].

**When it is opened.**

- Lazily, at the start of `commit`, so `/V`, uninstall and preview never need it.
- The `ready` page's `check-disk-space` uses the sizes in the project.
- If the file is missing, or its set id differs from `set_id`, show: "COMMON.LPK was not found next to SETUP (A:\COMMON.LPK), or it belongs to another version of Quill." Then fail before anything is written.

**Receipt and uninstall.**

- Unchanged: every installed file is an `F` line whichever pack it came from.
- Uninstall is driven by the receipt and the kept or own project (finding 0.5), so it works without `COMMON.LPK` present.
- The Win32 kept uninstaller copies the project from the target's own pack [V, `install.rs` around line 465]. That is not affected.
- Add a host test: install, delete `COMMON.LPK`, then uninstall. It passes and the folder is gone.

**Repair and update (L8, later)** will need `COMMON.LPK`. Record that in L8.

### 2.3 Pack format implications (`docs/lpk-format.md`)

- **No format change.** `COMMON.LPK` is an ordinary version 1.0 pack. Header, directory, chunks and target mask are all unchanged. The writer keeps writing the lowest minor version, so the compatibility rule is untouched.
- Add a "Shared packs" section. It is a convention, not format:
  - the `COMMON.LPK` name and the `Cnnnnnnn` entry names;
  - the target mask is the union of the consumers;
  - the chunk size is the smallest consumer's window;
  - each consumer's project records the set id, which is how a mismatched disk is caught.
- Add goldens to `crates/li-lpk/tests/golden/` (today: `basic.lpk`, `span.lpk`, `span.l02`, `span.l03` [V]):
  - `common.lpk`: 4 KiB chunks, target mask 7, three entries;
  - a two-volume `common-span.lpk` plus `.l02`, for M8.
- **Optional:** a reader check that the entry's mask includes the runtime's own bit, reported as `Corrupt`. Advisory, at most a few dozen bytes per runtime; do it in M2.

### 2.4 CLI and packaging

- `liminst build -C . --media floppy --version V --dos-runtime A --win16-runtime B --win32-runtime C -o out/`. The `--media` flag is the alternative to `-t`.
- **Outputs:**
  - `out/media-floppy/` (the loose tree);
  - `<slug>-<v>-floppy.img` for `floppy-1440`;
  - `<slug>-<v>-cd.iso` for `cdrom`;
  - `<slug>-<v>-<media>.zip` of the tree;
  - `SIZES.txt`, listing each pack's entries raw and packed, free bytes and the margin.
- **Floppy writer (`li-package/src/dos.rs`):**
  - takes `/` paths;
  - writes subdirectory clusters with `.` and `..`;
  - takes a label and a serial;
  - keeps the deterministic date;
  - `read_floppy` recurses.
- **Size gate:**
  - `DoesNotFit` gives the overrun by file;
  - a new media key `reserve`, default 16 KiB, fails the build when less is free;
  - the exit code is non-zero. Today a target's floppy output that does not fit reports "not built"; change that to an error for `--media` and decide for `-t` (earlier plan, [U]).
- **ISO writer (`li-package/src/iso.rs`)**, pure Rust, so `liminst build` needs no tool on any host:
  - ISO 9660 level 1 (`NAME.EXT;1`, upper case), plus a Joliet supplementary descriptor;
  - L and M path tables, 2048-byte sectors, fixed timestamps from `SOURCE_DATE_EPOCH`;
  - no Rock Ridge, no El Torito.
  - The tests cross-check with `xorriso -indev X.iso -find` and `isoinfo -l -J`, when present.
- **`AUTORUN.INF`:** `[autorun]` / `open=WIN95\SETUP.EXE` / `icon=WIN95\SETUP.EXE,0`, with CRLF line ends.
- **Generated `README.TXT`:** one paragraph per target, from its flavour and `dir` (section 4), unless `readme` is set.

### 2.5 Pointing users to the right installer (general, driven by the media's target list)

The compiled project of each media target gains `others: Vec<(flavour, path)>`, for example `[(Win16, "WIN31\SETUP.EXE"), (Win32, "WIN95\SETUP.EXE")]`.

**DOS runtime (install mode only):**

- Call INT 2Fh AX=1600h. AL=3 means 386 Enhanced mode Windows 3.x; AL=4 means Windows 95, 98 or Me (in a DOS box). AL=0, 80h, 1 or FFh means none.
- Then INT 2Fh AX=4680h, where AX=0 means standard or real mode 3.x.
- If Windows is running, show a built-in page: "Windows 95 is running. Close this window, then choose Start, Run and type A:\WIN95\SETUP. Press D to install the DOS version anyway, or Esc to quit."
- On Windows 95, an `L` key could try `exec A:\WIN95\SETUP.EXE`, which Windows 95 starts from a DOS box [U]. Leave it out of v1.
- In "Restart in MS-DOS mode" no Windows is detected, so the DOS install proceeds. That is correct.

**Win16 runtime:**

- If `GetVersion()` reports 3.95 or later (Windows 95, 98, Me), show `MessageBox`: "This is Windows 95. Start Setup for Windows 95 (WIN95\SETUP.EXE) instead?"
- Yes: `WinExec` (new import) and exit 0.
- No: continue. The 3.1 build runs on 95, and the Program Manager DDE makes a Start menu group there [U].

**Win32 runtime on Windows 3.1:** it never starts. Windows 3.1 refuses a PE with its own error [U: exact text], so `README.TXT` covers that case.

**Polish, optional (M9):**

- A custom DOS stub for the two Windows programs ("Run A:\SETUP to install Quill for DOS"):
  - for the Win32 one, `lld-link /STUB:`;
  - for the Win16 one, through `ne_pack.py`.

### 2.6 Two disks: spanning in the runtimes (M8, general)

li-lpk already writes and reads spanned sets with a next-volume marker [V, `lpk-format.md` Spanning]. Only the runtime refuses volume 2.

- M8 makes `open_volume(n)` close the file, prompt "Insert disk n (label `<label> n`) and press Enter / OK", and open `COMMON.Lnn` at the same path. If the volume is wrong, it asks again. Cancelling is handled.
  - DOS: a text page.
  - Win16: `MessageBox` with Retry and Cancel.
  - Win32: `MessageBoxA`.
- With `overflow = "span"` and `exclude` removed, the floppy media becomes disk 1 plus disk 2.
- The packer puts files in stream order, so the compiler places the components with `default = false` (and then the largest files) last. Only users who tick "More dictionaries" are asked for disk 2.
- This also lifts the documented "No spanning yet" limit (`retro-dos.md:148`).

### 2.7 General versus Quill

| | General (installer repo) | Quill (`installer/` scripts) |
|---|---|---|
| Media model, dedup, `COMMON.LPK`, `others` | ✓ | |
| Floppy subdirectories, labels, serial, ISO, `AUTORUN.INF`, size gate, `SIZES.txt`, generated README | ✓ | |
| Windows detection and redirect, set-id check, spanning prompts | ✓ | |
| Staging the built binaries into `installer/dist/`, `README.TXT` wording, `CHANGES.TXT`, label `QUILL`, Fizz-style serial | | ✓ |
| Release naming `QUILL-<v>.IMG`/`.ISO`, `release.yml`, changelog notes, emulator runs from the final image | | ✓ |

## 3. Sizes (data area of a 1.44 MB floppy: 1,457,664 bytes)

**Inputs.**

| What | Raw (bytes) | Packed at 32 KiB | Packed at 4 KiB | Source |
|---|---|---|---|---|
| `QUILLDOS.EXE` | 196,608 | 112,885 | 123,174 | `quill16.wt-site/dos/dist` [V] |
| `QUILL.EXE` (with icon) | 326,528 | 144,811 | 157,507 | `quill16.wt-win16-gui/win16/out` [V] (`quill16/win16/out` has 236,802) |
| `QUILL32.EXE` (newest) | 361,472 | 216,035 | 235,816 | `quill16.wt-win9x-editing/win9x/dist` [V] (`wt-play`: 332,800) |
| `QUILL.HLP` | 6,518 | 1,980 | 2,152 | `wt-win16-gui` [V]; its budget is 190 KB |
| `EN_CA.DIC`, `EN_US.DIC`, `EN_GB.DIC` (SCOWL 50) | 300,086 / 298,454 / 298,270 | about 200,000 each | 210,547 / 209,556 / 209,261 | built by `quill-dict`, found in the scratchpad [V] |
| Same at SCOWL 35 | about 148,000 each | | about 98,500 each | [V] |
| 4 samples, `README.TXT`, `LICENCE.TXT` | | | 25,028 together | [V] |
| Programs (`SETUP.EXE`) | DOS 165,000 [U: 153,088 after D51, a scratch build at 161,280]; Win16 206,179 [V doc]; Win32 140,000 [U: "about 135 KB"] | | | |
| `SETUP.PRJ` | 16,000 budgeted per target [U] | | | |

Layouts below are rounded to 512-byte clusters, with two subdirectory clusters and a 2 KB root `README.TXT`.

| Layout | Total | Margin | Fits? |
|---|---|---|---|
| Per-OS disks, 3 dictionaries (earlier plan): DOS / W31 / W95 | 920,064 / 1,036,800 / 999,936 | +537,600 / +420,864 / +457,728 | yes |
| All-in-one, no sharing, 3 dictionaries | 2,953,728 | −1,496,064 | no |
| All-in-one, no sharing, `EN_CA` only | 1,735,168 | −277,504 | no |
| All-in-one, `COMMON.LPK` at 4 KiB, 3 dictionaries | 1,705,472 | −247,808 | no |
| (the same at 32 KiB, which Win16 cannot read) | 1,676,288 | −218,624 | no |
| **All-in-one, `COMMON.LPK` at 4 KiB, `EN_CA` only** | **1,286,656** | **+171,008** | **yes** |
| All-in-one, `EN_CA` plus `EN_US`, both size 50 | about 1,496,700 | about −39,000 | no |
| All-in-one, all three at size 35 | about 1,372,000 | about +85,000 | yes, but `EN_CA` loses words |
| `EN_CA` at size 50 plus `EN_US` and `EN_GB` at size 35 | about 1,484,000 | about −26,000 | no |
| Disk 2 (M8 spanning): `COMMON.L02` with `EN_US` and `EN_GB` | about 419,000 | about +1,038,000 | yes |
| CD (`cdrom`), everything | about 1.75 MB | no limit | yes |

**What this means:**

- **The floppy (`QUILL-<v>.IMG`):** all three OSes with `EN_CA` and about 171 KB free. That leaves room for `QUILL.HLP` to grow by about 150 KB packed, and for the uncertain program and `SETUP.PRJ` sizes.
- **The full dictionaries:** on the CD, or on the floppy set's disk 2 once M8 lands (`QUILL-<v>-2.IMG`, label `QUILL 2`).
- **Until M8:** the floppy's `README.TXT` points to the CD image or the zip for the other dictionaries.

**Other options I looked at:**

- Raise the Win16 window to 32 KiB through a `GlobalAlloc` segment. This saves only about 29 KB on this payload and needs far-pointer decoding in the 16-bit runtime. Not worth it.
- Have Quill ship one combined dictionary with variant flags, so the shared words are stored once. That is a Quill format change; note it for later. It would likely let all three dictionaries fit [U].
- One dual-mode `SETUP.EXE`: see section 4.

## 4. What the user sees

**Floppy volume label `QUILL`, serial `QD01-MMmP`** (the Fizz scheme: version in the serial). The CD's primary volume id is `QUILL` and its Joliet label is `Quill <v>`.

**Layout:**

```text
A:\  SETUP.EXE  SETUP.LPK  COMMON.LPK  README.TXT  LICENCE.TXT  [AUTORUN.INF on the CD]
     WIN31\SETUP.EXE  WIN31\SETUP.LPK
     WIN95\SETUP.EXE  WIN95\SETUP.LPK
```

| OS | What to type or click | If they start the wrong one |
|---|---|---|
| DOS | `A:\SETUP` | `WIN31\SETUP`: "This program requires Microsoft Windows." `WIN95\SETUP`: "This program cannot be run in DOS mode." (better text after M9) |
| Windows 3.x | Program Manager, File, Run: `A:\WIN31\SETUP`. Or File Manager, drive A, `WIN31`, double-click `SETUP.EXE` | `A:\SETUP` opens a DOS box. The DOS runtime detects Windows (AL=3) and says to run `A:\WIN31\SETUP`, or D to install the DOS version anyway. `WIN95\SETUP`: Windows 3.1 refuses it [U: text] |
| Windows 95/98/Me | Start, Run: `A:\WIN95\SETUP`. On the CD, AutoPlay opens it | `A:\SETUP` runs in a DOS box; AL=4, so the DOS runtime explains and offers D or Esc. `WIN31\SETUP` asks "Start Setup for Windows 95 instead?" and starts it with `WinExec` |

**On CD.** DOS and Windows 3.x see the level-1 names through MSCDEX (`;1` removed), and `VOL` shows `QUILL`. Windows 95 sees the Joliet names, which are the same since every name is 8.3, and runs `AUTORUN.INF`. All three runtimes open their packs read-only (finding 0.3).

**`README.TXT` (ASCII, CRLF, at most 76 columns), Quill's own:**

- the name and version;
- the three "to install" lines from the table;
- uninstall:
  - DOS: `SETUP /U` from this disk;
  - Windows 3.x: the "Uninstall Quill" item, from the kept uninstaller in the first wave;
  - Windows 95: Add/Remove Programs;
- the requirements;
- "the US and UK dictionaries are on the CD or disk 2";
- "keep each `SETUP.EXE` with its `SETUP.LPK`, and `COMMON.LPK` at the top".

**A single dual-mode `SETUP.EXE`: no.**

- A file has one `e_lfanew`, so NE (Windows 3.x) and PE (Windows 95) cannot share one file.
- DOS plus one Windows format is possible in principle: the DOS runtime would be the MZ stub of the Windows program. But the DJGPP go32 stub with CWSDPMI finds its COFF image from the MZ header and may use the bytes around 0x3C [U]. It would be a loader spike, with real-hardware risk.
- The subfolders give the same experience without that risk.

## 5. Quill: build script, release workflow, versioning, emulator tests

**`installer/tools/build-media.sh <version>`**, replacing the earlier `build-disks.sh`:

1. Build or take the three Quill binaries, the help file and `quill-dict --size 50`.
2. Run `stage.sh`.
3. Run `liminst.sh` (pinned commit).
4. `liminst build --media floppy`, then `--media cd`.
5. Copy and rename to `installer/out/release/`: `QUILL-<v>.IMG`, `QUILL-<v>.ISO`, `QUILL-<v>.zip` (the CD tree), later `QUILL-<v>-2.IMG`, and `SHA256SUMS`.
6. Checks, after Fizz:
   - the image is 1,474,560 bytes;
   - `mdir -/ -i IMG ::` lists exactly the expected tree, with the label;
   - every file passes `mtype | cmp` against the tree;
   - `liminst lpk verify` passes on every pack;
   - free space is at least 16 KiB (the size gate);
   - `xorriso -indev ISO -find` and `isoinfo -J` list the same tree;
   - every time stamp equals `SOURCE_DATE_EPOCH`, with `TZ=UTC`.
7. Write `SIZES.txt`.

**Version.**

- `[workspace.package] version` is the single source. Bump it to `0.1.0` for the first tag.
- The script fails if the tag minus its `v` differs from the Cargo version.
- `--version` goes to liminst, which writes it into `DisplayVersion`, the receipt and `README.TXT`.
- `CHANGES.TXT` on the CD is made from `CHANGELOG.md`, as Fizz does it.

**`.github/workflows/release.yml`**, a copy of the Fizz workflow:

- Runs on `push: tags: ['v*']` and `workflow_dispatch` with `inputs.version`, and `concurrency: release-${{ github.ref }}`.
- **`build` job:**
  - the pinned shared CI image (`ci-rust` or `ci-dos` by digest, per `AGENTS.md`) [U: which image has DJGPP, lld and mtools];
  - check out Quill;
  - check out `liminal-hq-labs/liminal-installer` at `installer/liminst.rev`, cached by that SHA;
  - run Quill's checks;
  - `build-media.sh "$VERSION"`;
  - `upload-artifact release-files`.
- **`publish` job** (`needs: build`, `if: github.event_name == 'push'`, `contents: write`):
  - sparse checkout of `CHANGELOG.md`;
  - `awk` the section into `NOTES.md`, failing if it is empty;
  - `--prerelease` for `0.*`, never a draft;
  - `gh release create "$VERSION" --repo liminal-hq-labs/quill --verify-tag --title "Quill $VERSION" --notes-file NOTES.md --generate-notes` with `QUILL-*.IMG QUILL-*.ISO QUILL-*.zip SHA256SUMS`.
- Add `.github/release.yml` with the Fizz label categories.
- Emulator runs stay local: the Windows trees cannot be redistributed (Quill D21).

**`README.md` "Get it" paragraph:**

- each asset by name;
- the floppy volume label `QUILL`;
- the three commands;
- CD versus floppy (dictionaries);
- that it is checked in DOSBox Staging and DOSBox-X only.

**Emulator tests from the final images** (`installer/tools/e2e-{dos,w31,w95}.sh` with `IMG=` or `ISO=`):

| Path | How it is mounted | What the test does |
|---|---|---|
| DOS | DOSBox Staging, `imgmount a QUILL.IMG -t floppy`; CD: `imgmount d QUILL.ISO -t iso` | `A:\SETUP` with `DOS_AUTOTYPE`, then the earlier Q3 checks (receipt, first frame of `QUILLDOS.BAT`, `SETUP /U`) |
| DOS under Windows | the 3.11 harness, a DOS box running `A:\SETUP` | the redirect message in the `LI_DUMP` text |
| Windows 3.11 | DOSBox-X; add `imgmount a … -t floppy` (or `imgmount e … -t iso`, using DOSBox-X's own MSCDEX) to the `[autoexec]` of `run-w31.sh` before `win` | `A:\WIN31\SETUP.EXE /PLAY …`, then the earlier Q4 checks plus `[Extensions]` (default run versus `.RTF` ticked) |
| Windows 95 | `scripts/win95/run-win95.sh` gains `-a IMG` (`imgmount 0 IMG -t floppy` before `boot -l c`) and `-i ISO` (`imgmount d ISO -t iso -ide 2m`) | `A:\WIN95\SETUP.EXE /PLAY …`; the ISO run proves AutoPlay with a screenshot; also run `A:\WIN31\SETUP` to prove the hand-off; then the earlier Q5 checks plus the `.txt`/`.rtf` registry |

- [U] Whether Windows 95 under DOSBox-X reads an emulated floppy through its own driver, and whether it sees the IDE CD with its own driver. If not, copy the image's tree onto the D: exchange disk, keeping the layout. That still tests the layout and the relative `COMMON.LPK` path, but not the media. Record which of the two was used.
- Run the size gate and the image checks in CI on every pull request that touches `installer/**`, using stand-ins.

## 6. Pull requests (stacked, each sized for one Sonnet implementer)

First wave, in progress elsewhere: L1 (runtime discovery), L2 and L3 (Program Manager group over DDE), L10 (kept Win16 uninstaller).

### Installer repo

| PR | Branch, built on | Content | Acceptance |
|---|---|---|---|
| L9 | `feat/component-defaults`, on L2 | `default` (model, check, studio, web, engine plan, retro screens and seeding); a `component` index on component-derived ops and selection gating in `do_install`; explicit-tag decoder; `FORMAT` bump; docs | Host goldens: unticked component's checkbox off and its shortcut not made; decoder refuses an unknown tag; web and studio tests; `cargo nextest` green |
| L4 | `feat/file-types-model`, on L9 | `Op::FileType`; `Shell::{register,unregister}_file_type` returning the previous owner; the `A` record; uninstall restore only if still ours; rollback; sim; Win16 extension-length check; Planboard gets `.planboard` (default on) and `.txt` (default off) | Sim tests: default, ticked, restore, "user changed it since" and rollback |
| L5 | `feat/win32-file-types`, on L4 | Registry writes, `SHChangeNotify` (`imports.allow`), deepest-first delete, `wine-e2e.sh` checks | `win32/check.sh`; Wine: `HKCR\.txt` is `txtfile` → ours → `txtfile` |
| L6 | `feat/win16-file-types`, on L3 and L4 | `GetProfileString`/`WriteProfileString` (ordinals checked with `ne_dump`), `WM_WININICHANGE` | 3.11 run: the `WIN.INI` line added and restored; Wine logged |
| M1 | `feat/floppy-subdirs`, on main (independent) | `floppy_1440` with `/` paths, subdirectories, label and serial; recursive `read_floppy` | Round-trip tests; `mdir -/` in a test when mtools is present; the existing images do not change |
| M2 | `feat/lpk-shared-packs`, on main | `lpk-format.md` "Shared packs"; `common.lpk` golden; optional reader check of the target mask | Golden read on every run; documentation lint |
| M3 | `feat/media-model`, on L9 | `[[media]]` in `li-model` and `check` (labels, `dir` 8.3, targets known, `autorun` names a Win32 target, `exclude` known); the compiler's dedup into `Compiled` sets plus `FileEntry.pack`, `CommonRef` and `others`; `FORMAT` bump | Host tests: Planboard-based three-target media gives one common entry per shared file at 4 KiB with mask 7; each project refers to the right set id and path |
| M4 | `feat/runtime-common-pack`, on M3 | `Platform` `PackId`/`open_pack` (sim, DOS, Win16, Win32); `Packs` with a shared window; lazy open at commit; set-id check and message | Sim: install from two packs is byte-equal; a missing or wrong `COMMON.LPK` fails before any write; uninstall without `COMMON.LPK` passes. DOS e2e and 3.11 run with a shared Planboard tree; Win16 heap peak logged under 40 KiB |
| M5 | `feat/cli-media-floppy`, on M1 and M4 | `liminst build --media`; the tree, the image, the zip, `SIZES.txt`, `reserve` gate with a non-zero exit, generated `README.TXT`, `extra` | CLI test with stand-in runtimes: the image tree and label are right; an oversized payload exits 1 with a per-file overrun |
| M6 | `feat/media-redirect`, on M4 | DOS INT 2Fh 1600h/4680h page; Win16 `GetVersion` plus `WinExec` (verified ordinal); `others` used | DOSBox Staging: no redirect. 3.11 DOS box: the message. Windows 95 harness: `WIN31\SETUP` starts `WIN95\SETUP` |
| M7 | `feat/cli-media-cdrom`, on M5 | `li-package::iso` (level 1 plus Joliet, deterministic), `AUTORUN.INF`, the `cdrom` kind | `xorriso`/`isoinfo` cross-check in tests; DOSBox Staging `imgmount -t iso` install of Planboard |
| M8 | `feat/runtime-spanning`, on M4 (later) | `open_volume` prompts in all three runtimes; `overflow = "span"`; ordering of default-off components last; `common-span` golden | Sim disk swap; DOSBox Staging two-floppy swap (`imgmount a d1.img d2.img`, Ctrl-F4); 3.11 run |
| M9 (optional) | `polish/exe-stubs` | Custom DOS stubs for the two Windows programs | Running each from DOS prints the new text |
| L7 (optional, from the earlier plan) | `ci/publish-runtimes` | Publish the three runtimes and `liminst` | Lets Quill pin a release instead of a commit |

L9, M1 and M2 can run in parallel. M3 waits for L9. M4 waits for M3. M5 waits for M1 and M4. M6 and M7 wait for M5 or M4.

### Quill repo

| PR | Branch, built on | Content | Acceptance |
|---|---|---|---|
| Q0 | `docs/release-media` | Updates the earlier Q0: `.LPK`; one all-in-one floppy plus CD replaces the per-OS disks; the sizes table above; SPEC line 108 (`SETUP.EXE` at the root plus subfolders, the redirect); the roadmap's "never split" line and budgets; a new decision (D91 or the next free number) | Markdown lint |
| Q1 | `feat/installer-project` (earlier plan) | Adds `assoc-qld`, `assoc-rtf` and `assoc-txt` (needs L9 pinned), `dicts` (`EN_CA`) and `dicts-more` (`EN_US`, `EN_GB`) as separate components, and the two `[[media]]` entries (needs M3 pinned; until then only `-t`) | `liminst check` green on stand-ins |
| Q2 | `build/media`, on Q1 and pin M5/M7 | `build-media.sh`, `README.TXT`, `CHANGES.TXT`, serial, the Fizz-style checks and gate, `SIZES.txt` | `QUILL-0.1.0.IMG` built from `main`'s binaries with at least 16 KiB free (expected about 171 KB); the ISO lists the same tree |
| Q3, Q4, Q5 | `test/install-{dos,w31,w95}`, on Q2 | The earlier plan's tests, run from the final image and ISO; association runs (default versus `.RTF` ticked); redirect cases (pin M6) | Evidence and screenshots in `docs/media/` |
| Q6 | `ci/installer-check`, on Q1 | Pull request job: stand-ins, `check`, `preview`, media build and gate | actionlint passes; check green |
| Q7 | `ci/release`, on Q2 and Q6 | `release.yml`, `.github/release.yml`, `CHANGELOG.md` with a `## 0.1.0 (date)` section, the Cargo version bump, the README "Get it" paragraph | A dispatch with `v0.1.0-test` uploads an artefact with the IMG, ISO, zip and `SHA256SUMS`; a tag on a fork publishes a pre-release, not a draft |

## 7. Decisions for you (my recommended default in bold)

1. **Shared payload:** a general `[[media]]` with automatic dedup into `COMMON.LPK`, or explicit `shared = true` components.
2. **Floppy content:** all three OSes with `EN_CA` only (+171 KB), plus `QUILL-<v>-2.IMG` with the US and UK dictionaries once M8 lands. Alternatives: all three dictionaries at SCOWL 35 (+85 KB, a weaker `EN_CA`), or `EN_CA` only and never a disk 2.
3. **Folder names:** `\` for DOS, `WIN31\`, `WIN95\`.
4. **Wrong-OS handling:** explain and offer to continue (DOS runtime); offer to start the right one (Win16 on 95); no automatic launch from a DOS box.
5. **`.QLD` component:** optional and ticked, or folded into the required program component.
6. **`.RTF` and `.TXT`:** take over the default and restore it if still ours. Alternatively, add a non-destructive "Open with Quill" verb only (Win95), as a later `mode = "verb"`.
7. **Release assets:** `QUILL-<v>.IMG` and `QUILL-<v>.ISO`, the CD tree zip and `SHA256SUMS`; per-OS images dropped. Pre-release for `0.x`, no draft (as Fizz does).
8. **Label and serial:** `QUILL` with a version-encoded serial, and `Quill <v>` as the Joliet label.
9. **ISO writer:** pure Rust in li-package, with xorriso only as the test cross-check. The alternative is shelling out to xorriso.
10. **Dual-mode `SETUP.EXE`:** no.

## 8. Risks

- **Win16 heap:** a second pack's directory and reader on a 33.7 of 40 KiB peak. A shared 4 KiB window keeps this to a few hundred bytes, but it must be measured on 3.11 (M4).
- **Sizes [U]:** the current DOS and Win32 program sizes and `SETUP.PRJ` are estimates (about 50 KB of uncertainty in total); `QUILL.HLP` growing to its 190 KB budget uses most of the margin. The gate catches both.
- **Windows 3.1 associations:** if another program has registered `.rtf` in `REG.DAT`, `ShellExecute` may prefer that over `WIN.INI` [U]. Test on 3.11 with Write's default.
- **Emulator media:** Windows 95 reading the floppy and the IDE CD under DOSBox-X [U]. The fallback, copying the tree onto D:, tests less.
- **Imports:** the `WinExec` ordinal and the profile-string ordinals until checked with `ne_dump`. The PE refusal text on Windows 3.1 is untested.
- **Format bumps** in L9, L4 and M3 must land with matching runtimes. That is safe only because Quill pins one installer commit for the programs and the packs.
- **Op gating (finding 0.1):** this changes behaviour for existing projects (unticked components lose their shortcuts). It is the correct behaviour, but it changes Planboard and Lt. Fizz goldens.
- **ISO writer:** correctness on MSCDEX with path tables and level-1 names. Cross-check with `isoinfo` and test in DOSBox Staging before trusting it.
- **CI image:** whether one pinned image has DJGPP, lld, the Win16 toolchain and mtools [U]. The build chain is long; cache by installer SHA.

### Critical files for implementation

- /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-retro/src/install.rs (op gating, file-type records, receipts, uninstall without `COMMON.LPK`)
- /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-retro/src/pack.rs and /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-retro/src/platform.rs (`Packs`, `PackId`, `open_volume` prompts)
- /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-retro-build/src/lib.rs and /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-retro-build/src/payload.rs (dedup, `FileEntry.pack`, op component index, chunk size choice)
- /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-package/src/dos.rs and /home/scott/source/liminal-hq-labs/liminal-installer/crates/li-cli/src/dos.rs (subdirectories, label, `--media`, the gate; a new `iso.rs` beside `dos.rs`)
- /home/scott/source/experiments/fizz-dos/tools/make-release.sh and /home/scott/source/experiments/fizz-dos/.github/workflows/release.yml (the release practice Quill's `build-media.sh` and `release.yml` copy)
