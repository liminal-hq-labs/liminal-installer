# The DOS runtime

`liminst build -t dos-x86` turns an installer project into `SETUP.EXE` and `SETUP.DAT`. `SETUP.EXE` is written in Rust, built with no C compiler and no C library on the foundation that [Desk](https://github.com/liminal-hq/desk) proved (D40). Windows 3.1 stays C and Open Watcom (`retro/`, D4).

## Table of contents

- [What runs on what](#what-runs-on-what)
- [Building and testing](#building-and-testing)
- [The compiled project](#the-compiled-project)
- [Layouts become screens](#layouts-become-screens)
- [The runtime](#the-runtime)
- [What the DOS runtime does not do](#what-the-dos-runtime-does-not-do)
- [Limits](#limits)
- [Where the Desk code came from](#where-the-desk-code-came-from)
- [Follow-ups](#follow-ups)

## What runs on what

| Piece | Where | Is |
|---|---|---|
| `dos/rt` | DOS | The runtime crate copied from Desk: entry, DPMI client, heap, INT 21h, panic path. See [`dos/PROVENANCE.md`](../dos/PROVENANCE.md) |
| `dos/setup` | DOS | `SETUP.EXE`: the DOS `Platform` (text video, BIOS keys, files, ports, speaker, the Ctrl-C guard) and `main` |
| `crates/li-retro` | host and DOS | `no_std` plus `alloc`: the compiled format and validator, the condition VM, the flow VM, the renderer, the installer engine, the `Platform` trait and a host simulator |
| `crates/li-retro-build` | host | The compiler: project and target in, `SETUP.DAT` out |
| `crates/li-render-tui` | host | `flatten_artboard`: draws a layout as the artboard (see below) |
| `crates/li-package` (`dos`), `crates/li-cli` (`dos`) | host | The zip and FAT12 writers, and the `build` output for a `tui-retro` target |

Table 1. The pieces.

`dos/` is its own Cargo workspace with its own lockfile and is not a member of the root workspace, so `make validate` needs no DOS tools. It reaches `li-retro` by path.

## Building and testing

```sh
dos/tools/fetch-tools.sh                 # DOSBox Staging and CWSDPMI into $DOS_VENDOR (default dos/vendor), SHA-256 checked
dos/build.sh                             # dos/dist/SETUP.EXE (set CARGO_TARGET_DIR to keep the output out of the checkout)
dos/check.sh                             # fmt, clippy, build, and SETUP.EXE /V under headless DOSBox
dos/tools/e2e.sh                         # Lt. Fizz: build, install with typed keys, check files, receipt and screens, uninstall
make dos                                 # the four above, then the host tests of the DOS crates
liminst build -t dos-x86 --dos-runtime dos/dist/SETUP.EXE
```

The scripts need `rustup` with `rust-src` (the toolchain file lists it), `python3`, `curl`, `tar` and `xz`. Without the DOS tools `dos/check.sh` and `dos/tools/e2e.sh` skip the DOSBox part with a note, and `make validate` never needs them. `dos/tools/dosrun.sh DIR COMMAND` runs a DOS program headless (`SDL_VIDEODRIVER=dummy`) and returns its output and exit status; `DOS_AUTOTYPE` types keys, `DOS_ENV` sets DOS environment variables, `DOS_MOUNT_OPTS` and `DOS_KEEP` are documented in the script.

The emulator is DOSBox Staging, as Desk uses: it has `AUTOTYPE` and a dummy video driver, and its CPU mode `cputype = 486` faults on any instruction a 486 lacks. DOSBox-X would also work but nothing here needs it.

The tests, in the order they cost:

- **Host unit tests** (`cargo nextest run -p li-retro -p li-retro-build`): the codecs, the condition VM against `li-engine`'s own table, the renderer's keys, the inflater, the sound probes against a pretend port bus.
- **Host installs** (`li-retro-build/tests/e2e.rs`): the whole Lt. Fizz install, uninstall, rollback on cancel, rollback on a write failure, a damaged payload, a full drive, a typed folder and a deselected component, run on `HostPlatform` into a temporary directory, with the tree and the receipt checked.
- **Goldens** (`li-retro-build/tests/golden/`): the welcome, components, copying and done screens. `LI_UPDATE_GOLDEN=1` rewrites them.
- **Artboard structure** (`li-retro-build/tests/parity.rs`): cell-level checks that a screen has the title bar, the side column, the shadowed dialog, `< Button >` buttons and the hint bar, with the right attributes.
- **DOSBox** (`dos/tools/e2e.sh`): the same install under real DOS code, with the welcome, components and copying screens compared with the host goldens character for character.

## The compiled project

`SETUP.DAT` sits beside `SETUP.EXE` (the two are kept together on the disk; appending the data to the executable would save nothing and make the executable slower to patch). It is a header, the project and the payload:

| Offset | Holds |
|---|---|
| 0 | `LIRDAT1\0` |
| 8 | Four little-endian `u32`: project offset and length, payload offset and length |
| project | A [`postcard`](https://docs.rs/postcard) encoding of `li_retro::model::Project` |
| payload | Each file, stored or raw deflate, back to back |

Table 2. `SETUP.DAT`.

`Project` holds the product identity, the theme attributes, a string table, the components, the flows, the screens, the file list and the plan operations. The decoder validates every index, run and bytecode string before the VM touches it.

- **Flows.** `install` and `uninstall` are compiled; the other flows are reported as unused. A step has its kind, its screen, a `when` condition, a built-in, its `next` and its branches.
- **Conditions** are bytecode for a stack machine: `null`, `true`, `false`, a string, a whole number, a path (`target.renderer`, `components`, `state.x.y`, `system.free_space`), a list, `not`, `and`, `or`, `==`, `!=`, `in` and `not in`, with the engine's truthiness and containment rules. A fractional number is a compile error. The compiler walks `li_engine::cond`'s own parser, so the two cannot disagree on syntax, and a test runs the engine's table through both.
- **Files** have an 8.3 destination below the install folder (upper case, checked at compile time), their size, stored length, CRC-32 and method. Compression is raw deflate through `miniz_oxide` (`no_std`), chosen per file when it is smaller. The decoder is streaming: 32 KiB of window and an 8 KiB buffer, whatever the file size.
- **Operations** are `Mkdir`, `WriteIni` (one `key=value`, with `{placeholders}`, skipped when a field is unset) and `Shortcut`. A shortcut is a launcher: a batch file named after the target (`FIZZ.BAT`) that changes to the install folder and runs it. The sound page's answers are written to `SETUP.INI`.
- **Receipt.** `INSTALL.LIR` in the install folder, a text file with `\r\n` line ends: a `LIR 1` line, `product=`, `version=`, `target=`, `dir=`, `components=`, then one record per line: `D` a directory the install made (`.` is the install folder itself; a parent it also made is written as a full path), `F name size crc` a file with its size and CRC-32 in hexadecimal, `I` an INI file and `B` a launcher. `uninstall` removes the files in reverse order, then the receipt, then the directories it made when they are empty, so a file the person added is left alone. The receipt carries CRC-32, not SHA-256; this is the one difference from the host's `receipt.json`.

## Layouts become screens

Each layout a flow uses is drawn once at build time into 80 by 25 cells (a CP437 byte and a VGA attribute each, run-length packed) plus the widgets that need live values. The runtime has no layout engine: it unpacks the cells and draws the widgets over them.

`li_render_tui::flatten_artboard` draws a layout whose root is a `backdrop` as the Fizz-DOS artboard:

```text
 Lieutenant Fizz - Episode One  Setup                         Liminal Installer     <- cyan bar
░░Lt. Fizz░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
░░Episode One░░░░░░░╔═══════════════════ Welcome ════════════════════╗░░░░░░░░░░░░
░░▄▀▀▀▀▄░░░░░░░░░░░░║ Welcome to the Lieutenant Fizz setup.          ║  ░░          <- drop shadow
░░████▀█░░░░░░░░░░░░║ < Next >  < Cancel >                           ║  ░░
░░Step 1 of 8░░░░░░░╚════════════════════════════════════════════════╝  ░░
 Enter Continue  F1 Help  Esc Quit setup                                            <- grey bar, red keys
```

| Part | Comes from |
|---|---|
| Title bar | The backdrop's `title` (left, placeholders filled) and `credit` (right) |
| Background | The backdrop's `fill` character (default a space) |
| Side column | The child with `region = "side"`: `text` nodes (`role = "title"` bright white, others cyan), `image` nodes with `art`, and `progress` nodes |
| Art | `art = { width, height, rects: [[x, y, w, h, "#rrggbb"]] }`: each pixel becomes the nearest of the 16 VGA colours, and two pixel rows make one text row with half and full blocks (`▀ ▄ █`). Vector art (`viewBox`, `shapes`) is not drawn |
| Step bar | `progress` with `text`, `value` and `max`: the text, then a block bar 14 cells wide |
| Dialog | The `dialog` child, drawn by `Screen::flatten`: double border (`border`), the title in the border, shadow, a `width`, and the controls inside it. `button` nodes are `< Label >` in blue on cyan, the focused one white on blue, a disabled one grey, and the `hotkey` letter red |
| Hint bar | The child with `region = "hints"`: its `text` children, each with an optional `key` (red) before the text |

Table 3. The artboard.

Any other root is drawn with li-render-tui's plain Retro look. A screen that needs a different size or place is a layout change, not a runtime one.

Widgets the runtime draws live, in tab order for the controls: text with `{placeholders}` (`{product.name}`, `{install.size}`, `{system.free_space}`, `{progress.file}`, `{plan.summary}`, `{sound.detected}` and any bound field), buttons, checklists and radio groups, fields and paths, the three meters (`file`, `total`, `free_space`, with the low-space warning), the billboard (slides in turn on `interval_seconds`) and the document. Two extensions reach beyond what li-render-tui draws:

- `text` with `source = "LICENCE.TXT"` is a scrolling document (PgUp and PgDn), read at build time.
- `button` with `enabled_when = "state.licence.accepted == 'yes'"` is greyed and ignores Enter until the condition holds.
- A list whose `items` is `"components"` lists the target's components, all ticked, the required ones fixed.

A radio group with no `default` starts with nothing chosen, so the licence page needs a choice before Next works.

The focus description a screen reader needs (`Next, button`) is not on the grid. The runtime hands it to `Platform::announce` each time it draws. The layout is measured with typical-length stand-ins for the live values (`123.4 MB`, `SOUND\ADLIB.DRV`), so a placeholder has room to grow; text may grow to the right edge of its container.

### How the DOS screens differ from `liminst preview --style retro`

The terminal renderer (`tui`) still draws its own plain look and leaves the `side` and `hints` regions out. The differences are by design, until the follow-up below:

| What | `tui` Retro | DOS runtime |
|---|---|---|
| Title bar, side column, art, step bar, hint bar | Left out | Drawn |
| Buttons | `[ Next ]` | `< Next >` |
| Last row | Focus description and key help | Hint bar; the description goes to `announce` |
| `items = "components"` | An empty list | The target's components |
| `text` with `source` | A blank line | A scrolling document |
| `enabled_when` | Ignored | Disables the button |
| Radio group with no `default` | The first item shown chosen | Nothing chosen |
| Characters CP437 lacks (`—`) | Drawn as Unicode | A hyphen (`-`); other glyphs `?`, with a build warning |

## The runtime

`SETUP` installs, `SETUP /U [folder]` removes, `SETUP /V` prints versions. Exit codes are the runner's: 0 done, 1 failed (rolled back), 2 bad arguments or a missing or damaged `SETUP.DAT`, 3 cancelled.

- **Keys.** Tab and Shift-Tab, the arrows, Space, Enter, Esc, an access key (the red letter of a button), F3 and Ctrl-C or Ctrl-Break (all cancel, with Esc on a page with no Back button), PgUp and PgDn on a document. Esc on a page that has a Back button goes back. Esc during the copy cancels and rolls back.
- **Install.** The install folder is made, each selected file is extracted straight to disk and its CRC-32 checked, then the operations and the receipt. There is no journal: a cancel or a failure removes exactly what this run created, in reverse. A file that was already there is overwritten and, on rollback, left as the new copy.
- **Built-ins.** `builtin:detect-sound` reads `BLASTER` (`A220 I5 D1 T3`), else resets a DSP at 210h to 280h and reads the AdLib timers at 388h, and sets `sound.card`, `sound.port`, `sound.irq`, `sound.dma` and `{sound.detected}`. `builtin:check-disk-space` refuses to leave the page when the install does not fit the drive. `check-feed` and `launch-app` are skipped with a build warning.
- **Video.** Text mode 3, written to B8000h through a near pointer, with the card in intensity mode so a background can be bright (the entry field is black on bright white). The mode and the cursor are saved and put back on every exit, including a panic.
- **Memory and CPU.** A 386 or later, DOS 5 or later, a DPMI host (`SETUP.EXE` carries CWSDPMI's), and about 2 MB of extended memory: a 256 KiB stack, the compiled project, a 32 KiB inflate window and small buffers. The runtime must not use `cmpxchg`, which a 386 lacks (DOSBox's `cputype = 486` enforces that in the tests).
- **Testing hook.** `LI_DUMP=D:\DUMP.TXT` makes the runtime append every screen it shows to a file. Nothing else reads it.

## What the DOS runtime does not do

- **No Lua.** Scripts are out of scope for DOS v1 (D41). A step's `on_enter` may name only a `builtin:`. `liminst check` reports a script hook in a project that has a `tui-retro` target as `script-on-retro`, and `build` refuses it. The Lua subset for Windows 3.1 is separate and still a design.
- **One folder.** Every file goes below `{install}`; other placeholders, file types, URL schemes and `PATH` entries are reported at build time and skipped.
- **8.3 names.** A longer name is a build error that names the file.
- **No spanning.** A `floppy-1440` image holds `SETUP.EXE`, `SETUP.DAT` and `README.TXT` on one disk. When they do not fit, the output is reported as not built, with how many bytes it is over; the `zip` still builds.
- **No update, modify, repair or launch.** Only install and uninstall. Repair could be added: the receipt has the CRCs it needs.
- **"Launch after setup"** on the last page is drawn but nothing is started; a DOS exec call is not wrapped yet.
- **Uninstall runs from the install media**, `SETUP /U` with the folder; it does not copy itself into the install folder.
- **Whole numbers only** in conditions, and `system.free_space` stops at 2 GiB, which is what INT 21h AH=36h reports.

## Limits

| Thing | Limit |
|---|---|
| Screens | 80 by 25, code page 437 |
| Folder names | 8.3, upper case |
| Install folder length | 64 characters typed |
| Files | As many as a 224-entry root directory for a floppy; unlimited for a zip |
| A file | 4 GiB less the payload; the format uses 32-bit offsets, so the whole payload is under 4 GiB |
| Free space shown | 2 GiB |

Table 4. Limits.

## Where the Desk code came from

`dos/PROVENANCE.md` lists every file copied from Desk (commit `4a021a7`), what changed in it and how to refresh it. `rt` lost the modules this program does not use (the hardware abstraction layer, IRQ handling, the timer and the graphics proof) and gained `mkdir`, `rmdir`, `attributes` and `free_space`. The packer and the harness are copies with small changes. Desk packs behind DJGPP's stub; here `SETUP.EXE` is packed behind CWSDPMI's own `CWSDSTUB.EXE`, which has the DPMI host inside it, so DJGPP is not downloaded.

## Follow-ups

- Make the Studio's *Terminal (real)* view and `liminst preview` draw `flatten_artboard`'s cells instead of the plain Retro look, so the two agree with the DOS runtime. The flattening is in `li-render-tui` for that reason; what is left is painting a `Flat` into a ratatui buffer (a VGA attribute to colours table) and a `preview --style artboard`.
- A repair command from the receipt's CRCs, and a launch call.
- Disk spanning for floppy images.
- Run the example on a real DOS machine or a DOS virtual machine. Only DOSBox Staging has been used.
