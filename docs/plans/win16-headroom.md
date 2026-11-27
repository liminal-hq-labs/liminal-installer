# Plan: room in the Windows 3.1 runtime's data segment and code segments

The Windows 3.1 `SETUP.EXE` (`retro/win16`) has 12 bytes of its 64 KB automatic data segment left once WP-SYS-1 (the Windows and system folders) is in, and every installer feature still to come (the media model and `COMMON.LPK`, optional components, repair and update) lands in `li-retro` and the Win16 shell. This plan measures where the segment goes, ranks the ways to win back at least 4 KB, rebalances the code segments, and orders the work as `WP-W16-1` to `WP-W16-6`.

**Markers.** [V] means measured or checked for this plan; [U] means not, and the implementer must check it. Measured on `feat/system-folders-runtime` at `06a7f85` (pull request #65, the WP-SYS-1 code; `origin/main` at `42b4866` for the "before" figures), Rust 1.98.0, with `retro/win16/build.sh`, an `ld.lld -Map` of the same link (byte-identical ELF), `llvm-nm`, `llvm-readobj --stack-sizes` (`-Z emit-stack-sizes`), the packer's own placement run from Python, and a throwaway probe build (stack paint, local heap samples, heap block census) run under Wine and on Windows for Workgroups 3.11 in DOSBox-X. The probe was never committed.

## Table of contents

- [1. Where the 64 KB goes](#1-where-the-64-kb-goes)
- [2. What Quill learned](#2-what-quill-learned)
- [3. What the packer and Windows offer](#3-what-the-packer-and-windows-offer)
- [4. Ranked options](#4-ranked-options)
- [5. Budgets and the headroom rule](#5-budgets-and-the-headroom-rule)
- [6. Owner decisions](#6-owner-decisions)
- [7. Risks](#7-risks)
- [8. Work packages](#8-work-packages)
- [9. Sonnet briefs](#9-sonnet-briefs)

## 1. Where the 64 KB goes

### 1.1 The segment

A Windows 3.x program has one automatic data segment (DS = SS) of at most 64 KB. The packer lays it out as the linked `.data` (read-only data, data and zero data, in that order, from `link.ld`), then the shadow return stack; Windows adds the stack and the local heap from the NE header (`build.sh` asks for 8 KiB of each). The packer refuses a build whose data, stack and heap pass 65,536 bytes.

| Bytes | Share | What | Source [V] |
|---:|---:|---|---|
| 40,976 | 62.5% | The Rust heap: `lisetup::heap::HEAP`, a 40 KiB arena and 16 bytes of state | `.bss`, `heap.rs` `HEAP_SIZE` |
| 8,192 | 12.5% | Stack | NE header, `build.sh` |
| 8,192 | 12.5% | Windows' local heap (the text of every edit control) | NE header, `build.sh` |
| 7,319 | 11.2% | Read-only data | `.rodata*`, 263 input sections; see 1.2 |
| 529 | 0.8% | Other zero data (13 statics) | `.bss`; see 1.3 |
| 260 | 0.4% | Shadow return stack (a 4-byte pointer and 64 entries) | the packer, `--shadow 64` |
| 26 | | `w16::RESERVED` (16, Windows writes there) and the stack selectors and alias state (10) | `.data` |
| 30 | | Alignment padding | |
| **65,524** | | **of 65,536: 12 free** | `data 49140 + stack 8192 + heap 8192` |

Table 1. The automatic data segment of `SETUP.EXE` at `06a7f85`. On `main` the data is 48,972 (180 free); WP-SYS-1 takes 168. `docs/retro-win16.md` still says 628 free and 48,524 of data, which was already stale on `main`.

So "12 bytes free" is the static figure: the next static, string or table that LTO keeps breaks the build. The run-time room is elsewhere, in the slack of the two heaps and the stack (1.4 to 1.6).

### 1.2 Read-only data

| Bytes | Pieces | What [V] |
|---:|---:|---|
| 4,711 | 185 | Text: error and log messages, `progman` DDE command templates, page and switch names, `core::fmt` templates (`.rodata.str1.1` alone is 1,417) |
| 640 | 2 | `li_retro::cp437`'s `HIGH: [char; 128]` (512) and `LOW: [char; 32]` (128): four bytes per entry where two would do |
| 724 | 20 | Jump tables (`match` statements compiled to `jmp [table]`; the table is in DS and holds code offsets) |
| 256 | 1 | `core::unicode::white_space::WHITESPACE_MAP`, from `char::is_whitespace` (`split_whitespace`, `trim`) |
| 256 | 1 | `core::str`'s UTF-8 width table, from `String::from_utf8` and `from_utf8_lossy` |
| 200 | 1 | `core::fmt`'s two-digit table, from integer formatting |
| 149 | 12 | Trait-object vtables (`dyn Gui`, `dyn Shell`, `dyn Platform` and friends) |
| 383 | 31 | Other lookup tables and constants (`li_lpk::Error::message` 84, `lisetup::ui::ansi` 80, `li_retro::expr::holds` 76, LLVM constant pools) |
| **7,319** | **263** | |

Table 2. Read-only data by kind (a section with code relocations over half its words is a table of code pointers; one 80% printable is text).

### 1.3 Other statics

`lisetup::dde::INFO` 160 (the group listing being searched), `lisetup::dosio::PATH` 144 (a path buffer for INT 21h), `lisetup::dde::NAME` 96, the window classes and message buffers of `ui` and `dde` (26, 26, 24, 18, 18), the three `EV_*` event words (9), `CMDLINE` and `HINST` (8). The window procedures may not allocate (the heap is not reentrant), which is why `dde`'s are statics. `w16::LOG` (3 KB in Quill's runtime) is not linked: `lisetup` keeps its log in a heap `String`.

### 1.4 The Rust heap

The arena is a first-fit free list (`heap.rs`). Each block costs `round_up(max(size, 8) + 8 + max(align, 8), 8)`: an 8-byte header and, always, a whole alignment's worth of padding, so a 1-byte allocation takes 24 bytes. The probe counted the blocks live at the peak:

| Run | Peak, Windows 3.11 | Peak, Wine | Blocks live at the peak | Bytes asked for at the peak | Allocator overhead |
|---|---:|---:|---:|---:|---:|
| Lt. Fizz install | 36,720 | 36,112 | 493 | 26,460 | 9,652 (19.6 per block) |
| Planboard install | 32,704 | 32,152 | 413 | 24,111 | 8,041 |
| Planboard uninstall | 29,368 | 29,352 | 409 | 21,328 | 8,024 |
| Quill installer (earlier build, cached logs) | 35,832 | | | | |

Table 3. Heap peaks of the 40,960-byte arena [V]. The block census is from Wine; the peaks agree with Windows 3.11 within 2%. The Quill figure is from `~/.cache/quill16-installer-build` logs of an earlier runtime, not this build.

So a quarter of the heap at its peak is the allocator's own bookkeeping. A 4-byte header with 4-byte granularity, padding only when a type asks for more than 4 (on this target `u64` is 4-aligned and only `u128` is 16), would cost about 5.5 bytes a block: about 2.7 KB at the Lt. Fizz peak instead of 9.7 KB [U: the census after the change is the proof].

The margin today is 4,240 bytes on Windows 3.11 for Lt. Fizz (40,960 less 36,720).

### 1.5 The stack

The probe filled the stack from Windows' `pStackTop` (DS offset `0x0A`) to 256 bytes below the stack pointer at the start of `w16_main`, and at the end counted the bytes never written:

| | Windows 3.11 | Wine |
|---|---:|---:|
| Stack (`pStackTop` to `pStackBottom`) | 8,042 | 8,042 |
| Deepest use | 6,440 | 6,484 |
| Never touched | 1,586 | 1,542 |

Table 4. Stack depth, the same in the install and the uninstall of both examples [V].

The depth is the same in every run because the deepest path is start-up, not a page: the largest frames (`llvm-readobj --stack-sizes`) are `w16_main` 2,500 bytes (LTO has inlined the whole of `app::run` into it, a 15,825-byte unit of code, so its frame lives for the whole run), `li_retro::app::load_project` 1,856 and `li_retro::pack::Pack::next` 1,708 (the inflater's tables, on the stack because `li-lpk`'s reader allocates nothing): 6,064 of the 6,440. Every page then runs above a 2.5 KB frame it does not need.

A stack that is too small fails silently: Quill's ran into its statics and the program carried on wrong (section 2).

### 1.6 The local heap

Windows 3.x keeps the text of an edit control in the program's local heap. The runtime makes two kinds: the one-line `Field` and the multi-line read-only `Doc` (the licence), which holds the whole document. On Windows 3.11 the largest free block never fell below 7,874 of 8,192 bytes in the Planboard install (a 141-byte licence) [V, sampled only when the message pump was entered, so coarse]. Wine's edit control keeps its text elsewhere and always showed 8,116.

So most of the local heap is idle with the examples, but it is also the ceiling on a licence: a `Doc` longer than about 7 KB cannot be shown on Windows 3.x today [U: not tried]. Quill's full licence (`installer/LICENCE-FULL.TXT`) is 13,000 bytes; its installer uses the 230-byte `LICENCE.TXT`.

### 1.7 Code segments

| Segment | Code placed (budget 40,000) | Size with veneers and trampolines |
|---|---:|---:|
| 1 (fixed: entry, exports, import stubs) | 39,603 | 47,076 |
| 3 | 39,966 | 47,620 |
| 4 | 39,432 | 46,582 |
| 5 | 30,982 | 35,870 |
| **All** | **149,983** | **177,148** |

Table 5. Code at `06a7f85` [V]. On `main` it is 171,255 bytes in the same four segments (1: 35,780; 3: 47,633; 4: 46,903; 5: 40,939). The figures in the request for this plan (segments 3, 4 and 5 at 47,868, 47,538 and 44,127, and a sixth segment) did not reproduce here; both builds have four code segments.

- **Veneers are 14% of the code.** 1,565 call sites and 35 jumps cross a segment; each call costs 11 bytes in the caller and a 5-byte return trampoline in the callee, about 25.2 KB in all.
- **A few small functions take most of them.** `<RawVec<u8> as Drop>::drop` (21 bytes) is called across segments from 310 sites, `[u8]::to_vec_in` (108 bytes) from 113, `alloc::fmt::format` (27 bytes) from 63. Copying a function into every segment that calls it, where that is cheaper than its veneers, would save about 6.4 KB (20 functions of 32 bytes or less) to 9.3 KB (30 of 128 bytes or less) [V, computed from the packer's own placement; the copies' own outgoing calls counted as veneers].
- **Grouping by module does not help.** Placing `alloc` and `core` in segment 1 and `install`, `app` and the shell in a segment each gave 178,108 bytes in five segments, 960 more than the address-order fill [V].
- **Sharing veneers** (every cross call through the 9-byte dynamic stub, one per caller segment and target: 448 pairs, about 4 KB) would save about 21 KB of code but put every cross call on the shadow return stack, which is data, at a depth nobody has measured.
- **The heap is in the file.** The packer writes the whole data segment, so `SETUP.EXE` (253,184 bytes) carries the 40,976 zero bytes of the arena, and so does every `UNINST.EXE`. An NE data segment may be shorter in the file than in memory (the segment table's length and minimum allocation are separate fields), which would take about 41 KB off every Windows 3.1 disk.

## 2. What Quill learned

Quill's Win16 build (`~/source/experiments/quill16`, `docs/win16-data-segment.md`, decisions D200, W16-2, W16-3, W16-60) met the same wall with a word processor. What carries over:

- **The packer's limit is not the loader's.** A segment exactly full (65,536) was refused by Program Manager with "Insufficient memory to run this application"; 32 bytes free ran. Here, the 12-free build at `06a7f85` installed and uninstalled Lt. Fizz on Windows 3.11 [V, one run], so the loader needs 12 bytes or fewer. A gate should keep at least 64.
- **A short stack is silent.** Quill's stack ran 3 KB into its statics and the program stopped early with no fault. Quill now writes `STACK.LOG` from a painted stack and `check_stack_log.py` fails a run with less than 2,048 bytes of margin.
- **The main frame is the cheap win.** Inlined start-up work put 2.8 KB of buffers in `w16_main`'s frame for the whole run; moving each start-up step into an `#[inline(never)]` function took it to 512.
- **Bulk goes to far memory, by copy.** Rust references are near, so nothing in a `GlobalAlloc` block can be borrowed. Quill's `far.rs` (`FarBlock`: a moveable block locked once, whose selector stays put, read and written through `far_read` and `far_write`) holds the face list, the play-back staging buffer and the pictures; its `Store` holds the document in 32 KB chunks. The portable core has no `alloc` at all (D8) so every allocation is visible.
- **Text to string resources, relief before spending.** Quill moves hints and tips to `RT_STRING` resources loaded with `LoadString` (about 2.1 KB, WP-ARCH-9) and its rule W16-60 says relief lands before the features that spend it, and nothing merges with less than 1,024 bytes free.
- **Scan, don't trust.** LLVM emits `bswap` (a 486 instruction) for `u32::from_be_bytes` even for an `i386` target; Quill's `check_cpu.py` fails the build on it. The installer has no such scan [U: check `SETUP.EXE` for `bswap`, `cmpxchg`, `xadd`, `cpuid` and `cmov` in WP-W16-2].

What does not carry over: the installer's core (`li-retro`) uses `alloc` throughout and is shared with DOS, so the Quill pattern of "no allocation in the core" is not available; the heap is the thing to make cheaper.

## 3. What the packer and Windows offer

| Option | Can it help here? |
|---|---|
| Stack and heap sizes (`--stack`, `--heap` in `build.sh`) | Yes, but only by moving bytes between the stack, the local heap and the statics; the sum stays 64 KB |
| A second data segment | No for Rust data: code16 Rust reaches data only through DS, so a near reference cannot point elsewhere. A second segment is only far memory, which `GlobalAlloc` already gives without a packer change |
| Read-only data in a code segment | No: LLVM addresses every constant through DS; there is no CS-relative data for code16 |
| String resources (`RT_STRING`, `LoadString`) | Yes, for text the shell shows or logs: the packer already writes string resources from `res/lisetup.json`. Each use copies into a stack buffer; the text used by `li-retro` (shared with DOS) would need an id scheme |
| `GlobalAlloc` far memory | Yes, for buffers reached by copy (Quill's `FarBlock`); not for anything Rust borrows |
| `LocalAlloc` | Yes: it returns near DS offsets, so the Rust allocator can take its blocks from Windows' local heap and share one pool with the edit controls (option 5) |
| Shadow stack size (`--shadow`) | 64 entries (260 bytes) now; a measured maximum could take 128 [U], not worth a package |
| `-C jump-tables=no` | Yes: the jump tables and LLVM's switch lookup tables leave DS and become compare chains in code: data −808, code +773 [V, packed and checked with `ne_check.py`, not run] |
| Segment flags (load on call, discardable) | Memory on small machines, not DS |

Table 6. The tools, and whether each gives data room.

## 4. Ranked options

Ranked by bytes won for the risk and the work. "Data room" is what the packer's limit gains; "run-time room" is margin inside the heap or the stack.

| Rank | Option | Data room | Run-time room | Cost and risk | Package |
|---:|---|---:|---:|---|---|
| 1 | **A leaner allocator, then a smaller arena.** 4-byte headers and granularity, padding only for alignments over 4, `realloc` that grows into a free neighbour; then `HEAP_SIZE` from 40 to 36 KiB | **+4,096** | Heap peak about −7 KB, so the margin rises from 4.2 KB to about 7 KB even after the cut | Low: one file, testable on the host with `rustc --test`; proved by the census on 3.11 | WP-W16-1 |
| 2 | **A shallower stack.** Start-up out of `w16_main` (`#[inline(never)]`), the project load and the inflater's tables off the deepest path; then the stack from 8 to 6 KiB | **+2,048** [U] | Same margin, measured | Medium: a short stack is silent, so the stack gate (WP-W16-2) comes first | WP-W16-3 |
| 3 | **Jump tables off** (`-C jump-tables=no` in `.cargo/config.toml`) | +808 [V] | | Low; code +773 [V] | WP-W16-4 |
| 4 | **Small tables and statics.** CP437 tables as `u16` (+320), ASCII whitespace in place of `char::is_whitespace` (+256 when no use is left), `dosio::PATH` on the stack (+144) | +720 | | Low; `cp437.rs` is shared with DOS, whose host tests cover it | WP-W16-4 |
| 5 | **One pool.** The Rust allocator takes its blocks from `LocalAlloc`; the arena leaves the statics, the NE heap becomes everything left | No fixed figure: the packer's data falls to about 8 KB, and the idle local heap (about 7.8 KB at the measured points) and the heap's slack become one pool | Lifts the 7 KB licence ceiling | Medium to high: every allocation is a far call [U: install time], a new failure mode (out of memory at run time instead of a build error) | WP-W16-6, deferred |
| 6 | **Text to string resources** (lisetup's own log and message text first) | +1,000 to +2,000 [U] | | High: every message site changes; `li-retro`'s text needs an id scheme | Not planned |
| 7 | **Bulk buffers to far memory** (the pack's 4 KiB window and directory) | | Heap −4 KB or more [U] | High: `li-retro` and `li-lpk` API changes | Not planned |
| 8 | **The local heap from 8 to 4 KiB** alone | +4,096 | | Caps a licence at about 3.5 KB | Not recommended |

Table 7. Data options. Ranks 1 and 2 alone give about 6 KB of data room, 1 alone the 4 KB asked for; 1 to 4 give about 7.6 KB.

Code:

| Rank | Option | Code | Package |
|---:|---|---:|---|
| 1 | Copy small hot functions into each calling segment (packer) | −6 to −9 KB | WP-W16-5 |
| 2 | Leave the data segment's zero tail out of the file (packer) | File −41 KB, code 0 | WP-W16-5 |
| 3 | Split `w16_main` (from WP-W16-3), which also gives the packer finer units | Small [U] | WP-W16-3 |
| 4 | A size report that names each segment's largest units and the cross-call count | 0 | WP-W16-2 |
| — | Module placement in `lisetup.segments.json` | +960 measured | Rejected |
| — | Shared veneers through the dynamic stub | −21 KB, but data on the shadow stack | Rejected while data is the scarce thing |

Table 8. Code options. The 40,000-byte auto budget stays: the packer opens another segment when one fills, and a sixth segment costs veneers, not correctness.

## 5. Budgets and the headroom rule

| What | Today (`06a7f85`) | After WP-W16-1 | After WP-W16-1 to WP-W16-4 |
|---|---:|---:|---:|
| Data segment free (static) | 12 | 4,108 | about 7,700 |
| Heap margin, Lt. Fizz on 3.11 | 4,240 | about 7,000 [U] | about 7,000 [U] |
| Stack margin on 3.11 | 1,586 | 1,586 | at least 2,048 (the gate) |
| Code | 177,148 | +300 at most | after WP-W16-5, about −7 KB [U] |

Table 9. Budgets.

The rule, after Quill's W16-60, for `docs/decisions.md` (WP-W16-2 pastes it with the gates that enforce it):

> | D*n* | The Windows 3.1 runtime keeps measured headroom | **Decided.** `retro/win16/check.sh` fails when the data segment has fewer than 64 bytes free (the loader needs some; Quill's refused a full segment); `e2e-wine.sh` and `e2e-w31.sh` fail when the stack margin is under 2,048 bytes or the heap margin under 4,096, from the `stack` and `heap` lines `/LOG` writes; a package that spends data, heap or stack states the bytes in its pull request, relief lands before the features that spend it, and nothing merges into `retro/win16` or `li-retro` with less than 1,024 bytes of the segment free | Measuring when the segment fills, which is what left WP-SYS-4 12 bytes | Every installer feature to come runs in `li-retro` on Windows 3.x, and a stack or heap that runs short fails silently or at a customer's machine |

## 6. Owner decisions

| # | Question | Default taken | Where to swap |
|---:|---|---|---|
| 1 | Spend the allocator's saving on data room (cut the arena by 4 KiB) or keep it all as heap margin for `COMMON.LPK`? | Cut 4 KiB: the heap margin still rises from 4.2 to about 7 KB | `HEAP_SIZE` in `heap.rs` (WP-W16-1) |
| 2 | Does WP-SYS-4 (the Win16 system folders) wait for WP-W16-1? | Yes: it starts with 12 bytes | The order in section 8 |
| 3 | Packer changes upstream in `rust-win16` first, then a refresh by `PROVENANCE.md`, or a local patch? | Upstream first (the provenance rule) | WP-W16-5 |
| 4 | One pool (`LocalAlloc`) now or later? | Later, when `COMMON.LPK` or a licence over 6 KB needs it | WP-W16-6 |
| 5 | The stack margin the gate asks for | 2,048 bytes, as Quill | `e2e-*.sh` (WP-W16-2) |
| 6 | Until one pool lands, is a Win16 licence over 6 KB an error, a warning or nothing? | A `liminst check` warning on `win16` targets | WP-W16-2; drop the warning in WP-W16-6 |

Table 10. Owner decisions.

## 7. Risks

- **The loader's slack [V for 12 bytes, once].** `06a7f85` loads on Windows 3.11 with 12 bytes free; Quill's refused at 0. Until WP-W16-1 lands, any change that brings the segment to 0 to 11 free builds and may not start. WP-W16-2's gate keeps 64.
- **A heap census from Wine.** Wine's peaks agree with 3.11's within 2%, but the allocator change must be proved by the census on 3.11, and Quill's installer (more files, so a longer pack directory) must be run too [U].
- **Fragmentation.** Smaller headers change the layout of the free list; a pattern that fits today could fragment differently. The host test runs random sequences against a model; the e2e runs are the proof.
- **Stack depth on pages under real Windows.** The deepest path today is start-up; once it is cut, a page (a dialog message nested inside `DialogBox` or the DDE conversation, which run on the program's own stack in Windows 3.11) may become the deepest. The gate measures the whole run, so it catches it, but only for the scripted pages.
- **Jump tables and code size.** `-C jump-tables=no` grows code by 773 bytes [V]; with later growth a segment may open. It also removes the class of fault Quill met (W16-7: a jump table indexed by a bad state byte left the segment).
- **Upstream packer.** WP-W16-5 depends on `rust-win16` accepting the change; Quill uses the same packer, so it must keep Quill's 30-run Windows 3.11 loop green.
- **A flaky Wine uninstall [V].** In four `e2e-wine.sh` runs of `06a7f85`, one Planboard uninstall left `C:\PLANBOAR` and the `pbd=` line behind; the next three passed. Not caused by anything here; builders should run it twice and report a repeat.
- **The one-pool change moves the failure.** With one pool, a static that grows no longer fails the build: the pool shrinks and a later allocation may fail on a customer's machine. It needs the gates of WP-W16-2 first.
- **The 8-byte alignment assumption.** Something in `li-retro` or `alloc` may rely on 8-byte-aligned allocations without asking [U]; `Layout::align` is the contract, and the host test covers 1, 2, 4, 8 and 16.

## 8. Work packages

Build order: **WP-W16-1** first (before WP-SYS-4, which then rebases on it); then **WP-W16-2** and **WP-W16-4** in parallel; then **WP-W16-3**. **WP-W16-5** runs in `rust-win16` at any time and its refresh lands after WP-W16-4. **WP-W16-6** waits for an owner go. At most three builders at once.

### WP-W16-1 — A leaner allocator, its census, and a 36 KiB arena

- **Branch:** `perf/win16-heap`, from `main`.
- **Finish line:** `heap.rs` uses 4-byte headers and granularity, pads only for alignments over 4 and grows or shrinks in place in `realloc`; a host test (`rustc --test` on `heap.rs`, run by `check.sh`) passes; `/LOG` gains `heap blocks N at the peak, A bytes asked, M at most` after the unchanged `heap peak P of H bytes` line; with `HEAP_SIZE` at 36 KiB, Lt. Fizz and Planboard install and uninstall on Windows 3.11 (three runs each, and one with `KEPT=1`) with at least 4,096 bytes of heap margin, and the data segment has 4,108 bytes free.
- **Files owned:** `retro/win16/src/bin/lisetup/heap.rs`, `retro/win16/src/bin/lisetup/plat.rs` (the `finish` log lines only), `retro/win16/check.sh` (the host test step), `docs/retro-win16.md` (the Memory bullet and the stale 628 and 48,524 figures in the Program Manager section).
- **Must not touch:** the vendored files in `PROVENANCE.md` (`tools/ne_pack.py` and the rest, `src/lib.rs`, `link.ld`), `build.sh`'s stack and heap sizes, `res/lisetup.segments.json`, `tools/imports.tbl`, `crates/`, the e2e scripts' checks.
- **Tests:** `make win16` under a private Xvfb; `retro/win16/tools/e2e-w31.sh --runs 3 lt-fizz planboard` and `KEPT=1 ... --runs 1`; the host test.
- **Budgets:** data −4,096 (to 45,044); code +300 at most; heap peaks reported for every run.

### WP-W16-2 — Measure and gate: stack, local heap, size report, headroom rule

- **Branch:** `test/win16-gates`, on WP-W16-1.
- **Finish line:** `/LOG` writes `stack used N of M bytes` (painted from `pStackTop` at the start of `w16_main`, counted in `finish`) and `local heap largest free N bytes` (the least `LocalCompact(0)` seen at each page); `e2e-wine.sh` and `e2e-w31.sh` fail on a stack margin under 2,048 or a heap margin under 4,096; `check.sh` fails with fewer than 64 bytes of the segment free and prints a size report (`tools/size_report.py`: data by group as in table 1, the 20 largest statics and read-only pieces, each segment's placed bytes and veneers, the largest frames from `-Z emit-stack-sizes` when asked) and a CPU scan of every segment for 486 and later instructions (`bswap`, `cmpxchg`, `xadd`, `cpuid`, `rdtsc`, `cmov`); `liminst check` warns on a `win16` target whose licence `Doc` is over 6 KB; the headroom rule from section 5 is in `docs/decisions.md`.
- **Files owned:** `retro/win16/src/bin/lisetup/{main.rs,plat.rs,ui.rs}` (the probe calls only), a new `retro/win16/src/bin/lisetup/probe.rs`, `retro/win16/tools/imports.tbl` and the regenerated `src/imports.rs` (`KERNEL 13 LocalCompact w w`, ordinal read from 3.11's `KRNL386.EXE` with `tools/ne_dump.py`), `retro/win16/tools/{size_report.py,cpu_scan.py,e2e-wine.sh,e2e-w31.sh}`, `retro/win16/check.sh`, `crates/li-engine/src/check.rs` and its tests (the warning), SPEC §4 (the warning's code), `docs/decisions.md`, `docs/retro-win16.md`.
- **Must not touch:** `heap.rs` beyond reading its census, the vendored packer, `li-retro`, `cp437.rs`, `.cargo/config.toml` (WP-W16-4's).
- **Tests:** `make win16`; `e2e-w31.sh --runs 3`; a deliberately small stack (`build.sh` with 0x1A00, not committed) must fail the gate on 3.11; `cargo nextest run -p li-engine --locked`.
- **Budgets:** data +200 at most (say the figure); code +1 KB at most.

### WP-W16-3 — A shallower stack, then 6 KiB

- **Branch:** `perf/win16-stack`, on WP-W16-2 and WP-W16-4.
- **Finish line:** `w16_main`'s frame is under 512 bytes (the command line, the relaunch and the platform set-up each in an `#[inline(never)]` function, `app::run` called, not inlined); the start-up path's frames are reported before and after; the stack in `build.sh` is the smallest multiple of 512 that keeps 2,048 bytes of margin on every 3.11 run, 6 KiB or less [U]; the data segment gains what the stack gives.
- **Files owned:** `retro/win16/src/bin/lisetup/main.rs`, `retro/win16/build.sh` (the `lisetup` stack size), `crates/li-retro/src/app.rs` (`#[inline(never)]` on `run` and `load_project` only, if needed), `docs/retro-win16.md`.
- **Must not touch:** `li-lpk` (the inflater's stack tables are a separate decision: report their size), the packer, `heap.rs`.
- **Tests:** `make win16`; `e2e-w31.sh --runs 3` and `KEPT=1`; `cargo nextest run -p li-retro --locked`; the stack gate.
- **Budgets:** data +2,048 [U]; code ±1 KB.

### WP-W16-4 — The data diet: jump tables, CP437, whitespace, statics

- **Branch:** `perf/win16-rodata`, on WP-W16-1 (parallel with WP-W16-2).
- **Finish line:** `-C jump-tables=no` in `retro/win16/.cargo/config.toml`; `li_retro::cp437`'s tables are `[u16; 128]` and `[u16; 32]`; the Win16 build links no `WHITESPACE_MAP` (`split_ascii_whitespace`, `trim_ascii` and friends where text is ASCII by construction: switches, receipts, the command line); `dosio::PATH` is a stack buffer; each change's data and code delta in a table in the pull request; DOS and Win16 goldens unchanged.
- **Files owned:** `retro/win16/.cargo/config.toml`, `crates/li-retro/src/cp437.rs`, the `trim` and `split_whitespace` call sites in `crates/li-retro/src/{app.rs,install.rs,text.rs}` and `retro/win16/src/bin/lisetup/{main.rs,dosio.rs}`.
- **Must not touch:** text that is not ASCII by construction (page text, product names: keep `char` rules there), `heap.rs`, `plat.rs`, the packer.
- **Tests:** `cargo nextest run -p li-retro -p li-retro-build --locked`; `make dos`; `make win16`; `e2e-w31.sh --runs 1`.
- **Budgets:** data −800 to −1,530; code +2 KB at most [U].

### WP-W16-5 — Packer: copies of small hot functions, a shorter data segment in the file

- **Branch:** in `rust-win16`, `feat/clone-small`, from its `main`; then `chore/win16-packer-refresh` here, after WP-W16-4.
- **Finish line:** the packer copies a function of at most a configured size (default 64) into each calling segment when that saves bytes (`segments.json` `"clone": {"max": 64}`; never exports, pinned units or functions whose address is taken), and writes the data segment's file length up to its last non-zero byte with the full size as the minimum allocation; `rust-win16`'s tests and Quill's Windows 3.11 loop pass; here the vendored files are refreshed by `PROVENANCE.md`, `SETUP.EXE`'s code falls by about 7 KB and its file by about 41 KB.
- **Files owned:** `rust-win16`'s `tools/ne_pack.py`, `tests/`, `docs/packer.md`; here `retro/win16/tools/ne_pack.py`, `retro/win16/tests/`, `retro/win16/PROVENANCE.md`, `retro/win16/res/lisetup.segments.json` (the `clone` key).
- **Must not touch:** `lisetup`, `li-retro`, the stack and heap sizes.
- **Tests:** `rust-win16`'s `python3 -m unittest`, its segtest on 3.11; here `make win16`, `e2e-w31.sh --runs 3`, `ne_check.py` on the result.
- **Budgets:** data 0; code −6 to −9 KB [U]; file −41 KB.

### WP-W16-6 — One pool (deferred)

- **Branch:** `feat/win16-one-pool`, when the owner says go.
- **Finish line:** the global allocator takes blocks from `LocalAlloc(LMEM_FIXED)` (KERNEL 5, 6 and 7, ordinals read from 3.11), directly or in slabs (decide by the measured install time on 3.11); the arena is gone; the NE heap is 65,536 less the statics, the stack and 64; the gates read the pool; a 13 KB licence shows on 3.11; the 6 KB warning is dropped.
- **Files owned:** `heap.rs`, `build.sh`, `imports.tbl` and `imports.rs`, `docs/retro-win16.md`, `docs/decisions.md`, `crates/li-engine/src/check.rs`.
- **Must not touch:** the packer, `li-retro`.
- **Tests:** as WP-W16-2's, plus the 13 KB licence and an install-time comparison over 3 runs.
- **Budgets:** heap plus local-heap margin at least 8 KB on 3.11 for Lt. Fizz, Planboard and Quill.

## 9. Sonnet briefs

### Brief: WP-W16-1

> Sonnet builder, Liminal Installer, Windows 3.1 runtime. Worktree `~/source/worktrees/liminal-installer/win16-heap`, branch `perf/win16-heap` from `origin/main`. Read `AGENTS.md`, `docs/retro-win16.md`, `retro/win16/PROVENANCE.md` and `docs/plans/win16-headroom.md` sections 1.4, 4 and 5. The data segment of `SETUP.EXE` has 12 bytes free; a quarter of the Rust heap at its peak is the allocator's own overhead (493 blocks, 26,460 bytes asked, 36,112 taken for the Lt. Fizz install). You make the allocator leaner and give 4 KiB of the arena back to the segment.
>
> **Do this.**
>
> 1. **Allocator** (`retro/win16/src/bin/lisetup/heap.rs`). Keep the shape (a first-fit free list over a static arena, sorted by address, neighbours merged on free; single task, never reentrant) and change the costs:
>    - A block is 4-byte aligned and starts with a 4-byte header holding its total size (header included, a multiple of 4). A free block holds its size and the next free block's address (8 bytes), so the smallest block is 8.
>    - `alloc` with `align <= 4` takes `round_up(max(size, 4) + 4, 4)`; a remainder of 8 or more is split off as a free block.
>    - `alloc` with `align > 4` (rare: `u128`, `#[repr(align)]`) takes `round_up(size + 8 + align, 4)`, places the user pointer at the first multiple of `align` at least 8 bytes past the block's start, writes the block's start in the word before the header, and sets bit 0 of the header to say so. `dealloc` reads bit 0 to find the start.
>    - `realloc`: shrink in place (split off a tail of 8 or more); grow in place when the next block in memory is free and big enough; otherwise allocate, copy and free. Keep `used` and `peak` exact through all three.
>    - The census: count the blocks live and the bytes asked for; when `used` makes a new peak, record both. Expose `HEAP.census() -> (blocks_at_peak, asked_at_peak, most_blocks)`.
>    - Keep the arena 8-aligned. Keep the module's doc comment true (and drop its mention of `tools/size_report.py`, which does not exist yet; WP-W16-2 adds it).
> 2. **Host test.** Put the `#[global_allocator]` attribute behind `#[cfg(not(test))]` and add `#[cfg(test)] mod tests` that runs on the host: allocate and free 10,000 random sizes (1 to 600 bytes) and alignments (1, 2, 4, 8, 16) from a seeded generator (write a small xorshift; no crates) against a model `Vec` of live blocks, checking alignment, no overlap with any live block, contents kept through `realloc`, `null` on exhaustion with the heap still usable, and one free block of the whole arena once everything is freed. In `retro/win16/check.sh`, after the Python tests, compile and run it: `rustc --edition 2024 --test -O src/bin/lisetup/heap.rs -o "$out/heaptest" && "$out/heaptest"` (create `$out` before it is used, or use a second `mktemp -d` with its own trap).
> 3. **Log** (`retro/win16/src/bin/lisetup/plat.rs`, `finish` only). Keep the line `heap peak P of H bytes` exactly as it is (`e2e-w31.sh` reads it) and add after it `heap blocks N at the peak, A bytes asked, M at most`, built with `push_str` and `to_string` as the lines around it are.
> 4. **Measure, then cut.** Build with `HEAP_SIZE` still 40 KiB and run the Windows 3.11 loop (below). Record each run's peak and census. Then set `HEAP_SIZE` to `36 * 1024` if every install and uninstall peak, Quill's included if you can build its pack (step 5), is at most 32,768 (a margin of 4,096); if not, take the largest multiple of 1 KiB that keeps 4,096 of margin, and say so in the report as an `OWNER DECISION` line.
> 5. **Runs.** Start your own `Xvfb :NN -screen 0 1280x960x24 -nolisten tcp &` on a free display number, keep its PID, `unset WAYLAND_DISPLAY` and put `DISPLAY=:NN` explicitly in every command that runs Wine or DOSBox-X; never touch the desktop's display and kill only your own PID at the end. `make win16`. Then `DISPLAY=:NN DOSBOX_IMAGE=quill16-dosbox-x:local W31_TREE=$HOME/.cache/quill16-play/w31-base retro/win16/tools/e2e-w31.sh --runs 3 lt-fizz planboard` and the same with `KEPT=1` and `--runs 1` (the image named `liminal-installer-retro:local` in the script is not on this machine; the Quill tag is the same image). If you can build Quill's Win16 pack against your `SETUP.EXE` (Quill's `installer/tools/stage.sh --stand-ins` and `liminst build -t win16-x86 --win16-runtime`), install and uninstall it once on 3.11 and record its peak; if not, say so. `e2e-wine.sh` has one known flake (a Planboard uninstall that leaves the folder, about one run in four): if it fails, run it once more and report both.
> 6. **Docs** in the same commit: `docs/retro-win16.md`'s Memory bullet (the allocator's costs, the arena size, the measured peaks and margins on 3.11, the data figure) and the Program Manager section's stale "48 524 bytes of data ... leave 628 bytes" sentence (replace it with the new figure). No SPEC change: behaviour is unchanged.
>
> **Must not touch.** `retro/win16/tools/ne_pack.py` and every file `PROVENANCE.md` lists as copied, `src/lib.rs`, `link.ld`, `build.sh`'s stack and heap sizes, `res/lisetup.segments.json`, `tools/imports.tbl`, `crates/`, and the checks in the e2e scripts. If you need one of them, stop and report why.
>
> **Budgets.** Data −4,096 (from 49,140 to 45,044, so 4,108 free; the packer prints it); code at most +300 bytes (paste the packer's line before and after); heap margin at least 4,096 bytes on every 3.11 run.
>
> **Check before you commit.** `(cd retro/win16 && cargo fmt --all -- --check)`, `bash retro/win16/check.sh` (clippy with `-D warnings`, the Python tests, the new host test, the build and the NE check), `make win16`, the 3.11 runs above, `bash scripts/check-headers.sh`, and markdownlint in Docker on `docs/retro-win16.md`. Report the commands and results exactly, with a table of the peaks and census per run before and after.
>
> **Git.** Stage explicit paths (never `git add -A` or `git commit -a`). One commit, `perf(win16): leaner heap headers and a 36 KiB arena`, body in markdown with bold labels and no headings, hard-wrapped at 100, written to a file and committed with `git commit -F`, ending with `Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>` (your own model). Push the branch and open a PR: capitalized imperative title, `## Summary` bullets with bold lead-ins, `## Test plan` with `- [x]` items and a last unchecked "Not verified:" line. Do not merge. Clean up with plain commands and full literal paths, one per command (no globs), and delete the build targets you made when you are done.
>
> **Report.** The PR URL, the before and after table, the tests run, anything in the plan you changed and why, and any `OWNER DECISION: question | default taken | where to swap` lines.

### Brief: WP-W16-2

> Sonnet builder, Liminal Installer, Windows 3.1 runtime. Worktree `~/source/worktrees/liminal-installer/win16-gates`, branch `test/win16-gates` from `origin/main` after WP-W16-1 merged. Read `docs/plans/win16-headroom.md` sections 1, 2, 5 and WP-W16-2, and Quill's `check_stack_log.py` and `check_cpu.py` (`~/source/experiments/quill16/win16/tools/`, read only). Add `retro/win16/src/bin/lisetup/probe.rs`: `paint()` at the start of `w16_main` fills from DS word `0x0A` (`pStackTop`) plus 16 to 256 bytes below ESP with `0xA5`; `sample()` at each page build records the least `LocalCompact(0)`; `finish` writes `stack used N of M bytes` (M from `0x0E` less `0x0A`) and `local heap largest free N bytes`. Import `KERNEL 13 LocalCompact w w` (read the ordinal from 3.11's `KRNL386.EXE` with `tools/ne_dump.py`, name it in the table's header, regenerate `src/imports.rs` with `tools/gen_imports.py`). Make `e2e-wine.sh` and `e2e-w31.sh` fail a run whose stack margin is under 2,048 or heap margin under 4,096. Add `tools/size_report.py` (table 1's groups from the ELF and `-Map`, the 20 largest statics and read-only pieces, each segment's placed bytes and veneers from `ne_pack` run as a library, the largest frames when `-Z emit-stack-sizes` is given) and `tools/cpu_scan.py` (decode each segment as `ne_check.py` does and fail on `bswap`, `cmpxchg`, `xadd`, `cpuid`, `rdtsc`, `cmov*`), both run by `check.sh`, which also fails with fewer than 64 bytes of the segment free. Add the `liminst check` warning for a `win16` target whose licence `Doc` is over 6 KB (SPEC §4 names the code; pick `win16-long-document`). Paste the headroom rule from section 5 into `docs/decisions.md` with the next free number. Prove the stack gate fails on 3.11 with a stack of 0x1A00 (not committed). Data +200 at most, code +1 KB at most, both stated. Must not touch `heap.rs` beyond reading its census, the vendored packer, `li-retro`, `cp437.rs` and `.cargo/config.toml`. Xvfb, Git, PR, cleanup and report as in WP-W16-1's brief; commit `test(win16): measure the stack and the local heap, and gate the headroom`.

### Brief: WP-W16-3

> Sonnet builder, Liminal Installer, Windows 3.1 runtime. Worktree `~/source/worktrees/liminal-installer/win16-stack`, branch `perf/win16-stack` from `origin/main` after WP-W16-2 and WP-W16-4 merged. Read `docs/plans/win16-headroom.md` section 1.5 and WP-W16-3, and Quill's `docs/win16-data-segment.md` ("The main frame"). Report the frames on the start-up path with `tools/size_report.py --stack-sizes` first. Move the command-line parse, the uninstall relaunch and the platform set-up out of `w16_main` into `#[inline(never)]` functions so `w16_main` holds under 512 bytes; mark `li_retro::app::run` and `load_project` `#[inline(never)]` only if the frame report shows LTO folding them back. Do not change `li-lpk`: report the inflater's frame. Then lower the `lisetup` stack in `build.sh` to the smallest multiple of 512 that keeps 2,048 bytes of margin on every run of `e2e-w31.sh --runs 3 lt-fizz planboard` and a `KEPT=1` run, and say what it was. Update `docs/retro-win16.md`. Budgets: data plus whatever the stack gives (expected about 2,048), code ±1 KB. Xvfb, Git, PR, cleanup and report as in WP-W16-1's brief; commit `perf(win16): a shallower start-up and a smaller stack`.

### Brief: WP-W16-4

> Sonnet builder, Liminal Installer. Worktree `~/source/worktrees/liminal-installer/win16-rodata`, branch `perf/win16-rodata` from `origin/main` after WP-W16-1 merged. Read `docs/plans/win16-headroom.md` sections 1.2, 1.3 and WP-W16-4. Four changes, each measured alone (the packer's `data` and `code` before and after, in a table in the PR): add `"-C", "jump-tables=no"` to `retro/win16/.cargo/config.toml`'s rustflags with a comment (measured for this plan: data −808, code +773); make `li_retro::cp437`'s `HIGH` and `LOW` `[u16; …]` and convert at the use (`char::from_u32`), keeping the API; replace `char::is_whitespace` uses (`trim`, `split_whitespace`) with the ASCII ones where the text is ASCII by construction (switches, receipt lines, the command line, `/PLAY` scripts) until `WHITESPACE_MAP` is gone from the Win16 link (check with `llvm-nm`), leaving page text alone; and make `lisetup::dosio::PATH` a stack buffer in each caller. DOS and Win16 goldens must not change. Tests: `cargo nextest run -p li-retro -p li-retro-build --locked`, `make dos`, `make win16`, `e2e-w31.sh --runs 1`. Budgets: data −800 to −1,530, code +2 KB at most. Xvfb, Git, PR, cleanup and report as in WP-W16-1's brief; commit `perf(win16): jump tables, CP437 tables and whitespace out of the data segment`.

### Brief: WP-W16-5

> Sonnet builder, `rust-win16` then Liminal Installer. Read `docs/plans/win16-headroom.md` section 1.7 and WP-W16-5, `rust-win16`'s `docs/packer.md` and its `AGENTS.md`. In `rust-win16` (worktree `~/source/worktrees/rust-win16/clone-small`, branch `feat/clone-small`): (1) a `"clone": {"max": N}` key in the segments file makes the packer copy each function of at most N bytes into every segment that calls it across a segment, when the veneers it removes (16 bytes a call site) outweigh the copies and their own outgoing veneers; never an exported procedure, a pinned unit or a function whose address is taken (slots); references inside a copy are resolved in its own segment; the map lists each copy; (2) the data segment's file length stops at its last non-zero byte and the minimum allocation keeps the full size. Tests in `tests/` for both, the segtest on Windows 3.11, and Quill's 3.11 loop if you can run it. Open the PR there and stop; after the owner merges it, refresh `retro/win16` by `PROVENANCE.md` in a second worktree (`chore/win16-packer-refresh`), set `"clone": {"max": 64}` in `res/lisetup.segments.json`, and report the code and file sizes before and after with `make win16` and `e2e-w31.sh --runs 3`. Xvfb, Git, PR, cleanup and report as in WP-W16-1's brief; commits `feat(packer): copy small functions into the segments that call them` and `chore(win16): refresh the packer from rust-win16`.

### Brief: WP-W16-6

> Not to be started without the owner's go (owner decision 4). Sonnet builder, Liminal Installer, Windows 3.1 runtime. Read `docs/plans/win16-headroom.md` option 5 and WP-W16-6. Replace the arena with blocks from `LocalAlloc(LMEM_FIXED)` (KERNEL 5 `LocalAlloc`, 6 `LocalReAlloc`, 7 `LocalFree`, ordinals read from 3.11), first one call per allocation, then slabs of at least 2 KiB if a 3-run install-time comparison on 3.11 shows more than 10% slower; make the NE heap 65,536 less the statics, the stack and 64; keep the census and the gates reading the pool; prove a 13 KB licence on 3.11 and drop the `win16-long-document` warning. Xvfb, Git, PR, cleanup and report as in WP-W16-1's brief.
