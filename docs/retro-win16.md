# The Windows 3.1 runtime

`liminst build -t win16-x86` turns an installer project into `SETUP.LPK`, the Liminal Pack the Windows 3.1 runtime reads ([lpk-format.md](lpk-format.md)), and carries a Windows 3.1 `SETUP.EXE` when there is one. The runtime is Rust built for a 16-bit code segment and packed into an NE executable (D46, [win16-spike.md](win16-spike.md)), on the same portable core as the DOS runtime ([retro-dos.md](retro-dos.md)).

**Status.** The compiler, the page model, the shared flow VM hooks, the pack, the installer engine and the native pages all work and are tested. The whole `SETUP.EXE` does not link yet: its code is 111 211 bytes and a Win16 code segment holds 65 535. What is built instead are the two halves that each fit, `ENGINE.EXE` (install and uninstall from `SETUP.LPK`) and `UIDEMO.EXE` (the compiled project's pages in real Windows controls), both run on Windows 3.11, with an intermittent fault that is not about size ([the start-up fault](#the-start-up-fault-on-windows-311)). [The code limit](#the-code-limit-and-the-multi-segment-plan) says why, what was cut to get here and what the next step is.

## Table of contents

- [What runs on what](#what-runs-on-what)
- [Building and testing](#building-and-testing)
- [The target](#the-target)
- [Pages become Windows controls](#pages-become-windows-controls)
- [The runtime](#the-runtime)
- [Sizes](#sizes)
- [The code limit and the multi-segment plan](#the-code-limit-and-the-multi-segment-plan)
- [The start-up fault on Windows 3.11](#the-start-up-fault-on-windows-311)
- [Where each test ran](#where-each-test-ran)
- [What is not done](#what-is-not-done)
- [Limits](#limits)

## What runs on what

| Piece | Where | Is |
|---|---|---|
| `crates/li-retro` | host, DOS, Windows 3.1 | The flow VM, the condition VM, the installer engine, the pack reader and the hand-written project decoder. The seams are `Gui` (a window that shows pages, shared with Win32) and `Shell` (shortcuts, uninstall entry, long names; D69, D73) |
| `crates/li-retro-build` | host | The compiler. A `win16` target compiles to screens that are widgets in reading order and no text cells |
| `retro/win16/src/bin/lisetup/` | Windows 3.1 | `dosio` (INT 21h and KERNEL file calls), `heap` (the allocator), `plat` (the `Platform`), `ui` (the wizard window) and `main` |
| `retro/win16/src/bin/engine.rs` | Windows 3.1 | `ENGINE I dir` and `ENGINE U dir`: the engine with no pages |
| `retro/win16/src/bin/uidemo.rs` | Windows 3.1 | `UIDEMO`: the pages of the `install` flow in order, with no VM and no engine |
| `retro/win16/src/lib.rs`, `imports.rs` | Windows 3.1 | The task entry, the exported-procedure wrapper and the register-preserving import stubs from the spike |
| `retro/win16/tools/ne_pack.py` | host | The NE packer: code segment, data segment, imports, exports. It leaves the zero tail of the data segment out of the file (D71) |
| `crates/li-cli` (`win16`) | host | `build` for a `win16` target: the loose files, `zip` and `floppy-1440` |

Table 1. The pieces.

`retro/win16` is its own Cargo workspace (it needs `-Zbuild-std` and a custom target), so `make validate` never builds it.

## Building and testing

```sh
make win16                               # host tests, then check.sh, then the Wine install and uninstall
bash retro/win16/check.sh                # fmt, clippy, the packer's tests, the build, `file` on the result, the size report
bash retro/win16/build.sh                # out/ENGINE.EXE and out/UIDEMO.EXE (OUT_DIR and CARGO_TARGET_DIR move them)
bash retro/win16/build.sh --all          # and measure LISETUP, the whole runtime, which does not fit yet
python3 -m unittest discover -s retro/win16/tests -v
bash retro/win16/tools/e2e-wine.sh       # Lt. Fizz and Planboard installed and uninstalled under Wine
cargo test -p li-retro-build --test win16   # the host goldens: the page sequence and the receipt against DOS's
liminst build -t win16-x86 --win16-runtime retro/win16/out/SETUP.EXE   # when there is a SETUP.EXE to carry
```

The tests, in the order they cost:

- **Packer** (`retro/win16/tests/test_ne_pack.py`): header, segments, imports, exports, the zero tail, size limits.
- **Host goldens** (`li-retro-build/tests/win16.rs`): Lt. Fizz is compiled for `dos-x86` and for `win16-x86`; the DOS run is driven by keys and the Windows run by a scripted `ScriptGui` (`li_retro::sim`), which stands in for the person at the `Gui` seam. The sequence of `(step, screen)` pairs must be identical, the file trees must be identical, and the receipts must be identical except for the `target=` line and the game executable (a different binary for each target). The Windows receipt is also compared with `tests/golden/win16-receipt.txt`. The same file checks that the hand-written decoder agrees with serde's, and that the Windows uninstall removes everything.
- **Wine** (`tools/e2e-wine.sh`): both examples, installed and uninstalled by `ENGINE.EXE`.
- **Windows 3.11** (`tools/e2e-w31.sh`, manual): the same under DOSBox-X, then `UIDEMO.EXE` with screenshots. It needs a Windows 3.11 tree the repository cannot carry.

## The target

`win16-x86` with the `win16` renderer, as `examples/lt-fizz` already declared it and as SPEC §1 writes it (D68). `windows-x86_16` was considered and not used: the DOS target is `dos-x86` and the 32-bit one `windows-x86_64`, so the `os-arch` of a 16-bit Windows program that needs a 386 is `win16-x86`, and the renderer is `win16`.

Planboard now has a `win16-x86` target too (`app16` and `docs16` components with 8.3 names; the other three components are restricted to the other targets), so both examples build for Windows 3.1.

## Pages become Windows controls

The compiler does not draw a Windows page: it keeps what the page says. A `win16` screen is the layout's nodes in reading order, with no text cells (`runs` is one blank run), and the runtime builds the controls from them.

| Node | Control |
|---|---|
| `dialog` `title` | The window caption (a `Text` whose `attr` is the role `TITLE`; it is not drawn as a control) |
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

Text roles (`heading`, `caption`, `summary`, `title`) ride in `Widget::Text.attr`, which has no other use without VGA colours; every role is drawn in the system font for now.

## The runtime

`SETUP` installs, `SETUP /U [folder]` removes, `SETUP /V` shows the version; `/LOG=path` writes the pages shown and the exit status to a text file (the tests read it). Exit codes are the runner's: 0 done, 1 failed (rolled back), 2 bad arguments or a damaged pack, 3 cancelled.

- **Pack.** `SETUP.LPK` beside `SETUP.EXE`, found through `GetModuleFileName`, read with `_lopen`, `_llseek` and `_lread`. The compiler packs for Windows with 8 KiB chunks (`WIN16_CHUNK_LOG2`, 13) instead of DOS's 32 KiB, so the decoder's window is a quarter the size (see [Sizes](#sizes)). Every chunk's CRC-32 is checked before the bytes are written.
- **Files.** Created with `_lcreat` and `_lwrite`; directories, delete, attributes and free space by INT 21h (39h, 3Ah, 41h, 43h, 36h). Free space stops at 2 GiB, as on DOS.
- **Receipt.** `INSTALL.LIR`, the DOS receipt unchanged apart from `target=win16-x86:win16`. The same `FIZZ.BAT` launcher is written and recorded as a `B` line; it is a DOS-box launcher and harmless on Windows.
- **Built-ins.** `detect-sound` reads `BLASTER` through `GetDOSEnvironment` and probes the ports with `in` and `out`; `check-disk-space` is the engine's. Both are the DOS runtime's code.
- **Registers.** Every API call goes through the stubs that save EBX, ESI and EDI, clear the high half of ESP and reload ES (D46); the INT 21h wrapper does the same. The data segment's zero-initialised part and the unused stack are cleared by the program at start, because Windows 3.11 does not (D71).
- **Memory.** A 40 KiB heap in the data segment, a first-fit allocator over it, an 8 KiB stack and a local heap for the edit controls' text (4 KiB for `ENGINE`, 12 KiB for `UIDEMO`).

## Sizes

Measured by `tools/size_report.py` from the linked ELF (code is the `.text` section, data the `.data` section with the static heap in it). The limit is 65 536 bytes for code and 65 536 for the data segment with the stack and local heap added.

| Program | Code | Of 64 KB | Data (heap included) | Data + stack + local heap | File |
|---|---|---|---|---|---|
| `ENGINE.EXE` | 61 684 | 94 % | 44 344 | 56 632 | 65 276 |
| `UIDEMO.EXE` | 61 186 | 93 % | 44 480 | 64 760 | 65 052 |
| `LISETUP` (the whole runtime) | 97 265 | 148 % | 45 808 | not packed | not written |

Table 3. Segment sizes, in bytes.

The decoder's window is the pack's chunk size. Loading the pack is the heap's peak: with 16 KiB chunks it reached 34 408 bytes of a 36 KiB heap, which ran out on Windows 3.11 (a `Rust panic` box), and with 32 KiB it could not fit at all. The Windows pack therefore uses 8 KiB chunks and a 40 KiB heap: the peak is 26 216 bytes for Lt. Fizz and 22 440 for Planboard (`ENGINE.EXE` logs it when it finishes). The cost of small chunks is a few per cent on compressible files (`lpk-format.md`).

How the code came down: the first whole build was 121 725 bytes. Replacing serde's derive with a hand-written decoder (`li-retro/src/decode.rs`) took 11 KB, and writing the decoder as straight-line code that records a failure instead of returning early took another 13 KB (an early return from half a struct needs clean-up code for every field already read). Dropping `BTreeMap` for a vector (`VecMap`) and the sort in `selected_files` (the compiler already writes the files in order, and `Project::validate` now checks it) took the rest. The DOS runtime gets the same savings.

## The code limit and the multi-segment plan

LLVM's `code16` mode emits 32-bit near calls (`66 E8 rel32`) and 16-bit jumps. It cannot emit a far call, so everything the compiler places in the code segment must lie within 64 KB of everything that calls it. The whole runtime, the flow VM and the engine and the page code, is 97 KB after the cuts above. Its parts are:

| Part | Approximate code |
|---|---|
| Shared base: the decoder, `core`, `alloc`, `fmt`, the string routines | 24 KB |
| Native pages: `Page`, `Values`, the condition VM, `ui.rs` | 36 KB |
| Engine: the pack reader, inflate, install, uninstall, receipts | 37 KB |

`ENGINE.EXE` is the base and the engine, `UIDEMO.EXE` is the base and the pages, and `LISETUP` is both plus the flow VM (about 4 KB over their union less the shared base). More trimming can take perhaps 10 to 15 KB out of `LISETUP`; it cannot take 33 KB out, and the next feature would break it again. This is a limit of the toolchain D46 chose, and it is the first thing the spike's recommendation warned about ("the packer does not split code across segments").

The plan changed while this was written. The first idea was `SETUP.EXE` (flow VM and pages) plus `LIENGINE.DLL` (pack, install, uninstall). Measured against the merged code, `LISETUP` is 111 211 bytes (the Win32 work added shortcuts, the uninstaller and the registry entry to `install`), so the EXE side would still be about 80 KB with the engine moved out: a DLL alone does not make it fit. It was not built. Multi-segment code in the `rust-win16` packer (several NE code segments with far-call gates between them) is being built by another agent for this project and for Quill, and `retro/win16` will vendor that packer, with a provenance note, and relink the whole `SETUP.EXE` then. The other alternative remains Open Watcom C (D46's fallback), which does medium and large models but shares nothing with the portable core.

Until then, `liminst build -t win16-x86` writes `SETUP.LPK`, `README.TXT` and, when `--win16-runtime`, `LIMINST_WIN16_RUNTIME` or `retro/win16/out/SETUP.EXE` names a program, `SETUP.EXE`; without one it says so, and the `zip` and `floppy-1440` hold what there is.

## The start-up fault on Windows 3.11

`ENGINE.EXE` and `UIDEMO.EXE` sometimes died on Windows 3.11 in DOSBox-X: a General Protection Fault at a `rep movs` that copies a value built with `lea esi, [esp+x]`, a `Rust panic` box, or a fault inside `KRNL386`, at a different place from run to run. What was found, with the probe programs that found it (scratch, not committed):

- **The high half of ESP is dirtied at every timer tick.** A probe that loops in assembler, touching no stack memory, found ESP above 0xFFFF (always `8001xxxx`) 130 times in 6.3 s, about 20 a second, and cleaned it each time. Left alone it stays dirty until something rewrites ESP (the import stubs do, after each API call). code16 addresses the stack through ESP (`mov esi, [esp+0x14]` carries an address-size prefix and uses all 32 bits), so the first stack access after a tick faults. A stretch of computation of a few milliseconds between API calls (decoding the project, inflating a chunk) is hit with a probability that grows with its length, which fits the failure rates seen. A long inflate loop would almost always be hit.
- **Hooking the tick does not help.** A protected-mode handler on INT 1Ch and on INT 08h that cleaned ESP and chained on left the count at 133 and 129: the dirt is made after the handlers, where a task cannot reach.
- **A frame pointer does not help.** `mov ebp, esp` copies the dirt into EBP, and locals addressed through EBP fault the same way.
- **A 32-bit stack stopped the dirt but broke more.** Making SS an alias of DS with the big bit set (`AllocSelector`, then DPMI `0009h` with access rights `40F3h`, then `mov ss, ax` and `movzx esp, sp`) gave 0 dirty events in the same loop, and installs ran. Uninstalls then faulted inside `KRNL386` in 3 of 3 runs, and a probe moved the fault from place to place by changing the code's layout, so the dirt is probably still being made somewhere KERNEL's own stack operations see it. It was not adopted; the DOSBox-X debugger is the next tool.
- **Jump tables** were present (six indirect `jmp [reg*4+table]` in `ENGINE`), as in the Quill probes. They are off now (`-C jump-tables=no`), at a cost of about 1 KB of code.
- **The stubs save and restore all four of EBX, ESI, EDI and EBP** (each is pushed and popped at full width around every API call and in `export_proc!`), ES is reloaded after each call, and the runtime uses neither FS nor GS.
- **Cleared memory.** Windows 3.11 does not zero the part of a segment past its file length or the unused stack, so the entry code clears both (D71). That took the failure rate from about 2 in 5 to about 1 in 6 on the earlier, smaller build; the final build, which is bigger, fails more often (4 of 10).

So the Rust `code16` approach has a hard problem on Windows 3.x in 386 Enhanced mode that is not about size: any code that uses ESP-relative addresses is exposed to a clobbered ESP at every tick. The options, in the order to try: find out where the dirt is made with a debugger and whether a 32-bit stack plus cleaning around KERNEL's own stack use closes it; generate 16-bit addressing (`[bp+x]`, which ignores the high half) for stack accesses in the packer by rewriting the code; run the computation in short stretches that end in an API call; or the Open Watcom C fallback, whose 16-bit code never uses ESP. Another agent is building multi-segment support in the `rust-win16` packer, and this finding belongs in the same discussion.

## Where each test ran

| Environment | Ran | Result |
|---|---|---|
| Host (`cargo test`) | Page sequence, tree, receipt and uninstall for Lt. Fizz on `win16-x86` against `dos-x86`; the decoder against serde's; the CLI's outputs | Pass |
| Wine 11.21 (own prefix) | `e2e-wine.sh`: `ENGINE I` and `ENGINE U` for Lt. Fizz and Planboard; `UIDEMO` through the welcome and licence pages | Pass |
| Windows for Workgroups 3.11 in DOSBox-X 2025.02.01 (`win /3`, Pentium, S3), a private copy of the tree | `ENGINE I` and `ENGINE U` for Lt. Fizz (files byte for byte, receipt as the golden but for the sound INI) and for Planboard; `UIDEMO` through the welcome, licence and folder pages with typed keys | Passes, but see the intermittent failure below; screenshots: [welcome](images/win16-w31-uidemo-welcome.png), [licence](images/win16-w31-uidemo-licence.png), [folder](images/win16-w31-uidemo-folder.png) |
| Windows ME in VirtualBox | Not run: the VM was in use by the person (`VBoxManage list runningvms` showed it running), so none of the VM steps were started | Pending |
| Windows 3.1 Standard mode, a real 286, real hardware | Not run | Untested, as in the spike |

Table 4. Where it ran.

## What is not done

- **The whole `SETUP.EXE`.** It waits for multi-segment code in the packer (above). `ENGINE.EXE` and `UIDEMO.EXE` are the two halves.
- **A reliable start on Windows 3.11.** With the merged code and jump tables off, 4 of the last 10 `ENGINE` runs on 3.11 (5 installs and 5 uninstalls) faulted, and a long computation would nearly always: the high half of ESP is dirtied at every timer tick (see [the start-up fault](#the-start-up-fault-on-windows-311) for the probes, what was ruled out and what is next).
- **A Program Manager group.** The `shortcut` operation still writes the DOS batch launcher. The seam is in place: Windows 3.1's `Shell` says `ShortcutKind::GroupItem`, the install calls `add_group_item` and records a `G group|item` receipt line (tested on the host with a pretend Program Manager), and an uninstall calls `remove_group_item`. What is missing is the DDE conversation with `PROGMAN` (`[CreateGroup]`, `[AddItem]`) behind `add_group_item`: there is no stub for it, and the atom functions' ordinals have not been checked against Windows 3.1.
- **Launch after setup, repair and update**, as on DOS.
- **Uninstall from the install folder.** `SETUP /U` runs from the media, as on DOS.
- **Browse for a folder**, icons, bold headings and any colour: the pages use the system font and the system colours.
- **Windows ME**, Standard mode and a second Windows 3.1 install: see Table 4.

## Limits

| Thing | Limit |
|---|---|
| CPU | A 386 or later in 386 Enhanced mode (the code carries operand-size prefixes). No Win32s |
| Code | One 64 KB segment; the packer refuses more |
| Data | One 64 KB segment: the static heap, the stack and the local heap share it |
| Names | 8.3, upper case, one install folder |
| Pack | 16 KiB chunks, volume 1 only |
| Free space shown | 2 GiB |
