# The Windows 3.1 runtime

`liminst build -t win16-x86` turns an installer project into `SETUP.LPK`, the Liminal Pack the Windows 3.1 runtime reads ([lpk-format.md](lpk-format.md)), and carries `SETUP.EXE`, the runtime, when `--win16-runtime`, `LIMINST_WIN16_RUNTIME` or `retro/win16/out/SETUP.EXE` names one; the `floppy-1440` output fails without it, naming `--win16-runtime` (D75). The runtime is Rust built for a 16-bit code segment and packed into an NE executable (D46, [win16-spike.md](win16-spike.md)), on the same portable core as the DOS and Windows 95 runtimes: the flow VM and installer engine of `li-retro`, a `Gui` for the pages (shared with Win32) and a `Shell` for what the machine's shell does (D69, D73).

**Status.** The whole `SETUP.EXE` links and runs. Its code is 138 827 bytes, which no single 16-bit code segment can hold, so the packer places it in four (the packer is vendored from rust-win16, see [`retro/win16/PROVENANCE.md`](../retro/win16/PROVENANCE.md)). Under Wine and on Windows for Workgroups 3.11 in DOSBox-X it installs and uninstalls Lt. Fizz and Planboard through its real controls: 40 of 40 runs passed on 3.11, 20 of each example. What is unproven is real hardware (see [What is not done](#what-is-not-done)).

## Table of contents

- [What runs on what](#what-runs-on-what)
- [Building and testing](#building-and-testing)
- [The target](#the-target)
- [Pages become Windows controls](#pages-become-windows-controls)
- [The runtime](#the-runtime)
- [Sizes and segments](#sizes-and-segments)
- [The ESP finding and how it was fixed](#the-esp-finding-and-how-it-was-fixed)
- [Where each test ran](#where-each-test-ran)
- [What is not done](#what-is-not-done)
- [Limits](#limits)

## What runs on what

| Piece | Where | Is |
|---|---|---|
| `crates/li-retro` | host, DOS, Windows 3.1, Windows 95 | The flow VM, the condition VM, the installer engine, the pack reader and the hand-written project decoder. The seams are `Gui` (a window that shows pages, shared with Win32) and `Shell` (shortcuts, uninstall entry, long names; D69, D73) |
| `crates/li-retro-build` | host | The compiler. A `win16` target compiles to screens that are widgets in reading order and no text cells |
| `retro/win16/src/bin/lisetup/` | Windows 3.1 | `SETUP.EXE`: `dosio` (INT 21h and KERNEL file calls), `heap` (the allocator), `plat` (the `Platform` and the `Shell`), `ui` (the wizard, a `Gui`, with the `/PLAY` and `/DUMP` test hooks) and `main` |
| `retro/win16/src/lib.rs`, `imports.rs`, `link.ld`, `tools/` | Windows 3.1, host | The task entry with its 32-bit stack, the exported-procedure wrapper, the import stubs, the linker script and the packer, copied from rust-win16 (`PROVENANCE.md`) |
| `crates/li-cli` (`win16`) | host | `build` for a `win16` target: the loose files, `zip` and `floppy-1440` |

Table 1. The pieces.

`retro/win16` is its own Cargo workspace (it needs `-Zbuild-std` and a custom target), so `make validate` never builds it.

## Building and testing

```sh
make win16                               # host tests, then check.sh, then the Wine install and uninstall
bash retro/win16/check.sh                # fmt, clippy, the packer's tests, the build with its size gate, `file`, ne_check.py
bash retro/win16/build.sh                # out/SETUP.EXE and out/SETUP.MAP (OUT_DIR and CARGO_TARGET_DIR move them)
python3 -m unittest discover -s retro/win16/tests -v
bash retro/win16/tools/e2e-wine.sh       # Lt. Fizz and Planboard installed and uninstalled under Wine
W31_TREE=/path/to/win311 retro/win16/tools/e2e-w31.sh --runs 20 lt-fizz planboard   # the same on Windows 3.11 in DOSBox-X
cargo test -p li-retro-build --test win16   # the host goldens: the pages, the step sequence and the receipt against DOS's
liminst build -t win16-x86 --win16-runtime retro/win16/out/SETUP.EXE
```

The tests, in the order they cost:

- **Packer** (`retro/win16/tests/`): header, segments, imports, exports, veneers, slots, the shadow stack and the refusals, from rust-win16, plus `tools/ne_check.py` on `SETUP.EXE` (no branch leaves its segment or lands inside an instruction).
- **Size gate** (`res/lisetup.segments.json`): the packer fails when a segment would pass its budget, 40 000 bytes of code for segment 1 and for each segment it fills automatically, which leaves about 18 KB in each for veneers and slots under the 64 KB limit. CI and `make win16` run it through `check.sh`.
- **Host goldens** (`li-retro-build/tests/win16.rs`): Lt. Fizz is compiled for `dos-x86` and for `win16-x86`; the DOS run is driven by keys and the Windows run by a scripted `ScriptGui`. The `(step)` sequence, the file trees and the receipts must agree (only the `target=` line and the game executable differ), the pages are compared with `tests/golden/win16/*.pages.txt`, and the decoder is compared with serde's.
- **Wine** (`e2e-wine.sh`): `SETUP.EXE /PLAY=PLAY.TXT /DUMP=...` presses the real controls (the script chooses the licence, then every page presses its default button) and writes the text of each page; the dump must equal the golden, the receipt's sizes must match the files, and `SETUP.EXE /U` must leave nothing.
- **Windows 3.11** (`e2e-w31.sh`, manual): the same on a fresh DOSBox-X for every session, a number of times. The comparison ignores the free space and what DOSBox-X's emulated Sound Blaster makes the sound page say.

`/PLAY=file` is the Win32 runtime's script (`step op args` with `set`, `toggle`, `choose` and `click`), `/DUMP=file` writes the page text and `/PAUSE=ms` holds each page for a screenshot; `/LOG=file` writes the steps, the heap's peak and the exit status as the run goes.

## The target

`win16-x86` with the `win16` renderer, as `examples/lt-fizz` already declared it and as SPEC §1 writes it (D68). `windows-x86_16` was considered and not used: the DOS target is `dos-x86` and the 32-bit one `windows-x86`, so the `os-arch` of a 16-bit Windows program that needs a 386 is `win16-x86`, and the renderer is `win16`.

Planboard has a `win16-x86` target too (`app16` and `docs16` components with 8.3 names; its other components name the targets they apply to), so both examples build for Windows 3.1.

## Pages become Windows controls

The compiler does not draw a Windows page: it keeps what the page says. A `win16` screen is the layout's nodes in reading order, with no text cells (`runs` is one blank run), and the runtime builds the controls from them.

| Node | Control |
|---|---|
| `dialog` `title` (the screen's `title`) | The window caption |
| `text` | A static control, wrapped to the window; `{placeholders}` are filled from the values each time they change |
| `text` with `source` | A read-only multi-line edit with a scroll bar |
| `radio` | Auto radio buttons in a group |
| `checklist` | Auto check boxes; a required item is ticked and disabled |
| `field`, `path` | A label and an edit control |
| `meter`, `progress` | A label with its percentage, and a gauge made of a white rectangle, a black fill moved along it and a frame (only on the page that copies) |
| `billboard` | A static control that shows the next slide each `interval_seconds` |
| `button` | A push button in a row at the bottom, right-aligned; the `default` one is the default push button; `hotkey` becomes the `&` access key; `enabled_when` enables and disables it |
| The side column (`region = "side"`), the hint bar, `image` | Left out |

Table 2. Nodes and controls.

The window is a top-level popup of a class whose default procedure is `DefDlgProc`, so Windows gives it a dialog's keyboard (Tab, arrows in a radio group, Enter for the default button, access keys) through `IsDialogMessage`, and a dialog's grey background. Esc is the DOS runtime's: back where there is a Back button, else cancel. Closing the window cancels.

Text roles (`heading`, `caption`, `summary`) ride in `Widget::Text.attr`, which has no other use without VGA colours; every role is drawn in the system font for now.

## The runtime

`SETUP` installs, `SETUP /U [folder]` removes, `SETUP /V` shows the version. Exit codes are the runner's: 0 done, 1 failed (rolled back), 2 bad arguments or a damaged pack, 3 cancelled.

- **Pack.** `SETUP.LPK` beside `SETUP.EXE`, found through `GetModuleFileName`, read with `_lopen`, `_llseek` and `_lread`. The compiler packs for Windows with 4 KiB chunks (`WIN16_CHUNK_LOG2`, 12) instead of DOS's 32 KiB, so the decoder's window, which the heap holds for the whole run, is an eighth the size. Every chunk's CRC-32 is checked before the bytes are written.
- **Files.** Created with `_lcreat` and `_lwrite`; directories, delete, attributes and free space by INT 21h (39h, 3Ah, 41h, 43h, 36h), each on the real 16-bit stack (see the ESP finding below). Free space stops at 2 GiB, as on DOS.
- **Receipt.** `INSTALL.LIR`, the DOS receipt unchanged apart from `target=win16-x86:win16`. The same `FIZZ.BAT` launcher is written and recorded as a `B` line.
- **Shell.** Windows 3.1's `Shell` says `ShortcutKind::GroupItem` and has no uninstall list; `add_group_item` (a DDE conversation with `PROGMAN`) is not written, so the install falls back to the batch launcher. The `G` receipt line and the uninstall call exist and are tested on the host.
- **Built-ins.** `detect-sound` reads `BLASTER` through `GetDOSEnvironment` and probes the ports with `in` and `out` (DOSBox-X's emulated Sound Blaster answers it); `check-disk-space` is the engine's.
- **Memory.** A 40 KiB heap in the data segment under a first-fit allocator, an 8 KiB stack, an 8 KiB local heap for the edit controls' text and a 64-entry shadow return stack; the data segment holds all of it in 64 KB. The heap's peak was 33 680 bytes for Lt. Fizz and 30 392 for Planboard.

## Sizes and segments

| | Bytes |
|---|---|
| Code, in four segments | 138 827 (34 842, 46 861, 45 355 and 11 769) |
| Call veneers, jump stubs and pointer slots | 1 169, 12 and 20 |
| Data (static data and the 40 KiB heap) | 47 396 |
| Stack, local heap | 8 192, 8 192 |
| `SETUP.EXE` | 206 179 |

Table 2. What the packer reports for `SETUP.EXE`.

The file is larger than the code because the packer keeps the data segment, with its 40 KiB of zero heap, in the file (Windows 3.11 does not zero what a file leaves out). The first whole build, before the hand-written decoder and the other cuts, was 121 725 bytes of code; with several segments the limit stopped being a blocker, and `res/lisetup.segments.json` keeps a budget so a segment cannot creep to 64 KB unnoticed.

The data segment is the limit now: the heap, the stack, the local heap and the shadow stack share 64 KB. Loading the pack is the heap's peak; with 16 KiB chunks it ran out on 3.11, and with 8 KiB it left 3 KB, so the pack uses 4 KiB.

## The ESP finding and how it was fixed

Early runs of the first, smaller programs (`ENGINE` and `UIDEMO`, one segment each) failed on Windows 3.11 in DOSBox-X: a General Protection Fault at a `rep movs` that copies a value built with `lea esi, [esp+x]`, a `Rust panic` box, a fault inside `KRNL386`, at a different place each run. Probe programs found the cause: **Windows 3.11 in 386 Enhanced mode leaves the high half of ESP dirty at every timer tick** (`8001xxxx`, about 20 a second), and code16 addresses the stack through ESP, so a stretch of computation of a few milliseconds between API calls faulted with a probability that grew with its length (about 1 run in 6 for a project decode; a long inflate would nearly always). A frame pointer, hooks on INT 1Ch and INT 08h and cleaning ESP after each call did not help. A 32-bit stack stopped the dirt (0 events against 130 in the same loop) but made `KRNL386` fault, because Windows ran on it.

The rust-win16 packer's runtime fixed it, as that project's probes had found at the same time: Rust runs on a 32-bit alias of the stack segment, made through DPMI by `_start`, and every import stub switches to the task's real 16-bit stack for its call into Windows and back, so Windows only ever runs on the stack it gave the task; `export_proc!` and the inline assembler in `dosio.rs` do the same, and a stub re-syncs the alias when Windows has moved the data segment. This runtime vendors that change. The same binary with the alias off failed about 1 run in 7 in their test, which matches the 1 in 6 seen here.

With it, 40 of 40 install-and-uninstall runs of the whole `SETUP.EXE` passed on Windows 3.11 (the heap's own exhaustion, found on the way, was a separate bug and is fixed by the 4 KiB chunks and the 40 KiB heap). Jump tables are left on, as the packer handles them.

## Where each test ran

| Environment | Ran | Result |
|---|---|---|
| Host (`cargo test`) | Step sequence, tree, receipt, uninstall, cancel, Program Manager seam and pages for Lt. Fizz and Planboard on `win16-x86`; the decoder against serde's; the CLI's outputs | Pass |
| Packer tests, `ne_check.py` | The vendored packer and the output's branches | Pass |
| Wine 11.21 (own prefix) | `e2e-wine.sh`: the whole `SETUP.EXE` through `/PLAY` for Lt. Fizz and Planboard, install and `/U` | Pass, pages equal the goldens |
| Windows for Workgroups 3.11 in DOSBox-X 2025.02.01 (`win /3`, Pentium, S3, a fresh session for each) | `e2e-w31.sh --runs 5` in four parallel workers: 20 install-and-uninstall runs of each example | 40 of 40 pass: exit 0, pages equal the goldens, receipt sizes match, folder gone. Screenshots of the [sound](images/win16-w31-setup-sound.png), [copying](images/win16-w31-setup-copying.png) and [done](images/win16-w31-setup-done.png) pages |
| Windows ME in VirtualBox | Not run: the VM was in use and then being debugged, and the rules say to leave it alone | Pending |
| Windows 3.1 Standard mode, a 286, real hardware | Not run | Untested |

Table 3. Where it ran.

## What is not done

- **Real hardware.** Everything on Windows 3.11 ran under DOSBox-X. The ESP fix is the same code rust-win16 verified there; whether a real 386, 486 or Pentium behaves as the emulator does is not shown, nor another emulator or a Windows 3.1 retail install, nor Windows ME.
- **A Program Manager group.** The `shortcut` operation still writes the DOS batch launcher. The seam is in place and tested on the host; the DDE conversation with `PROGMAN` (`[CreateGroup]`, `[AddItem]`) behind `add_group_item` is not written, and the atom functions' ordinals have not been checked against Windows 3.1.
- **Launch after setup, repair and update**, as on DOS.
- **Uninstall from the install folder.** `SETUP /U` runs from the media, as on DOS; a Windows 3.1 install keeps no uninstaller.
- **Browse for a folder**, icons, bold headings and any colour: the pages use the system font and the system colours.
- **Standard mode and a 286**: the code needs a 386.
- **A smaller file.** The heap's zeros are in `SETUP.EXE`; trimming them needs the program to clear its `.bss`, which the packer does not mark yet.

## Limits

| Thing | Limit |
|---|---|
| CPU | A 386 or later in 386 Enhanced mode (the code carries operand-size prefixes); DPMI is needed for the 32-bit stack, and without it the program runs as before and is exposed to the ticks. No Win32s |
| Code | 253 segments of 64 KB; the packer fails a segment over its budget |
| Data | One 64 KB segment: the heap, the stack, the local heap and the shadow stack share it |
| Names | 8.3, upper case, one install folder |
| Pack | 4 KiB chunks, volume 1 only |
| Command line | 120 characters |
| Free space shown | 2 GiB |
