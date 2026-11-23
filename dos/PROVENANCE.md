# Provenance

The Rust-on-DOS foundation in this folder is copied from [Desk](https://github.com/liminal-hq/desk)'s `dos/` (and its `tools/fetch-tools.sh`), which proved a no-C-compiler, no-C-library way to build 32-bit DOS programs from Rust. House rules allow copies with provenance, so the files are kept close to the original and every change is listed here.

- **Source:** `liminal-hq/desk`, commit `4a021a7` ("Record the layered layout as merged in the Manual and the progress report (#100)").
- **Licence:** Apache-2.0 OR MIT, the same as this repository; every copied file keeps its header.

## What was copied

| Here | From Desk | Changes |
|---|---|---|
| `dos-elf.json` | `dos/dos-elf.json` | None. The i486 ELF target specification (x87 hard float, SIMD off) |
| `link.ld` | `dos/link.ld` | None. The linker script `rust-lld` uses with it |
| `.cargo/config.toml` | `dos/.cargo/config.toml` | None. `-Zbuild-std` for `core`, `alloc` and `compiler_builtins`, and the linker flags |
| `rt/src/{start,dpmi,dos,heap,env,fpu,io,panic,cleanup}.rs` | `dos/rt/src/` | Reformatted with hard tabs (`rustfmt.toml`). Otherwise unchanged |
| `rt/src/lib.rs` | `dos/rt/src/lib.rs` | The modules not copied (`bios_kbd`, `irq`, `timer`, `vga`, `hal`, `mem`) are removed from the list |
| `rt/Cargo.toml` | `dos/rt/Cargo.toml` | The `desk-hal`, `desk-pic` and `desk-vga` dependencies are removed: the copied modules do not use them |
| `tools/elf2go32.py` | `dos/tools/elf2go32.py` | None. Packs the linked ELF behind a go32 stub |
| `tools/dosrun.sh` | `dos/tools/dosrun.sh` | Finds DOSBox under `$DOS_VENDOR` (default `dos/vendor`), mounts the scratch directory as `D:`, adds `DOS_ENV` (DOS environment variables) and `DOS_KEEP` (keep the scratch files, such as a screen dump) |
| `tools/fetch-tools.sh` | `tools/fetch-tools.sh` | Fetches only DOSBox Staging and CWSDPMI, into `$DOS_VENDOR`. Same URLs and SHA-256 sums. DJGPP is not fetched |
| `build.sh` | `dos/build.sh` | Builds one program, `setup`, and packs it behind CWSDPMI's `CWSDSTUB.EXE` instead of DJGPP's stub, so `SETUP.EXE` carries its DPMI host |

Not copied: the `hal` crates, the IRQ, timer and VGA modules, the graphics-mode proof programs, the transcripts and the other tools. This repository's own code is `setup/` (the program) and, outside this folder, `crates/li-retro`.

## What differs in use

Desk packs behind DJGPP's `STUB.EXE` and ships `CWSDPMI.EXE` beside the programs. Here `SETUP.EXE` is packed behind `CWSDSTUB.EXE`, the stub that has CWSDPMI inside it, so the installer runs from a bare floppy. `elf2go32.py` supports both stubs, so DJGPP's `stubify -g` output can be used instead by passing it as the stub.

## Refreshing

1. Check out Desk at the commit to take, and diff each file in the table against its Desk original, ignoring whitespace (`diff -w`).
2. Copy the changed files over, then run `cargo fmt --all` in `dos/` to put the hard tabs back.
3. Re-apply the changes in the table (`lib.rs`, `rt/Cargo.toml`, `dosrun.sh`, `fetch-tools.sh`, `build.sh`).
4. Run `dos/check.sh` with the tools fetched, and update the commit hash above.
