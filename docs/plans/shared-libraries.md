# Plan: shared libraries across the retro Rust projects

Quill, the Afterglow savers, afterfetch, the Liminal Installer and the ReactOS images all build retro programs from shared Rust crates, and every program links its own copy. This plan asks where a shared library (a Win32 or Win16 DLL, a Linux `.so`, or a static equivalent) would pay for itself, measures the one place where bytes are clearly duplicated, and gives a verdict per case, the rules a DLL must follow on each system, how the Liminal Installer places and removes one, and the work packages in order of value.

**Markers.** [M] measured for this plan (the method and raw figures are in `~/Documents/Liminal HQ/Liminal Installer/Shared libraries/`); [V] checked in the code or docs at the path given, on each repository's `origin/main` of 2026-11-27 (liminal-installer `aec86e0`, afterglow-savers `42dc1a4`, Quill `904c0e4`, rust-win16 `25cbe4b` and pull request 5 at `c4910c7`); [W] from Microsoft, Wine or otvdm sources, not run here; [E] estimate; [U] not checked, the implementer must check it. No emulator was started for this plan; one throwaway build of the savers was made and removed.

## Table of contents

- [1. Bottom line](#1-bottom-line)
- [2. What exists today](#2-what-exists-today)
- [3. Measurements](#3-measurements)
- [4. Case (a): one DLL for the Afterglow savers](#4-case-a-one-dll-for-the-afterglow-savers)
- [5. Case (b): Quill's kit and PDF crates](#5-case-b-quills-kit-and-pdf-crates)
- [6. Case (c): Windows 3.x DLLs](#6-case-c-windows-3x-dlls)
- [7. Case (d): afterfetch](#7-case-d-afterfetch)
- [8. Case (e): the installer's own runtime](#8-case-e-the-installers-own-runtime)
- [9. A `no_std` Win32 DLL without a C runtime](#9-a-no_std-win32-dll-without-a-c-runtime)
- [10. ABI, versions and side by side](#10-abi-versions-and-side-by-side)
- [11. Installing and removing a DLL with the Liminal Installer](#11-installing-and-removing-a-dll-with-the-liminal-installer)
- [12. Floppy space](#12-floppy-space)
- [13. Risks](#13-risks)
- [14. Decision ids reserved](#14-decision-ids-reserved)
- [15. Owner decisions](#15-owner-decisions)
- [16. Work packages](#16-work-packages)
- [17. Sonnet briefs](#17-sonnet-briefs)

## 1. Bottom line

A shared library pays only where two or more programs that land **on the same disk** carry the same code. Across these projects that happens once, in the Afterglow savers. Everywhere else a DLL would split one file into two, add headers, relocations and a version check, and save nothing; the static sharing the projects already do (one crate, linked into each build with LTO) is the right kind.

The Win16 case is different in kind: there a DLL is not about bytes but about a **second 64 KB data segment**, which is the only near memory a Windows 3.x program can add. That buys working memory for a cold, compute-only module (Quill's picture decoders), not room in the program's own segment.

| Case | What | Verdict | Why, in one line |
|---|---|---|---|
| (a) | `AGCORE1.DLL`: every scene, score, the shell and the core of the Win32 savers, with six stub `.scr` files, for the ReactOS images and the Quill remix | **GO** | Measured duplication: the five main files shrink from 1,025,536 to about 462,000 bytes (about 55%), all six from 1,196,544 to about 474,000, and the image parts by about 260 KB |
| (a′) | Quill Ghostwriter as shipped by Quill's own installers | **NO-GO** (stays standalone) | One saver alone: the DLL would add 406 KB to a 171 KB file; SAV-D22 keeps it shippable alone |
| (a″) | A DLL of only the shell and shared core, scenes left in each `.scr` | **NO-GO** | Saves less (about 320 KB) and needs a scene ABI across the boundary |
| (b) | `quill-pdf`, `quill-pdfdoc` or other Quill kit crates as a DLL or `.so` | **NO-GO** | One Quill build per machine: nothing to share; 34 KB of PDF code in `QUILL32.EXE` is not a reason |
| (b′) | A Win32 DLL for a future optional Quill feature of 100 KB or more | **Later** (W9X-41 in Quill's modules plan) | Only then is an optional, separately installed file worth a second link |
| (c) | The Win16 DLL mechanism in rust-win16 (pull request 5, then the runtime and its go/no-go gate) | **GO** for the spike | The only way to get a second near segment; the gate decides the rest |
| (c′) | `QUILLPIC.DLL`, Quill's Win16 picture decoders | **GO after the gate** | PNG and JPEG need more near working memory than `QUILL.EXE` can ever have |
| (c″) | A Win16 DLL to relieve `QUILL.EXE`'s or `SETUP.EXE`'s own data or code segments | **NO-GO** | What fills those segments cannot move (near statics, the heap, the stack); code segments are not short |
| (d) | afterfetch's core as a library | **NO-GO** | One program per system, 24 to 48 KB, no second consumer |
| (e) | A runtime DLL shared by `SETUP.EXE` and the uninstaller | **NO-GO** | They are already one program; the setup must stay one self-contained file |
| (e′) | A helper DLL inside `SETUP.LPK` for cold Win16 installer features | **Later**, only if the headroom plan's levers run out | Keeps the disk's file list; adds a loader path the runtime does not have |
| (e″) | Shared-file support in the installer (SharedDLLs counts, newer-version rule) | **Later**, when a second product installs the same DLL | No product does today; every planned DLL is private or image-only |
| — | DOS (DXE3, overlays) and Linux `.so` for any project | **NO-GO** | As Quill's modules plan found: no loader on DOS, no limit on Linux |

The order of work (section 16): the savers DLL first (the measured win, and the proof of the Win32 recipe every later DLL will use), the rust-win16 DLL runtime and gate in parallel, then the ReactOS recipe switch, then Quill's `QUILLPIC.DLL` if the gate passes, and a small installer check that warns about a DLL placed in the system folder.

## 2. What exists today

- **rust-win16 pull request 5** (`feat/ne-library`, open) [V]: `ne_pack.py --library MANIFEST.json` writes an NE DLL (flags `0x8301`: library, Windows API, single data; a moveable data segment; `LibEntry` as `CS:IP`; no stack; heap from the manifest), fixed ordinals from the manifest with gaps written as empty bundles, a resident `WEP` (a default `mov ax, 1 ; retf 2` when the program has none), and an import rule (a `__w16imp_` symbol outside the manifest's `imports` fails the build; the default list is KERNEL's `GlobalAlloc`, `GlobalLock`, `GlobalUnlock`, `GlobalFree`). D27 in its `docs/decisions.md`. The runtime (`LibEntry`, the per-export stack switch) is explicitly the next change, and nothing has been loaded by a real loader yet.
- **Quill's modules and DLLs plan** (`~/Documents/Liminal HQ/Quill/Quill modules and DLLs plan.md`, 2026-11-25) [V]: the full Win16 DLL design (the private stack and 32-bit alias inside the DLL's segment, no Windows calls from a module, explicit loading, `QUILLPIC.DLL` with five exports, the go/no-go of 30 of 30 runs on Windows 3.11, 10 of 10 under otvdm and a Wine smoke), the Win32 verdict (one `QUILL32.EXE`, W9X-40), DOS (no modules, DOS-50) and Linux (none). It reserves W16-40 to W16-48, W9X-40 and W9X-41, DOS-50 to DOS-52 and CORE-90 to CORE-91; none of those ids is used in Quill's repository yet [V]. This plan adopts its Win16 and Quill verdicts and does not repeat its detail.
- **Quill's parked item** "Win16 DLL go/no-go (rust-win16 #5)" in `Quill work queue.md` [V] is this plan's WP-SL-2.
- **Quill D14** [V] rejected "DLLs per subsystem" because a DLL runs with SS ≠ DS. The modules plan's W16-40 revises it for Rust code that never runs on the caller's stack.
- **The installer's vendored packer** (`retro/win16/tools/ne_pack.py`, from rust-win16 `4c0bac4`) [V] has no library mode, and `SETUP.EXE` has 12 to 180 bytes of its data segment free (`docs/plans/win16-headroom.md`), relieved by an allocator, a shallower stack and smaller tables, not by a DLL.
- **The installer's file flags** [V]: `.LPK` defines `REPLACE_IF_NEWER` (0x0002) and `SHARED` (0x0004) (`crates/li-lpk/src/format.rs`, `docs/lpk-format.md`), but no runtime honours either and no project field sets them.
- **`{windows}` and `{system}`** (D95, `docs/plans/system-folders.md`) [V]: a file directly in either folder, on `win16` and `win32-classic` targets; a file that was already there and is not the product's is backed up to `BACKUP\WIN\` or `BACKUP\SYS\` and put back on uninstall. That rule is right for a screensaver the product owns and wrong for a shared DLL (section 11).
- **The savers' build** [V]: six `.scr` files, each its own `[[bin]]` of `platforms/win32`, `opt-level = 3`, fat LTO, `/FIXED`, no `.reloc`, imports by name from `raw-dylib`. The Collection links every scene and score again (SAV-D29, SAV-D47); file size "is not a constraint, frame rate is" (SAV-D29), with guards of 384 KiB a file, 448 KiB for the Collection and 1.5 MiB for the five.
- **ReactOS** [V]: reactos-liminal writes the five savers to `ReactOS/system32` from a pinned savers commit (`docs/packing.md` WP-PACK-3 to WP-PACK-5), and the Quill remix adds `AGQUILL.SCR`. The web demo's disk is served as 1 MiB zstd parts; SAV-D55 records the savers at "about 1.0 MB on the disk and 0.45 MB in the parts".

## 3. Measurements

### 3.1 The savers, i586 [M]

| File | File | `.text` | `.rdata` | `.rsrc` | `.reloc` if relocatable | zstd -19 |
|---|---:|---:|---:|---:|---:|---:|
| `AGMAZE.SCR` | 164,864 | 145,101 | 10,272 | 7,552 | 4,956 | 74,586 |
| `AGCRACK.SCR` | 162,816 | 137,775 | 15,724 | 7,568 | 5,028 | 75,010 |
| `AGTWILIT.SCR` | 160,768 | 139,997 | 10,528 | 8,664 | 5,120 | 71,507 |
| `AGINK.SCR` | 136,192 | 116,093 | 10,092 | 8,624 | 4,160 | 63,583 |
| `AGCOLLCT.SCR` | 400,896 | 372,015 | 18,696 | 8,664 | 13,156 | 166,475 |
| `AGQUILL.SCR` | 171,008 | 148,175 | 13,808 | 7,560 | 4,576 | 78,807 |
| Five main files | **1,025,536** | | | | | 451,161 |
| All six | **1,196,544** | | | | | 529,968 |

Table 1. The shipping files (byte for byte those in the main checkout's `platforms/win32/dist/i586`), and `.reloc` from a rebuild with `/FIXED:NO`. x86-64: five main 1,158,656, all six 1,363,456; its Collection's `.reloc` is 672 bytes.

| File | Its own scenes and scores | Shared `savers-core` | Win32 shell | `core`, `alloc`, builtins | Entry (inlined) |
|---|---:|---:|---:|---:|---:|
| `AGMAZE` | 49,280 | 31,664 | 41,056 | 4,701 | 18,400 |
| `AGCRACK` | 40,384 | 32,032 | 41,808 | 5,183 | 18,368 |
| `AGTWILIT` | 38,861 | 33,280 | 41,424 | 4,640 | 21,792 |
| `AGINK` | 34,717 | 31,616 | 41,360 | 4,640 | 3,760 |
| `AGQUILL` | 54,352 | 38,144 | 41,808 | 6,415 | 7,456 |
| `AGCOLLCT` | 219,200 (all six) | 41,792 | 39,152 | 6,479 | 65,392 |

Table 2. `.text` by owner, from a rebuild with a COFF symbol table (`/DEBUG:SYMTAB`, same `.text` to the byte); each symbol owns the bytes to the next. LTO inlines across crates, so "entry" is each program's `start` with what was inlined into it.

**What this shows.** Each single saver carries about 78 KB of code that is the same in all of them (shared core about 32 KB, shell about 41 KB, runtime about 5 KB) and about 18 KB of data and resources. The five main files carry that five times. The Collection carries it once but repeats every scene and score (219 KB) that the single files already hold. That is why the Collection is smaller than the sum, and why the saving comes from putting **everything** in one DLL, not just the common part.

**Compression.** A compressor that sees all six files at once (`zstd -19 --long=27` over the concatenation) gets them to 256,231 bytes, against 529,968 one file at a time. The disk image's parts are 1 MiB slices of the disk, so files are compressed roughly one at a time; the DLL takes the duplication out before the compressor, wherever the files sit.

### 3.2 The other programs

| Program | Size | Shares code with, on the same disk | Source |
|---|---:|---|---|
| Quill `QUILL32.EXE` | 407,040; 441,344 with Print to PDF | Nothing (one Quill build per system) | Quill pull request 167 [V] |
| Quill `QUILL.EXE` (Win16) | 409,536; data segment about 2.8 KB free | Nothing | Quill pull request 167, `docs/progress.md` [V] |
| Quill `QUILLDOS.EXE` | 437,760 | Nothing | Quill pull request 167 [V] |
| Installer `SETUP.EXE` (Win16) | 253,184 | `UNINST.EXE`, which is a copy made on the target | `docs/plans/win16-headroom.md` [V] |
| Installer `SETUP.EXE` (Win32) | about 135 KB | As above | `docs/retro-win32.md` [V] |
| afterfetch | Win32 i586 24,576; Win16 35,136; DOS 47,616; Desk 23,940 | Nothing | `platforms/*/dist`, `docs/platforms.md` [V] |

Table 3. Only the savers have two programs with common code on one disk.

## 4. Case (a): one DLL for the Afterglow savers

### 4.1 Shape

**`AGCORE1.DLL`** holds what `AGCOLLCT.SCR` holds today: every scene and score, the shared core, the Win32 shell (window, settings dialog, registry, sound) and the allocator. It exports two functions with a C ABI. Each of the six `.scr` files becomes a **stub**: its resources (string 1, the icon, the version resource), and a `start` that finds the DLL beside itself, checks it and calls in with its `SaverKind`.

| Export | Signature (C) | Does |
|---|---|---|
| `AgCoreInfo` | `uint32_t AgCoreInfo(AgInfo *out)` | Fills `{ size, abi_major, abi_minor, kinds }` (four `uint32_t`); returns 1 |
| `AgSaverMain` | `uint32_t AgSaverMain(uint32_t abi_major, uint32_t kind)` | Runs the saver of that kind for this process's command line (`/s`, `/p`, `/c`, `/a`) and returns the exit code; returns 2 at once for an unknown kind or another `abi_major` |

Nothing else crosses: no Rust types, no allocation, no callback. The DLL reads the command line itself (`GetCommandLineA` is per process), and finds the stub's icon and strings through `GetModuleHandleA(NULL)`, which names the program, not the DLL [W]. `installed_beside("AGQUILL.SCR")` already names the program's folder (`GetModuleFileNameA(NULL)`), so the Collection's Ghostwriter rule (SAV-D29) is unchanged.

**Why not the narrower DLL** (shell and core only, scenes in each `.scr`). It saves about four copies of 80 KB, about 320 KB, against about 560 KB, keeps the Collection's second copy of every scene, and needs the `Scene` and `Score` traits to cross the boundary as a table of function pointers. Rejected.

### 4.2 Sizes and budgets [E, from Table 1]

| Part | Bytes |
|---|---:|
| `AGCORE1.DLL` | about 406,500: the Collection's code and data, its measured `.reloc` (13,156), a version resource, headers and about 150 of exports |
| Each stub | about 10,500 to 11,500: headers, about 1 KB of code, the imports, the measured resources |
| Five stubs and the DLL | about 462,000 (was 1,025,536) |
| Six stubs and the DLL | about 474,000 (was 1,196,544) |
| In the image parts (zstd, file by file) | about 190,000 (was 451,161 for the five) |

Budgets for the shared set (SAV-D81): the DLL at most 448 KiB on i586 and 512 KiB on x86-64, each stub at most 16 KiB, the DLL and five stubs at most 512 KiB on i586. The standalone files keep SAV-D29's guards.

**Run time.** No call crosses the boundary per frame: the whole saver runs inside the DLL as one LTO unit, so the frame loop is the Collection's code. The cost is a `LoadLibraryA` and, on i586 when the preferred base is taken, a relocation pass over 13 KB of entries, once per start. On Windows 9x a DLL whose file alignment is under 4 KB is read in whole rather than paged from the file [G]; the shared set reads about 417 KB at each start where a single saver reads about 165 KB, which an 8 MB machine absorbs. `/FILEALIGN:4096` on the DLL alone would avoid it at a few KB of padding; measure before choosing (section 15).

### 4.3 Two flavours, one build

- **Standalone** (as today): `platforms/win32/dist/<arch>/` holds the six self-contained files. Quill's installers ship `AGQUILL.SCR` from here (SAV-D22), and the five can still be copied anywhere one at a time.
- **Shared** (new): `platforms/win32/dist/<arch>-shared/` holds `AGCORE1.DLL` and six stubs. A cargo feature `shared` on `afterglow-win32` turns `screensaver_main!` into the stub's `start`; the bins' sources do not change, and LTO drops the scene code they no longer reach.
- `scripts/build-scr.sh OUT_DIR` keeps its contract (the six standalone files and `SHA256SUMS` at the top) and adds `OUT_DIR/SHARED/` with the seven shared files and their own `SHA256SUMS`.

### 4.4 Where the files go

- **ReactOS images and the Quill remix** (reactos-liminal): `AGCORE1.DLL` and the stubs go to `ReactOS/system32` together. Display Properties lists `.scr` files from its own folder, the system folder and the Windows folder [V: the savers' `docs/reactos-scr.md`], and the loader looks in the program's own folder first [W], so the DLL beside the stubs is always the one found. The images are built, not installed, so no reference count is needed.
- **Quill's installers**: the standalone `AGQUILL.SCR` only, to `{system}` on Win32, as the system-folders plan says. No DLL.
- **Anywhere else** (someone copying a stub without the DLL): the stub shows `Afterglow needs AGCORE1.DLL beside this screen saver.` for `/s` and `/c`, and exits quietly for `/p`, so Display Properties' preview never raises a box.

### 4.5 Verdict

**GO** (SAV-D80 to SAV-D83). It is the one measured duplication, it costs one package in the savers and one recipe change, and it proves the no-CRT Win32 DLL recipe (section 9) that Quill's W9X-41 would otherwise prove later. SAV-D29 says size is not a constraint, so this is worth doing for the images' parts and the web demo's download, not as a rescue; if the owner prefers to keep one kind of file, the cost of saying no is about 560 KB on each image.

## 5. Case (b): Quill's kit and PDF crates

- **What a DLL would share.** `quill-pdf` and `quill-pdfdoc` add 34,304 bytes to `QUILL32.EXE` (Quill pull request 167) and are planned for the GTK client's printing (Quill pull request 166) and the web demo. Those are three builds on three systems; no machine has two of them. `quill-core`, the kit crates (`quill-chrome`, `quill-paint`, `quill-strike`, `quill-grid`) and the editor crates are in the same position: each OS gets one Quill.
- **Tools.** `quill-cli` and the host tools run on the developer's machine, where size does not matter.
- **Linux.** A Rust `.so` has no stable ABI between compiler versions, so a Rust-to-Rust `.so` would have to be shipped and rebuilt with every program that uses it, which is what static linking already does without a loader. A C-ABI `.so` for outside users (a PDF writer for other programs) is a product decision, not a size one.
- **Static equivalents.** The crates are already shared at build time, and the layer rules (`scripts/check-layers.py`) keep them shareable. That is the "static library" answer, and it is the right one.
- **Win16.** `quill-pdf` is not linked on Win16 [V: pull request 167 rebuilt `QUILL.EXE` byte for byte]. A PDF DLL on Win16 would be a converter-style module (the modules plan's "maybe" row) and should wait for a plan of its own.

**Verdict: NO-GO** for a Quill DLL or `.so` on Win32, DOS and Linux. **Later** for a Win32 DLL holding an optional feature of 100 KB or more (W9X-41 in the modules plan), which section 9's recipe will by then have proved on the savers.

## 6. Case (c): Windows 3.x DLLs

### 6.1 What a Win16 DLL is

- **One data segment for every task.** A library with the single-data flag has one automatic data segment (its DGROUP), shared by every program that loads it, up to 64 KB, apart from the caller's [W]. It is near memory the caller can never have.
- **Called far, on the caller's stack.** An export is a far procedure reached through the entry table; Windows does not switch stacks, so inside the DLL SS is the caller's and DS is the DLL's (SS ≠ DS) [W]. LLVM's `code16` output treats every pointer as DS-relative, so Rust must never run on the caller's stack: each export switches to a private stack inside the DLL's own segment (the modules plan, section 3.3).
- **`LibEntry` and `WEP`.** `LibEntry` runs once when the module is first loaded, with DS = DGROUP, DI = hInstance, CX = the heap size and ES:SI = the command line; it returns AX = 1 (real Windows fails the load on 0; Wine ignores AX) [W]. `WEP` runs when the last user frees it, with `WEP_FREE_DLL` or `WEP_SYSTEM_EXIT`, on a small stack; Windows 3.1 looks it up by name and allows only a few KERNEL, USER and GDI calls there [W: KB Q112638].
- **Module names are global.** A second `LoadLibrary` of another file with the same module name returns the copy already loaded, whatever path was asked for [W]. So every DLL needs its own version check, and the module name is part of the ABI.
- **Search order.** Windows 3.1's `LoadLibrary` with a bare name looks in the current directory, then the Windows directory, then the system directory, then the program's own directory, then `PATH` [W]. A private DLL must be loaded by full path (from `GetModuleFileName`), or another program's file of the same name in `C:\WINDOWS` wins.
- **Implicit or explicit.** An import of a missing DLL makes Windows refuse to start the program with a system box; `LoadLibrary` returns a value under 32 on failure, and `SetErrorMode(SEM_NOOPENFILEERRORBOX)` stops the box [W]. A helper DLL is loaded explicitly.
- **Segments.** The DLL's data segment is moveable (fixed DLL data costs conventional memory, KB Q105274 [W]); its code segments follow the packer's veneers as the EXE's do, and pull request 5 keeps that working [V].

### 6.2 Windows 3.0, 3.1 and what the projects support

| System | DLLs | What the projects need |
|---|---|---|
| Windows 3.0 | Loads NE libraries; `WEP` must be in a fixed segment and is not reliably called at system exit [W, unverified]; `VER.DLL` is not part of the system | Out of scope: Quill (D4) and the installer (D46) need a 386 in 386 Enhanced mode; neither runs on 3.0 |
| Windows 3.1 and 3.11, 386 Enhanced | The KRNL386 model above; `VER.DLL` ships in `SYSTEM` | The target |
| Windows 95, 98, Me (16-bit programs) | The same Win16 model in KRNL386 [W] | One hand run, best effort |
| otvdm on ReactOS, Wine `krnl386` | Load NE DLLs; segments always moveable; `LibEntry`'s AX ignored; `SwitchStackTo` differs between them (otvdm issue 745) [W] | The gate runs on both; the design never calls `SwitchStackTo` |

### 6.3 What the packer has and lacks

Pull request 5 writes the file [V]. What is missing, all in rust-win16 (D28 to D30 there):

1. **The runtime** (`w16::dll`): `LibEntry` (store hInstance, return 1), `WEP` (free the alias, return 1), and `export_dll!(Name, rust_fn, words)`, the thunk that saves the caller's SS:ESP, loads the DLL's DS and ES, switches to the private stack through a 32-bit alias of the DLL's segment (checked against the segment's base at every entry, since the segment moves), calls the Rust function, switches back and returns `retf 2 × words`. A panic switches back, poisons the module and returns an error status.
2. **The caller's side**: a dynamic stub kind in `gen_imports.py` that calls `far [slot]` through a 4-byte slot `GetProcAddress` fills, returning a missing-status when the slot is zero; and the KERNEL rows `LoadLibrary` (95), `FreeLibrary` (96) and `GetProcAddress` (50) in `imports.tbl`, checked with `--check-spec`.
3. **The proof**: `examples/hellodll` and `examples/dllhost`, and the go/no-go of the modules plan section 3.6 (30 of 30 runs on Windows 3.11 in DOSBox-X, 10 of 10 under otvdm on ReactOS, a Wine smoke, 200 load and unload cycles with no lost selectors, a forced segment move between calls, the panic, missing-file and wrong-version paths).

### 6.4 Where a Win16 DLL helps, and where it does not

| Program | Segment that is short | Can a DLL relieve it? |
|---|---|---|
| `QUILL.EXE` | Its data segment (about 2.8 KB free) | Not directly: the `App`, the stack, the UI text and the `Store` table are near because Quill's code reaches them through DS. What a DLL gives is about 48 KB of **separate** near scratch for a module that brings its own data, which is what the picture decoders need (PNG at 4,096 px needs about 126 KB of locals) [V: modules plan]. **GO after the gate** for `QUILLPIC.DLL` |
| `QUILL.EXE` | Code | Not short: 253 segments are possible and 8 are used [V: modules plan] |
| `SETUP.EXE` | Its data segment (12 to 180 bytes free) | No: two thirds of it is the Rust heap holding `li-retro`'s state, which every page uses; the rest is the stack and the edit controls' local heap. The headroom plan's leaner allocator (WP-W16-1) frees 4 KB with one file. **NO-GO** |
| `SETUP.EXE` | Code | Not short: the packer opens another segment when one fills; veneers cost bytes, not correctness |
| afterfetch `AFETCHW.EXE` | Neither | **NO-GO** |
| Ghostwriter `QUILLGW.SCR` | One saver | **NO-GO** |

### 6.5 Placement and versioning on Windows 3.x

- **Private, beside the program.** `QUILLPIC.DLL` goes in Quill's folder, loaded by full path; nothing goes in `WINDOWS\SYSTEM` (the modules plan section 12, Quill W16-45).
- **`VER.DLL` is not needed.** Its `VerFindFile` and `VerInstallFile` exist for shared files in `SYSTEM` (they compare version resources, refuse an older file, and report a file in use) [W]. A private DLL installed with its program, checked at run time by its own info call, has none of those questions. If a Win16 product ever must place a shared DLL in `SYSTEM`, the installer compares the version resources itself rather than importing `VER.DLL` (section 11.3).
- **In use.** Windows 3.x cannot replace a loaded module and has no delayed rename [W]; the installer must ask the person to close the program first, which `GetModuleHandle("QUILLPIC")` can detect [U: the runtime has no such check yet; add it with the first DLL].
- **Version resource.** A `VERSIONINFO` matching the EXE's is a courtesy for File Manager, not a check; the info export is the check.

### 6.6 Verdict

**GO for the mechanism** in rust-win16 (merge pull request 5, then WP-SL-2), because without it the modules plan's picture route cannot be tried. **GO after the gate** for `QUILLPIC.DLL`, as the modules plan orders it (its PRs 3 to 6). **NO-GO** for a DLL in the installer's Win16 runtime or as relief for any program's own segments.

## 7. Case (d): afterfetch

One program per system, 24 to 48 KB, with its core (`afterfetch-core`, `afterfetch-art`) linked statically into each platform build [V]. Nothing else on the same disk uses that core: the ReactOS images install `afterfetch.exe` and four one-line batch files that run it [V: reactos-liminal `docs/afterfetch.md`]. The Desk build is already a `.DMD` module, which is Desk's program format, not a shared library. A DLL would add more than the 24,576-byte i586 file's whole budget in headers and relocations. **NO-GO.**

## 8. Case (e): the installer's own runtime

- **There is nothing to share.** The uninstaller is the setup program: on Windows 3.1 and Win32 the install copies the running `SETUP.EXE` to `UNINST.EXE` and the compiled project to `UNINST.DAT` (D66, D78) [V]; on DOS, uninstall runs from the install media. A runtime DLL for "setup and uninstall" would split one program into an EXE and a DLL that always travel together.
- **It costs floppy bytes.** A DLL adds its own headers, an export table, relocations (about 3% of an i586 image, Table 1) and a version check, and every product then carries two files. The setup must stay one self-contained file beside its pack (D110).
- **Win16 data relief comes from elsewhere.** The headroom plan's WP-W16-1 to WP-W16-6 win back 4 to 8 KB in the one file (section 6.4).
- **Later, perhaps: a helper inside the pack.** If the headroom plan's levers run out when repair, update and `COMMON.LPK` land, a cold Win16 feature could live in a helper DLL carried **inside** `SETUP.LPK`, written to the temporary folder and loaded by full path, so the disk keeps the same two files. It needs WP-SL-2's runtime, a temporary-file path in the runtime, and the same gate. Revisit only when the headroom plan's gate fails (D110's trigger).
- **Later, perhaps: one uninstaller for every product.** Each product keeps a 135 to 253 KB `UNINST.EXE` in its folder. A shared uninstaller in the Windows folder would save that per product on the hard disk, not on the floppy, at the price of versioning every installed product against one file. Not planned.

**Verdict: NO-GO** for a runtime DLL on every retro target.

## 9. A `no_std` Win32 DLL without a C runtime

The recipe WP-SL-1 proves on the savers, for any later Labs DLL (Quill's W9X-41 among them). Items marked [U] are what the package must confirm.

- **Crate type.** A `cdylib` package (for the savers, `platforms/win32/agcore`) in the same workspace, so it shares the custom targets (`i586-win32.json`, `x86_64-win32.json`, which already say `dynamic-linking` and `dll-suffix`), `-Zbuild-std` and the profile. `#![no_std]`, `#![no_main]`, its own `#[global_allocator]` (the process heap, as SAV-D3) and `#[panic_handler]` (which ends the process: a saver's panic ends the saver).
- **Link line.** rustc passes `/DLL` and an export list for a `cdylib`. The workspace's flags add `/ENTRY:start` and `/FIXED`, which a DLL must not have: the DLL's `build.rs` adds `/ENTRY:DllMain`, `/FIXED:NO` and `/BASE:0x63000000` with `cargo:rustc-link-arg-cdylib` [U: which `/ENTRY` and `/FIXED` lld-link honours when both appear; check with `llvm-readobj --file-headers`, which must show the DLL characteristic, no `RELOCS_STRIPPED`, the entry at `DllMain` and a `.reloc` section]. Keep `/NODEFAULTLIB`, `/SUBSYSTEM:WINDOWS,4.0` (5.02 on x86-64), `/FILEALIGN:512`, `/MERGE:.data=.rdata`, `/DEBUG:NONE`, `/Brepro`.
- **Exports.** `#[unsafe(no_mangle)] pub extern "C" fn AgSaverMain(...)`; `extern "C"` (cdecl) rather than `extern "system"`, so the i586 name has no `@n` suffix [U: `llvm-readobj --coff-exports` must list exactly `AgCoreInfo` and `AgSaverMain`, undecorated, and nothing else]. Names, not ordinals, are the ABI: the callers use `GetProcAddress` by name, which every Windows and ReactOS resolves the same way; an ordinal pin is not needed.
- **No import library.** The stubs load the DLL with `LoadLibraryA` by full path and find the exports with `GetProcAddress`, so nothing links against the DLL and no `.lib` is made. This also means a missing DLL is the stub's own error, never the loader's box. `LoadLibraryA`, `GetProcAddress` and `SetErrorMode` join `imports.allow` (all in Windows 95's KERNEL32 [W]).
- **`DllMain`.** `extern "system" fn DllMain(module, reason, reserved) -> i32`, which stores nothing and returns 1. No work under the loader lock: no `LoadLibrary`, no thread, no allocation. No thread-local storage (the targets say `has-thread-local: false`), so no TLS callbacks. Rust has no static constructors, so no CRT start-up is missing.
- **Relocations.** Required: a DLL cannot count on its preferred base. i586 code carries absolute addresses (13,156 bytes of `.reloc` for the Collection's code, Table 1); x86-64 code is mostly RIP-relative (672 bytes). The base `0x63000000` is clear of the program (`0x400000`), of `0x10000000` (the default many DLLs keep) and of Windows 9x's shared system DLLs above `0x80000000` [U].
- **Checks per file.** `check-pe.sh` learns the DLL's shape (characteristics, `.reloc`, exports, subsystem), `check-imports.sh` runs on the DLL and each stub against `imports.allow`, `scan-pentium.sh` on the DLL and stubs. The stubs import only KERNEL32 (and nothing the standalone files do not).
- **Loaders.** Windows 95 to Me: a DLL in the program's own folder is found first; sections must be 4 KB aligned in memory (lld's default) [W]. NT, ReactOS 0.4.16 and Wine: the same search order with SafeDllSearchMode, the program's folder first [W]; ReactOS's `ntdll` loader applies base relocations and calls `DllMain` with `DLL_PROCESS_ATTACH` as Windows does [U: the guest run is the proof].
- **Debugging.** `/MAP` on each file for a fault address; Wine's `WINEDEBUG=+loaddll,+module`; a ReactOS debug log on QEMU's serial port.

## 10. ABI, versions and side by side

The rules for every Labs DLL, Win32 or Win16:

1. **A C-style ABI and nothing else.** `#[repr(C)]` structs of fixed-width integers, far or flat pointers to caller-owned buffers, status codes. No Rust types, slices, traits, `String` or `Box` cross; memory is freed by the side that allocated it.
2. **An info export first.** Every DLL exports a call that fills `{ size, abi_major, abi_minor, … }`. The caller refuses another `abi_major`; a higher `abi_minor` is accepted. The struct's `size` lets a newer DLL add fields.
3. **The major version is in the file name.** `AGCORE1.DLL`, `AGCORE2.DLL`: an incompatible DLL is a new file that can sit beside the old one (side by side in the 8.3 way), so an old stub never loads a new, incompatible core. On Win16 the module name carries the same number, since module names are global.
4. **A version resource.** `VERSIONINFO` with the release's version in every DLL, matching its programs. It is what Explorer, File Manager and a shared-file installer compare (section 11.3).
5. **Shipped as a set.** The DLL and its programs come from one build and are pinned together (the savers' `SHA256SUMS`, reactos-liminal's outputs). The run-time check catches a hand-mixed copy; it is not a licence to mix.
6. **Fail softly.** A missing or mismatched DLL gives the program's own message (or silence in a preview), never a loader box and never a crash.

## 11. Installing and removing a DLL with the Liminal Installer

### 11.1 A private DLL needs nothing new

A DLL in the program's own folder is an ordinary `{install}/NAME.DLL` file: copied, recorded as `F name size crc` in `INSTALL.LIR`, removed on uninstall like the program (D111). This covers `QUILLPIC.DLL` and any private DLL. The one addition worth making when the first private Win16 DLL ships: refuse to install while the module is loaded (`GetModuleHandle`), as a running program would be.

### 11.2 A DLL in `{system}` or `{windows}` today

D95 treats a file in the Windows or system folder as the product's own: a foreign file already there is backed up and put back on uninstall. For a **shared** DLL that is wrong twice: it can replace a newer copy with an older one, and on uninstall it can put back an old copy or remove a file another product still uses. So until section 11.3 exists, `liminst check` warns (`system-folder-library`) when a `.dll`, `.drv` or `.exe` other than a `.scr` goes to `{system}` or `{windows}` (WP-SL-6). No planned product needs one.

### 11.3 Shared files, when a product needs them (later)

The design to build when the first trigger arrives (two products installing the same DLL), recorded so it is not rediscovered (D112):

- **Project.** A file rule gains `shared = true`, valid only with `{system}` or `{windows}`; the compiler sets the `.LPK` flags `SHARED` and `REPLACE_IF_NEWER` that already exist [V].
- **Install.** Compare the version resources (`VS_FIXEDFILEINFO` file version) of the file there and the one in the pack: keep a newer one, replace an older or unversioned one, never back it up (a shared file belongs to no product). On Win32 increment `HKLM\Software\Microsoft\Windows\CurrentVersion\SharedDLLs`, a `REG_DWORD` named by the file's full path; when the file exists but has no count, count the earlier user too (start at 2) [W]. Record `H path` in the receipt (a new record letter).
- **Uninstall.** Decrement; at zero, keep the file by default (Windows' own convention asks the person, and most answers are "keep") [W]. Never restore an old copy.
- **Windows 3.x.** There is no registry count. Count in a `[SharedFiles]` section of `LIMINAL.INI` in the Windows folder, which only Liminal products read, and never remove a shared file another installer may have placed (one with no count). Do not import `VER.DLL`: compare the version resources in `li-retro`, which is also what DOS and Win32 need.
- **In use.** Win32 can write the new file beside and replace it at the next start (`MoveFileExA` with `MOVEFILE_DELAY_UNTIL_REBOOT` on NT and ReactOS, `WININIT.INI`'s `[rename]` on Windows 9x) [W]; Windows 3.x cannot, so the installer asks the person to close the programs using it.
- **ReactOS.** It honours the same registry key, since the count is an installer convention, not the system's [W].

### 11.4 The savers and Quill

None of this is needed for the plan's GO cases: the savers' DLL is written by the image builder (reactos-liminal), never by an installer, and Quill's installers ship the standalone Ghostwriter (section 4.4).

## 12. Floppy space

A shared library saves floppy space only when two programs on **the same disk** share code. On every disk these projects make, that never happens: Quill ships one program per system's disk (D57 there), the installer is one program and copies itself as the uninstaller on the target, and the savers do not ship on floppies. On a single product's disk a DLL always costs bytes (headers, exports, relocations, a check). The savers' DLL saves space on the ReactOS images and in the web demo's download, which are not floppies.

## 13. Risks

| Risk | Where | Containment |
|---|---|---|
| DLL hell: an old or foreign `AGCORE1.DLL` found first | Win32 | Full-path load from the stub's own folder; `abi_major` in the file name and the info check; the set pinned by SHA-256 |
| A stub without its DLL raises a box in Display Properties' preview | Win32 | Explicit loading with `SetErrorMode`; `/p` exits quietly; a guest test with the DLL renamed |
| lld-link keeps `/ENTRY:start` or `/FIXED` from the workspace flags | Win32 build | The DLL's own link args, checked by `llvm-readobj` in `check-pe.sh` |
| i586 relocations wrong or missing, the DLL loads only at its base | Win32 | Load it in the guest with its base taken (a test that reserves `0x63000000` first under Wine) |
| ReactOS or Windows 9x loader differs from Wine | Win32 | The ReactOS guest run (WP-SL-3) and a Windows 98 hand run |
| Windows 9x reads the whole DLL on every start | Win32 | Measure; `/FILEALIGN:4096` on the DLL if it matters (section 15) |
| Two flavours of `AGQUILL.SCR` with one name | Savers | Different folders (`dist/<arch>/` and `dist/<arch>-shared/`, `OUT_DIR/SHARED/`) and different sums; Quill's recipe and installers name the folder |
| Rust runs on the caller's stack in a Win16 DLL (SS ≠ DS) | Win16 | Only the assembly thunk runs there (the modules plan); `hellodll` checks a stack buffer against a near copy |
| The Win16 DLL's segment moves and the alias goes stale | Win16 | A base check at every entry; `dllhost` forces a move (rust-win16 pull request 4's method) |
| Module name clash with another copy on Windows 3.x | Win16 | The info check and a message naming the other copy |
| A shared DLL installed into `{system}` through D95's backup rule | Installer | The `system-folder-library` warning now; section 11.3 when needed |
| Debugging across a boundary | All | A map per file; Wine `+loaddll`; ReactOS serial log; `ne_addr2sym.py` on Win16 |

## 14. Decision ids reserved

Each repository's own convention, with the next free numbers left as a gap so parallel work does not clash; a merge may renumber.

| Repository | Ids | For |
|---|---|---|
| liminal-installer | **D110** to **D113** (main has D98; D100 is used on the open `feat/system-folders-studio`, so D99 to D109 are left for work in flight) | D110 the retro setup stays one self-contained program with no runtime DLL, revisited only when the Win16 headroom gate fails; D111 a private DLL is an ordinary `{install}` file; D112 shared files (section 11.3) deferred until two products install one, the design recorded; D113 spare |
| afterglow-savers | **SAV-D80** to **SAV-D83** (main has SAV-D76) | SAV-D80 `AGCORE1.DLL` and six stubs for the images, beside the standalone files; SAV-D81 the shared set's budgets; SAV-D82 the C ABI, the info check and the file name's major version; SAV-D83 Ghostwriter stays standalone for Quill's installers (SAV-D22 kept) |
| rust-win16 | **D28** to **D30** (main and pull request 5 reach D27) | D28 the DLL runtime (`LibEntry`, `WEP`, the export thunk with its private stack and alias, no `SwitchStackTo`, no Rust on the caller's stack); D29 the dynamic stub kind and the three KERNEL rows; D30 the go/no-go result |
| Quill | **W16-40** to **W16-48**, **W9X-40**, **W9X-41**, **DOS-50** to **DOS-52**, **CORE-90**, **CORE-91** as the modules plan reserved them (none used yet) | Unchanged from that plan; this plan adds no Quill id |
| reactos-liminal | **RL-70** (main has RL-62; RL-63 is the savers' packing plan's) | The shared savers set in `system32` |

## 15. Owner decisions

| # | Question | Default taken | Where to swap |
|---:|---|---|---|
| 1 | Ship the images' savers as one `AGCORE1.DLL` and six stubs, when SAV-D29 says size is not a constraint? | **Yes**: about 560 KB less on each image and about 260 KB less in the web demo's parts, for one package | SAV-D80; WP-SL-1 and WP-SL-4 |
| 2 | Quill's installers ship the standalone `AGQUILL.SCR` rather than a stub and the DLL? | **Standalone** (171 KB against about 418 KB) | SAV-D83; Quill's installer components |
| 3 | Keep the standalone six building beside the shared set? | **Yes**: Quill needs one, and the five stay copyable one at a time | `build.sh`'s flavours (WP-SL-1) |
| 4 | The DLL's name: `AGCORE1.DLL`? | **Yes**: 8.3, the ABI major in the name | SAV-D82 |
| 5 | `/FILEALIGN:4096` on the DLL for Windows 9x paging? | **No** until a Windows 98 run shows a slow start | The DLL's `build.rs` |
| 6 | Merge rust-win16 pull request 5 and start the DLL runtime now? | **Yes**, in parallel with WP-SL-1 | WP-SL-2 |
| 7 | The installer warns on a library placed in the system folder? | **Yes**, a warning, not an error | `system-folder-library` in `check.rs` (WP-SL-6) |
| 8 | Shared-file counting (SharedDLLs, `LIMINAL.INI`) now or when needed? | **When a second product installs the same DLL** | D112 |
| 9 | Windows 3.x: never remove a shared file that has no Liminal count? | **Never remove** | D112, section 11.3 |

## 16. Work packages

Ordered by value. At most three builders at once (WP-SL-1 and WP-SL-2 are in different repositories and can run together).

| Order | Package | Repository | Needs | Size |
|---:|---|---|---|---|
| 1 | **WP-SL-1** `AGCORE1.DLL`, the stubs and the Wine checks | afterglow-savers | — | M, 2 to 3 days |
| 2 | **WP-SL-2** The Win16 DLL runtime and the go/no-go | rust-win16 | Pull request 5 merged | M to L, 3 days |
| 3 | **WP-SL-3** The shared set in the ReactOS guest, and the docs | afterglow-savers | WP-SL-1, WP-PACK-2's `guest.py` | S to M, 1 to 2 days |
| 4 | **WP-SL-4** The images take the shared set | reactos-liminal | WP-SL-3's merge commit, WP-PACK-4 | S, 1 day |
| 5 | **WP-SL-5** `QUILLPIC.DLL` (the modules plan's PRs 3 to 6) | Quill | WP-SL-2's go | L, 7 to 9 days |
| 6 | **WP-SL-6** D110 to D112 and the `system-folder-library` warning | liminal-installer | — | S, half a day |
| — | WP-SL-7 Shared-file support (section 11.3) | liminal-installer | A second product with the same DLL | Later |

### WP-SL-1 — `AGCORE1.DLL`, the stubs and the Wine checks (afterglow-savers)

- **Finish line.** `platforms/win32/build.sh i586` and `x86_64` write the standalone six to `dist/<arch>/` exactly as today (the same SHA-256 as `origin/main`) and the shared set to `dist/<arch>-shared/`: `AGCORE1.DLL` and six stubs. `check.sh` checks both. Under Wine every stub runs `/s`, `/p` and `/c` like its standalone file, the Collection plays its scenes, a stub with the DLL renamed shows its message for `/s` and stays silent for `/p`, and a stub with the DLL's base taken still runs. `scripts/build-scr.sh` adds `OUT_DIR/SHARED/` with its own `SHA256SUMS`.
- **Files owned.** `platforms/win32/Cargo.toml` (the `shared` feature, the `agcore` member), `platforms/win32/agcore/` (new: `Cargo.toml`, `build.rs`, `src/lib.rs`), `platforms/win32/src/lib.rs` (the macro's shared arm and a `stub` module), `platforms/win32/build.sh`, `platforms/win32/check.sh`, `platforms/win32/tools/check-pe.sh`, `platforms/win32/imports.allow`, `scripts/build-scr.sh`, `scripts/smoke-wine.sh` (a `--shared` mode), `Makefile`, `.github/workflows/ci.yml` (the shared checks in the existing Win32 jobs), `SPEC.md`, `docs/decisions.md` (SAV-D80 to SAV-D83), `docs/packing.md` (one paragraph), `docs/testing.md`, `docs/progress.md`.
- **Must not touch.** `crates/savers-core` (no scene, score or setting changes), `platforms/win16`, `platforms/linux`, the standalone files' bytes, the defaults, the existing size guards' numbers.
- **Tests.** `make check`; `bash platforms/win32/check.sh i586` and `x86_64`; the standalone sums equal to `origin/main`'s; `bash scripts/smoke-wine.sh --shared` on its own Xvfb; `bash scripts/check-archive-build.sh`; markdownlint and actionlint in Docker.
- **Budgets.** The DLL at most 458,752 bytes (i586) and 524,288 (x86-64); each stub at most 16,384; the DLL and five stubs at most 524,288 (i586). Report every size in the pull request against Table 1.

### WP-SL-2 — The Win16 DLL runtime and the go/no-go (rust-win16)

- **Finish line.** The modules plan's section 3.6 gate passes or fails with evidence: `examples/hellodll` (`Info`, `Add`, `FillFar`, `Poison`) and `examples/dllhost`; 30 of 30 runs on Windows 3.11 in DOSBox-X, 10 of 10 under otvdm on ReactOS, a Wine smoke, 200 load and unload cycles with `GetFreeSpace` and the free selectors unchanged, a forced segment move between calls, the panic, missing-file and wrong-version paths. D28 to D30 recorded with the result.
- **Files owned.** `src/dll.rs` (new), `src/lib.rs` (the module), `tools/gen_imports.py` (the dynamic kind), `tools/imports.tbl` (three rows), `src/imports.rs` (regenerated), `examples/hellodll/`, `examples/dllhost/`, `scripts/build.sh`, `scripts/run-w31.sh` (a `--dll` mode if needed), `tests/`, `docs/packer.md`, `docs/decisions.md`, `SPEC.md`, `.github/workflows/ci.yml` (build the examples).
- **Must not touch.** The EXE path of `tools/ne_pack.py` (its golden sums), Quill and the installer's vendored copies (they refresh by `PROVENANCE.md` later).
- **Tests.** `python3 -I -m unittest discover -s tests -v`; `bash scripts/build.sh`; the Windows 3.11 loop, otvdm and Wine runs above.
- **Budgets.** `HELLODLL.DLL`'s private stack at least 1,024 bytes of painted margin; the thunk under 64 bytes an export; `DLLHOST.EXE`'s data segment change reported.

### WP-SL-3 — The shared set in the ReactOS guest, and the docs (afterglow-savers)

- **Finish line.** WP-PACK-2's `scripts/guest/guest.py` runs over `dist/i586-shared` in a copy of the Afterglow image under the builder's own headless QEMU: the list shows the six names, every preview, Settings, Preview and the idle start work as with the standalone files, the AC97 capture has each score, and a run with the DLL renamed shows the stub's message for Preview and nothing in the monitor. A Windows 98 hand run if a VM is at hand. `docs/reactos-integration.md` names the shared set as the image contract. Its merge commit is the new pin.
- **Files owned.** `scripts/guest/` (a `--shared` mode), `docs/reactos-integration.md`, `docs/reactos-scr.md`, `docs/testing.md`, `docs/progress.md`; fixes in `platforms/win32/` that the guest shows, one commit each.
- **Must not touch.** `crates/savers-core`, the standalone files' bytes.
- **Tests.** The guest run; `make check`.

### WP-SL-4 — The images take the shared set (reactos-liminal)

- **Finish line.** `recipes/afterglow.toml` writes `AGCORE1.DLL` and the five stubs from `built:afterglow-savers/SHARED/…` to `ReactOS/system32`; the Quill remix's recipe adds the `AGQUILL.SCR` stub from the same folder; both locks are current; `docs/recipes.md`'s screensaver sizes are updated; RL-70 recorded.
- **Files owned.** `recipes/afterglow.toml`, `recipes/*.lock.json`, the Quill remix recipe's savers lines, `docs/recipes.md`, `docs/decisions.md`, and `reactos_liminal/recipe.py` only if WP-PACK-3's output names refuse a folder [U].
- **Must not touch.** `tools/afterfetch`, the patchers, `guest/reg/`.
- **Tests.** `./rl pin`, `./rl lock`, `./rl verify`, `./rl build recipes/afterglow.toml afterglow-savers`, the Python checks; the compact image built and booted once with the six names listed.

### WP-SL-5 — `QUILLPIC.DLL` (Quill)

The modules plan's PRs 3 to 6 (`refactor/image-work-buffers`, `build/w16-dll`, `feat/w16-png-dll`, `feat/installer-quillpic`) with its gate 2, started only after WP-SL-2's go and a provenance refresh of Quill's packer copy. Its finish lines, files and tests are in that plan, sections 3.6 and 9; this plan adds nothing to them.

### WP-SL-6 — The installer's decisions and the `system-folder-library` warning (liminal-installer)

- **Finish line.** D110 to D112 in `docs/decisions.md`, a sentence in SPEC §4 and §9, and `liminst check` warns `system-folder-library` for a file rule whose `to` is `{system}/NAME` or `{windows}/NAME` with an extension `.dll`, `.drv`, `.exe`, `.386` or `.vxd`; Lt. Fizz's `FIZZ.SCR` stays silent.
- **Files owned.** `crates/li-engine/src/check.rs`, its tests, `SPEC.md`, `docs/decisions.md`.
- **Must not touch.** `li-retro`, `li-retro-build`, the runtimes, the `.LPK` format and its golden files.
- **Tests.** `cargo nextest run -p li-engine --locked`; `make validate`.

## 17. Sonnet briefs

### Brief: WP-SL-1 (in full)

> You are a Sonnet builder on afterglow-savers (`liminal-hq-labs/afterglow-savers`). Never work in the main checkout `~/source/liminal-hq-labs/afterglow-savers`: run `git -C ~/source/liminal-hq-labs/afterglow-savers fetch origin`, then `git -C ~/source/liminal-hq-labs/afterglow-savers worktree add -b feat/shared-core ~/source/worktrees/afterglow-savers/shared-core origin/main`, and work only there. Read `AGENTS.md`, `SPEC.md`, `docs/decisions.md` (SAV-D3, SAV-D20, SAV-D22, SAV-D29, SAV-D47, SAV-D54 to SAV-D59), `docs/packing.md`, `docs/testing.md`, and liminal-installer's `docs/plans/shared-libraries.md` sections 3.1, 4, 9, 10 and 16 (WP-SL-1), which are your specification.
>
> **Goal.** Beside today's six standalone `.scr` files, build a shared set: `AGCORE1.DLL`, holding every scene, score, the shared core and the Win32 shell, and six stub `.scr` files that load it. The standalone files must come out byte for byte as on `origin/main`.
>
> 1. **The DLL.** Add a `cdylib` package `platforms/win32/agcore` (workspace member, `#![no_std]`, `#![no_main]`, SPDX header, the workspace's lints) depending on `afterglow-win32` and `savers-core`. Export, with `#[unsafe(no_mangle)] pub extern "C"`, `AgCoreInfo(out: *mut AgInfo) -> u32` (`#[repr(C)] struct AgInfo { size: u32, abi_major: u32, abi_minor: u32, kinds: u32 }`, `abi_major` 1, `abi_minor` 0) and `AgSaverMain(abi_major: u32, kind: u32) -> u32`, which returns 2 at once for another `abi_major` or an unknown kind and otherwise runs `afterglow_win32::run` with the same scene and score makers each bin passes today (one `match` on `SaverKind`; take the makers from the six `src/bin/*.rs`). `DllMain` is `extern "system"`, does nothing and returns 1. Its `build.rs` adds `cargo:rustc-link-arg-cdylib=` `/ENTRY:DllMain`, `/FIXED:NO` and `/BASE:0x63000000`, and a version resource with no icon (reuse the resource writer in `platforms/win32/build.rs` by moving it to a shared module if needed; keep the standalone resources' bytes). Confirm with `llvm-readobj --file-headers --coff-exports` that the file is a DLL, has `.reloc`, has its entry at `DllMain`, and exports exactly `AgCoreInfo` and `AgSaverMain`, undecorated, on both architectures. If lld-link honours the workspace's `/ENTRY:start` or `/FIXED` instead, find the order that works and say what you found.
> 2. **The stubs.** Add a cargo feature `shared` to `afterglow-win32`. Under it, `screensaver_main!` expands to a `start` that calls a new `afterglow_win32::stub::run(kind)` and ignores the makers (LTO then drops the scene code). `stub::run` builds the DLL's full path from `GetModuleFileNameA(NULL)` (as `installed_beside` does), calls `SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX)`, `LoadLibraryA`, `GetProcAddress` for both names, checks `AgCoreInfo`'s `size` and `abi_major`, and calls `AgSaverMain(1, kind as u32)`, exiting with its code. On any failure: for `/p` exit 0 with no window; otherwise show `Afterglow needs AGCORE1.DLL beside this screen saver.` with `MessageBoxA` (add it to `imports.allow` if it is not there) and exit 1. Add `LoadLibraryA`, `GetProcAddress` and `SetErrorMode` to `imports.allow` (all Windows 95 KERNEL32). The bins' sources do not change.
> 3. **The build.** `platforms/win32/build.sh` builds the standalone set as now, then the shared set (`--features shared` for the bins and the `agcore` package) into its own target subfolder (so the two flavours never share artefacts), copying `AGCORE1.DLL` and the six stubs to `dist/<arch>-shared/` with the same upper-case names. `scripts/build-scr.sh OUT_DIR` keeps its top-level contract and adds `OUT_DIR/SHARED/` with the seven files and their own `SHA256SUMS`. `check.sh` checks both sets: `check-pe.sh` learns the DLL's shape, `check-imports.sh` and `scan-pentium.sh` run on every file, and the budgets of WP-SL-1 apply to the shared set (the DLL at most 458,752 bytes on i586 and 524,288 on x86-64, each stub at most 16,384, the DLL and five stubs at most 524,288 on i586); the standalone guards stay as they are.
> 4. **Wine.** `scripts/smoke-wine.sh --shared` runs every stub as it runs the standalone files (`/s` for a few seconds with a non-blank capture, `/c` and `/p` against a host window), then: the DLL renamed (a stub's `/s` shows the message, `/p` exits 0 with no window), and the DLL's base taken (a tiny test program, or a `WINEDLLOVERRIDES`-free method you choose, that reserves `0x63000000` before loading the DLL and calls `AgCoreInfo`; it must succeed through relocation). Run Wine on an Xvfb of your own: pick a free display number, start `Xvfb :NN` yourself and stop only that PID, set `DISPLAY=:NN` explicitly on every command that needs a display, and `unset WAYLAND_DISPLAY` in the same command. Never touch `:0`, never `pkill` or `killall`. Use your own `WINEPREFIX` under `~/.cache/afterglow-savers/shared-core/wine`.
> 5. **Docs, in the same change as the behaviour.** SPEC (the shared set, the stub's behaviour), `docs/decisions.md` SAV-D80 to SAV-D83 (or the next free numbers if `main` has moved on), with the text of the plan's section 14, a paragraph in `docs/packing.md` that the images will take `SHARED/` once WP-SL-3 merges, `docs/testing.md` and `docs/progress.md`. Canadian English, sentence case, no hard-wrapped prose.
> 6. **Sizes.** Report in the pull request a table of every file of both sets on both architectures against the plan's Table 1, and the standalone set's sums against `origin/main`'s (they must be equal).
>
> **Must not touch.** `crates/savers-core`, `platforms/win16`, `platforms/linux`, the defaults, the standalone files' bytes, the existing guards' numbers.
>
> **Tests.** `make check`; `bash platforms/win32/check.sh i586` and `x86_64`; `bash scripts/smoke-wine.sh` and `--shared`; `bash scripts/check-archive-build.sh`; `docker run --rm -v "$PWD:/workdir" davidanson/markdownlint-cli2:v0.17.2 "**/*.md"`; `docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint`. `CARGO_BUILD_JOBS=4`; one heavy tool at a time; check `df -h /home` stays above 20 GB.
>
> **Git.** Small Conventional Commits (`feat(win32): …`, `docs: …`), each written to a file and committed with `git commit -F`, body in markdown with bold labels and no headings, hard-wrapped, ending `Co-Authored-By:` with your own model. Stage files by name; never `git add -A` or `git commit -a`. Push only `feat/shared-core`; open the pull request ("Build the savers as one shared core and six stubs"), `## Summary` of bold-led bullets, `## Test plan` of `- [x]` and `- [ ]` items ending with an unchecked `- [ ] Not verified: …` (the ReactOS guest and Windows 9x, which are WP-SL-3). Do not merge.
>
> **Cleanup.** One removal per command with a full literal path, for example `rm -r /home/scott/.cache/afterglow-savers/shared-core/wine`. No globs, variables, `$(...)` or loops in a removal.
>
> **Report.** The pull request URL, the size table, what you found about lld-link's `/ENTRY` and `/FIXED` order, and any `OWNER DECISION: question | default taken | where to swap` lines.

### Brief: WP-SL-2

> Sonnet builder, rust-win16. After pull request 5 (`feat/ne-library`) has merged, make `~/source/worktrees/rust-win16/dll-runtime` on `feat/dll-runtime` from `origin/main` (never the main checkout). Specification: Quill's modules plan (`~/Documents/Liminal HQ/Quill/Quill modules and DLLs plan.md`) section 3.3 "The runtime" and "Calling it from the EXE", and section 3.6, with liminal-installer's `docs/plans/shared-libraries.md` section 6.3. Build `src/dll.rs` (`LibEntry`, `WEP`, `export_dll!` with the private stack and its own 32-bit alias inside the DLL's segment, a base check at every entry, a poisoning panic path; no `SwitchStackTo`; no Rust on the caller's stack), the dynamic stub kind in `tools/gen_imports.py`, KERNEL `LoadLibrary` 95, `FreeLibrary` 96 and `GetProcAddress` 50 in `tools/imports.tbl` (checked with `--check-spec`), and `examples/hellodll` and `examples/dllhost`. Run the gate: 30 of 30 on Windows 3.11 in DOSBox-X with `W31_TREE=… scripts/run-w31.sh --runs 30` on your own Xvfb (`DISPLAY=:NN` set and `WAYLAND_DISPLAY` unset in the same command; your own cache folder under `~/.cache/rust-win16/dll-runtime`; give the script's DOSBox-X container a unique `rw16-dll-` name if it could clash with another agent's), Wine on your own Xvfb (`DISPLAY=:NN` set and `WAYLAND_DISPLAY` unset in every command), and otvdm on ReactOS in your own headless QEMU from a copy of the owner's image in your cache folder (`-display none`, the monitor on a short socket such as `/tmp/rw16dll.sock`), 10 of 10. Record D28 to D30 with the result. Commit via `git commit -F` with the house body and your model's trailer; push; open "Run Windows 3.x DLLs: the runtime and the go/no-go"; do not merge. Report the URL, the gate table and `OWNER DECISION:` lines; a mechanism fault you cannot fix in two days is a no-go, reported as such.

### Brief: WP-SL-3

> Sonnet builder, afterglow-savers, after WP-SL-1 merges. Worktree `~/source/worktrees/afterglow-savers/shared-guest`, branch `test/shared-guest`. Add a `--shared` mode to `scripts/guest/guest.py` and run every check WP-PACK-2 defines over `platforms/win32/dist/i586-shared` in a copy of the Afterglow image under `~/.cache/afterglow-savers/shared-guest`, QEMU headless (`-display none`, monitor socket `/tmp/agsh.sock`), plus a run with `AGCORE1.DLL` renamed (Preview shows the stub's message; the monitor stays empty with no error box). Check `df -h /home` and `free -g` before copying the image. Update `docs/reactos-integration.md` (the shared set is the image contract), `docs/reactos-scr.md`, `docs/testing.md` and `docs/progress.md`. Evidence screenshots to `~/Documents/Liminal HQ/Afterglow/Savers/images/shared-*.png`. House git, pull request "Check the shared savers in a ReactOS guest", do not merge; report the URL, the audio table and `OWNER DECISION:` lines.

### Brief: WP-SL-4

> Sonnet builder, reactos-liminal, after WP-SL-3 and WP-PACK-4 merge. Worktree `~/source/worktrees/reactos-liminal/shared-savers`, branch `feat/shared-savers`. Point `[builds.afterglow-savers]` at WP-SL-3's merge commit, list `SHARED/AGCORE1.DLL` and the five `SHARED/*.SCR` stubs (name each; no globs) as outputs with their sums, write them to `ReactOS/system32`, and give the Quill remix the `SHARED/AGQUILL.SCR` stub. If WP-PACK-3's output names refuse a folder, allow one level in `reactos_liminal/recipe.py` with a test. Run `./rl pin`, `./rl lock`, `./rl verify`, `./rl build recipes/afterglow.toml afterglow-savers` and the Python checks; build and boot the compact image once and confirm the five names list. Update `docs/recipes.md`'s sizes and record RL-70. House git, pull request "Install the savers as one shared core", do not merge.

### Brief: WP-SL-5

> Not before WP-SL-2's go. Follow Quill's modules plan PRs 3 to 6 as written there (each its own Sonnet builder), after a provenance refresh of `win16/tools` from rust-win16. Quill's checkout `~/source/experiments/quill16` is touched only by `git fetch` and `git worktree add` or `remove`; worktrees under `~/source/worktrees/quill/`.

### Brief: WP-SL-6

> Sonnet builder, liminal-installer. Worktree `~/source/worktrees/liminal-installer/system-folder-library`, branch `feat/system-folder-library` from `origin/main`. In `crates/li-engine/src/check.rs` add the warning `system-folder-library` for a file rule whose `to` is `{system}/NAME` or `{windows}/NAME` and whose name ends in `.dll`, `.drv`, `.exe`, `.386` or `.vxd` (any case), with the message "`{system}` and `{windows}` treat a file as this product's own; a library shared with other programs needs shared-file support, which the installer does not have yet (D112)". A `.scr` stays silent. One test per case in the style of `crates/li-engine/tests/project_and_flow.rs`. Add D110 to D112 (or the next free numbers) to `docs/decisions.md` from the plan's section 14 and section 11, and a sentence each to SPEC §4 (the warning) and §9 (shared files are not counted). Do not touch `li-retro`, `li-retro-build`, the runtimes or `li-lpk`. Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo nextest run -p li-engine --locked`, `make validate`. Commit `feat(engine): warn about a library placed in the system folder` via `git commit -F` with the house body and your model's trailer; push; pull request "Warn about a library placed in the system folder"; do not merge.
