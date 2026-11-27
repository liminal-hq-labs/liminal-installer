# The Windows 3.1 runtime

`liminst build -t win16-x86` turns an installer project into `SETUP.LPK`, the Liminal Pack the Windows 3.1 runtime reads ([lpk-format.md](lpk-format.md)), and carries `SETUP.EXE`, the runtime, when `--win16-runtime`, `LIMINST_WIN16_RUNTIME` or `retro/win16/out/SETUP.EXE` names one; the `floppy-1440` output fails without it, naming `--win16-runtime` (D75). The runtime is Rust built for a 16-bit code segment and packed into an NE executable (D46, [win16-spike.md](win16-spike.md)), on the same portable core as the DOS and Windows 95 runtimes: the flow VM and installer engine of `li-retro`, a `Gui` for the pages (shared with Win32) and a `Shell` for what the machine's shell does (D69, D73).

**Status.** The whole `SETUP.EXE` links and runs. Its code is 178 851 bytes, which no single 16-bit code segment can hold, so the packer places it in five (the packer is vendored from rust-win16, see [`retro/win16/PROVENANCE.md`](../retro/win16/PROVENANCE.md)). Under Wine and on Windows for Workgroups 3.11 in DOSBox-X it installs and uninstalls Lt. Fizz and Planboard through its real controls: every run passed on 3.11 (see Table 3). What is unproven is real hardware (see [What is not done](#what-is-not-done)).

## Table of contents

- [What runs on what](#what-runs-on-what)
- [Building and testing](#building-and-testing)
- [The target](#the-target)
- [Pages become Windows controls](#pages-become-windows-controls)
- [The runtime](#the-runtime)
- [The Program Manager group](#the-program-manager-group)
- [The kept uninstaller](#the-kept-uninstaller)
- [The Windows and system folders](#the-windows-and-system-folders)
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

`SETUP` installs, `SETUP /U [folder]` removes, `SETUP /V` shows the version; `/S` and `/COMPONENTS=a,b` install without pages and name the components (see [retro-dos.md](retro-dos.md)). Exit codes are the runner's: 0 done, 1 failed (rolled back), 2 bad arguments or a damaged pack, 3 cancelled.

- **Pack.** `SETUP.LPK` beside `SETUP.EXE`, found through `GetModuleFileName`, read with `_lopen`, `_llseek` and `_lread`. The compiler packs for Windows with 4 KiB chunks (`WIN16_CHUNK_LOG2`, 12) instead of DOS's 32 KiB, so the decoder's window, which the heap holds for the whole run, is an eighth the size. Every chunk's CRC-32 is checked before the bytes are written.
- **Files.** Created with `_lcreat` and `_lwrite`; directories, delete, attributes and free space by INT 21h (39h, 3Ah, 41h, 43h, 36h), each on the real 16-bit stack (see the ESP finding below). Free space stops at 2 GiB, as on DOS.
- **Receipt.** `INSTALL.LIR`, the DOS receipt unchanged apart from `target=win16-x86:win16`. The same `FIZZ.BAT` launcher is written and recorded as a `B` line.
- **Shell.** Windows 3.1's `Shell` says `ShortcutKind::GroupItem` and has no uninstall list; `add_group_item` returns `GroupAdd::Unavailable` or `Added { made_group }`, and `remove_group(group, items, made_group)` takes the items and the group out in one conversation. The commands are built by `li_retro::progman` (quoting, the `IconPath` rule, the `.GRP` name) and recorded by the host's simulated shell, which the goldens in `tests/golden/win16/lt-fizz.progman.txt` read. The conversation with `PROGMAN` is `lisetup/dde.rs` (D77, below); when there is no Program Manager to talk to, the install falls back to the batch launcher.
- **Built-ins.** `detect-sound` reads `BLASTER` through `GetDOSEnvironment` and probes the ports with `in` and `out` (DOSBox-X's emulated Sound Blaster answers it); `check-disk-space` is the engine's.
- **Memory.** A 36 KiB heap in the data segment under a first-fit allocator with a four-byte header and four-byte granularity (a block of 10 bytes takes 16; a block of 1 byte takes 8), an 8 KiB stack, an 8 KiB local heap for the edit controls' text and a 64-entry shadow return stack; the data segment holds all of it in 64 KB. `realloc` shrinks in place and grows into a free neighbour, and an alignment over four costs eight bytes more and a back-pointer. The allocator's host test (`heap.rs` under `rustc --test`, run by `check.sh`) runs 10,000 random allocations, frees and reallocations against a model. The log's `heap peak P of H bytes` line is followed by `heap blocks N at the peak, A bytes asked, M at most`. Measured on Windows 3.11: the install's peak was 29 688 bytes for Lt. Fizz (493 blocks, 27 017 bytes asked) and 26 888 for Planboard (413 blocks), so the margin in the 36 864-byte arena is 7 176 bytes; the kept uninstaller and `SETUP /U` peak at 19 744 to 24 036. Before, with eight-byte headers and sixteen-byte granularity, the peaks were 35 552 and 31 952.

## The Program Manager group

Windows 3.1 has no function that makes a group or an item, so `lisetup/dde.rs` talks to Program Manager over DDE with the raw `WM_DDE_*` messages (D77). DDEML was not used: it needs a callback made with `MakeProcInstance`, string handles and a fourth imported module, where the raw messages need a hidden window and its procedure (`export_proc!`, as the wizard has).

1. A hidden window of class `LiminalDde`; `SendMessage(-1, WM_DDE_INITIATE, hwnd, MAKELONG(atom, atom))` with the global atom `PROGMAN` for the application and the topic. The first `WM_DDE_ACK` names the server window; the atoms in every ACK are deleted. No ACK in a quarter of a second means no Program Manager (another shell), and `add_group_item` returns `Unavailable`, so the batch launcher is written.
2. `WM_DDE_REQUEST` for the item `PROGMAN` brings the group names as `WM_DDE_DATA`; the group exists when one line equals its name, ignoring case. A list that cannot be had counts as "exists", so a group is never deleted by mistake.
3. Each command is a `WM_DDE_EXECUTE` with a `GMEM_MOVEABLE | GMEM_DDESHARE` block, waited for with the message pump for at most five seconds. Install: `[CreateGroup(name,FILE.GRP)]`, `[ShowGroup(name,1)]`, `[ReplaceItem(item)]` (refused when there is no item yet, which is fine), `[AddItem(command,item,iconpath,0,-1,-1,workdir)]`, and the four-argument `AddItem` if that is refused. If the item cannot be added the group is deleted again when this call made it, and the install falls back to the batch launcher. Uninstall: `[ShowGroup]`, a `[DeleteItem]` for each `G` record, and `[DeleteGroup]` only when the receipt has `P`.
4. After an add, the group's own listing is requested (item = the group name) and its start is written to `/LOG`: `progman: group: "Planboard",C:\WINDOWS\PLANBOAR.GRP,1,1|"Planboard","C:\PLANBOAR\PLANBRD.EXE",...`. That line, and `progman: removed; group gone` after an uninstall, are the in-guest proof; they do not depend on Program Manager having written its files.
5. `WM_DDE_TERMINATE`, a wait of a second for the server's own, and the window is destroyed.

The window procedure never allocates (the heap is not reentrant) and the state is about 1 KB of statics, because the data segment has little room left (45 144 bytes of data, an 8 KiB stack and an 8 KiB local heap leave 4 008 bytes of 64 KB): the group list is searched as it is read.

The new imports are `GlobalAddAtom` (USER 268), `GlobalDeleteAtom` (USER 269) and `IsWindow` (USER 47). The atom functions are **USER**'s, not KERNEL's: KERNEL 70 and 71 are the local `AddAtom` and `DeleteAtom`. All three ordinals were read from Windows 3.11's own `USER.EXE` with `tools/ne_dump.py` and agree with Wine's `user.exe16.spec`.

Group names go through the same ANSI mapping as the pages, so an em dash in a product name is a hyphen in Program Manager (`Lieutenant Fizz - Episode One`); the `.GRP` file name is the first eight letters and digits of the name (`LIEUTENA.GRP`).

## File types

A component's `file_types` become `FileType` operations (D86). Windows 3.x associates a program with an extension in the `[Extensions]` section of `WIN.INI`, one line each, `ext=PROGRAM ^.ext`, and only for three-letter extensions: the compiler skips a longer one with a warning, which is why Planboard's Windows 3.1 component declares `.pbd` beside the `.planboard` of the others (D88).

- **Install.** `GetProfileString("Extensions", ext)` reads the line that is there (a default no value can be, `\x01`, tells a missing line from an empty one), `WriteProfileString` writes `ext=<install folder>\PROGRAM.EXE ^.ext`, and `SendMessage(HWND_BROADCAST, WM_WININICHANGE, 0, "Extensions")` tells File Manager to read the section again. The receipt's `A` record keeps what the line said (SPEC §9).
- **Uninstall.** The old line goes back, or the line is deleted (`WriteProfileString` with a NULL value) when there was none, but only while the line still names this program; a line changed since is left. A second install over the first keeps what the first one replaced.
- **Not `WriteIni`.** `WriteIni` edits an INI file below the install folder; `WIN.INI` is the system's, so the section is written through the profile functions, which are also what tell the system the file changed.
- **Imports.** `GetProfileString` (KERNEL 58) and `WriteProfileString` (KERNEL 59) are in `tools/imports.tbl`, with the ordinals read from Windows 3.11's own `KRNL386.EXE` with `tools/ne_dump.py`. `WriteProfileString_delete` is the same export called with a far NULL for the value.
- **Wine.** Wine, like Windows NT, maps WIN.INI's `[Extensions]` to `HKCU\Software\Microsoft\Windows NT\CurrentVersion\Extensions`, so `e2e-wine.sh` reads and seeds it there; `e2e-w31.sh` reads the real `WIN.INI`.

## The kept uninstaller

The install keeps an uninstaller in the install folder, as the Win32 runtime does (D66, D78): `UNINST.EXE`, a copy of the running `SETUP.EXE` made 1 KiB at a time (a 250 KB program does not fit the 36 KiB heap), and `UNINST.DAT`, the compiled project, which the install streams from the pack a chunk at a time (`Pack::write_project`, `Shell::streams_project`): reading it whole into a `Vec` at that point of an install ended in a "Rust panic" box on Windows 3.11 (the heap is at about 35 KB there, so an allocation failure is the likely cause; the cause was not isolated, only the streaming shown to remove it). Both are `X` records in the receipt, and the group gets an item, `Uninstall <product>`, whose command line is `C:\FOLDER\UNINST.EXE /U C:\FOLDER`. Windows 3.1 has no Add/Remove Programs, so the item is the way in; the Windows 95 key is not written.

Windows will not delete a program that is running, so `UNINST.EXE /U` does not do the work itself. When `UNINST.DAT` is beside the running program and the command line has no `/X`, it copies itself to `LIUNINST.EXE` in the folder `TEMP` (or `TMP`) names, else the Windows folder, starts that copy with `WinExec` (`/U folder /X /DAT=folder\UNINST.DAT`, and the test flags `/LOG`, `/PLAY`, `/DUMP` and `/PAUSE` as they were) and exits at once, before it creates a window or a log. The copy reads the project from `/DAT`, runs the project's `uninstall` flow (the confirm page, the copying page, the done page), asks Program Manager to delete the items and, when the receipt has `P`, the group, removes the files the receipt lists including `UNINST.EXE` and `UNINST.DAT`, and then the folder. The copy cannot delete itself: `LIUNINST.EXE` stays in the temporary folder (the Win32 runtime leaves its copy on Windows 9x for the same reason), and the next uninstall overwrites it. `SETUP /U [folder]` from the install media works as before and reads `SETUP.LPK`. When the copy or the `WinExec` fails, the uninstall runs in place, and removes everything but `UNINST.EXE`.

One more fix came out of it: DOS refuses to remove the current directory, and an uninstall started with the install folder as its working folder (from the group item) is in it, so `rmdir` moves to the drive's root and tries again. The new imports are `WinExec` (KERNEL 166) and `GetWindowsDirectory` (KERNEL 134), both read from Windows 3.11's `KRNL386.EXE`.

## The Windows and system folders

A component may install a file into the Windows folder or the system folder (`{windows}/` and `{system}/` in the project, `%WINDIR%\NAME` and `%SYSDIR%\NAME` in the compiled file list; D95, D97, D98). Lt. Fizz's unticked `screensaver` component installs `FIZZ.SCR` into the Windows folder, where Control Panel's Desktop lists screen savers.

- **Folders.** `Win16Platform` answers `Shell::windows_dir` with `GetWindowsDirectory` (KERNEL 134) and `Shell::system_dir` with `GetSystemDirectory` (KERNEL 135), each into the same 144-byte stack buffer and without a trailing backslash (`C:\WINDOWS`, `C:\WINDOWS\SYSTEM`). Neither costs a static.
- **The ordinal.** `GetSystemDirectory` is `GETSYSTEMDIRECTORY`, 135, in the export table of Windows for Workgroups 3.11's own `KRNL386.EXE` (`tools/ne_dump.py` lists it beside `GETWINDOWSDIRECTORY`, 134), which is also what Wine's `krnl386.exe16.spec` says. It is one import more in `tools/imports.tbl`, so `src/imports.rs` was regenerated, never edited.
- **Copying.** `Shell::copy_file` is `dosio::copy_file`, the same 1 KiB-at-a-time copy the kept uninstaller uses, so a backup costs a 1 KiB buffer for the moment it runs. `Platform::notice` writes a line to `/LOG` through `note`: `backup: C:\WINDOWS\FIZZ.SCR -> BACKUP\WIN\FIZZ.SCR`.
- **What the install does.** A file of the same name that this product did not put there is copied to `BACKUP\WIN\` (or `BACKUP\SYS\`) in the install folder, then replaced; the receipt has `F C:\WINDOWS\FIZZ.SCR size crc` and `O C:\WINDOWS\FIZZ.SCR|BACKUP\WIN\FIZZ.SCR`. Cancelling or failing puts the backup back; the uninstall removes the file, copies the backup over the original, and removes `BACKUP` when it is empty (SPEC §8 and §9). The core is `li-retro`'s; the runtime only answers the three shell calls and logs.
- **Budgets.** No static was added: the data segment is 45 144 bytes, 4 008 free (it was 45 128 and 4 024: 16 bytes of read-only text for the copy error and the log). The heap's peak on Windows 3.11 is 30 488 of 36 864 bytes for Lt. Fizz with the saver ticked, against 29 932 without it (the backup's buffer and paths), so the margin is 6 376 bytes. The code grew by 431 bytes (178 420 to 178 851) in the same five segments; the last, which took the new code, has 9 333 bytes of its 40 000.

## Sizes and segments

| | Bytes |
|---|---|
| Code, in five segments | 178 851 (29 800, 47 876, 47 337, 44 505 and 9 333) |
| Call veneers, jump stubs and pointer slots | 1 560, 19 and 40 |
| Data (static data and the 36 KiB heap) | 45 144 |
| Stack, local heap | 8 192, 8 192 |
| `SETUP.EXE` | 251 223 |

Table 2. What the packer reports for `SETUP.EXE`.

The file is larger than the code because the packer keeps the data segment, with its 36 KiB of zero heap, in the file (Windows 3.11 does not zero what a file leaves out). The first whole build, before the hand-written decoder and the other cuts, was 121 725 bytes of code; with several segments the limit stopped being a blocker, and `res/lisetup.segments.json` keeps a budget so a segment cannot creep to 64 KB unnoticed.

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
| Wine 11.21, Program Manager | The same run: `progman: group missing`, `progman: added` in `/LOG` for both examples, one `G` and one `P` record each, and the groups gone after `/U` | Wine's `PROGMAN` accepted the DDE conversation |
| Windows for Workgroups 3.11 in DOSBox-X, Program Manager | `e2e-w31.sh --runs 20` for each example, in two parallel workers: the group is made and the item added (`progman: added`, then Program Manager's own listing of the group in `/LOG`), `LIEUTENA.GRP` or `PLANBOAR.GRP` is in `C:\WINDOWS` and named in `PROGMAN.INI`, then after `/U` the log says `progman: removed; group gone`, the file and the `.INI` line are gone | 20 of 20 for Lt. Fizz and 20 of 20 for Planboard, each also passing the page, receipt and folder checks as before. Screenshots: [Lt. Fizz group open](images/win16-w31-progman-group-lt-fizz.png), [Planboard group open](images/win16-w31-progman-group-planboard.png), [after the uninstall](images/win16-w31-progman-removed-lt-fizz.png) |
| Wine 11.21, kept uninstaller | `KEPT=1 e2e-wine.sh`: after the install the group has two items and the folder has `UNINST.EXE` and `UNINST.DAT`; `C:\FOLDER\UNINST.EXE /U` removes it all through its copy | Pass for both examples |
| Windows for Workgroups 3.11 in DOSBox-X, kept uninstaller | `KEPT=1 e2e-w31.sh --runs 20` for each example, in two parallel workers: the install keeps `UNINST.EXE` and `UNINST.DAT` and the group lists two items; the uninstall is `C:\FOLDER\UNINST.EXE /U C:\FOLDER`, which starts its copy from the temporary folder; the group, the files and the folder go | 20 of 20 for Lt. Fizz and 20 of 20 for Planboard. Screenshots: [Planboard group with its Uninstall item](images/win16-w31-kept-group-planboard.png), [Lt. Fizz](images/win16-w31-kept-group-lt-fizz.png), [after the uninstall](images/win16-w31-kept-removed-planboard.png) |
| Windows for Workgroups 3.11 in DOSBox-X, kept uninstaller installed, `SETUP /U` from the disk | `e2e-w31.sh --runs 10` for each example | 10 of 10 for each |
| Wine 11.21, file type | `e2e-wine.sh`: Planboard's `pbd` line in `[Extensions]` (Wine keeps the section in its registry) is seeded with Notepad's, replaced by Planboard's on install with the receipt's `A` record keeping Notepad's, and Notepad's is back after `/U` | Pass |
| Windows for Workgroups 3.11 in DOSBox-X, file type | `e2e-w31.sh --runs 3 planboard`: `WIN.INI`'s `pbd=notepad.exe ^.pbd` becomes `pbd=C:\PLANBOAR\PLANBRD.EXE ^.pbd`, the receipt's `A` record keeps Notepad's line, and after `/U` the `WIN.INI` line is Notepad's again, with the group, files and folder checks as before | 3 of 3 pass; Lt. Fizz, which has no file type, 2 of 2 |
| Wine 11.21, the Windows folder | `e2e-wine.sh`: the third run, `lt-fizz-saver`, ticks the screen saver over a `FIZZ.SCR` seeded in `C:\windows`: the new file is there, the seeded bytes are in `BACKUP\WIN`, the receipt has the `F` and `O` records and the log a `backup:` line; after `/U` the seeded bytes are back and the folder is gone. Plain Lt. Fizz installs no `FIZZ.SCR` | Pass |
| Windows for Workgroups 3.11 in DOSBox-X, the Windows folder | `e2e-w31.sh --runs 5` (and `KEPT=1 --runs 2` for the saver): the same checks against `C:\WINDOWS`, with `lt-fizz` and `planboard` as before | 15 of 15 and 2 of 2 pass; the peak was 30 488 bytes with the saver |
| Windows ME in VirtualBox | Not run: the VM was in use and then being debugged, and the rules say to leave it alone | Pending |
| Windows 3.1 Standard mode, a 286, real hardware | Not run | Untested |

Table 3. Where it ran.

## What is not done

- **Real hardware.** Everything on Windows 3.11 ran under DOSBox-X. The ESP fix is the same code rust-win16 verified there; whether a real 386, 486 or Pentium behaves as the emulator does is not shown, nor another emulator or a Windows 3.1 retail install, nor Windows ME.
- **A Program Manager group on another shell.** Norton Desktop or another replacement shell that does not answer `PROGMAN` gets the batch launcher; that path is tested only by the host's simulated shell and by a hand-made refusal, not on such a shell. A group the person added items to is deleted with its items when this install made it.
- **Launch after setup, repair and update**, as on DOS.
- **A temporary copy that stays.** The uninstaller's copy, `LIUNINST.EXE`, is left in the temporary folder, as on Windows 9x.
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
