# Provenance

The Win32 build follows two local experiments that proved a Rust program with no C runtime running on Windows 95-era systems. House rules allow copies with provenance, so nothing is fetched at build time.

| Here | From | Changes |
|---|---|---|
| `.cargo/config.toml` | `~/source/win9x-rust/.cargo/config.toml`, as extended by Quill's `win9x/.cargo/config.toml` | The target is `win32.json`, and `core` and `alloc` are built from source (`-Zbuild-std`) so the code is Pentium class. `/FILEALIGN:512` is added. The `.res` link argument is dropped: the wizard needs no menu, icon or version resource |
| `win32.json` | Quill's `win9x/win9x.json`, itself `rustc --print target-spec-json --target i686-pc-windows-msvc` | None. CPU `pentium`, MMX, SSE and x87 off, the soft-float ABI, the `i586` triple, `panic-strategy` abort |
| `setup/src/rt.rs` | Quill's `win9x/quill-win9x/src/rt.rs` | The allocator over the process heap, `__chkstk` and the 64-bit division helpers are unchanged; the panic handler exits with 70 so a script can tell a crash from the runner's exit codes |
| `setup/src/sys.rs` | The declarations in `win9x-rust/src/main.rs` and Quill's `sys.rs` | Many more calls (windows, controls, GDI, shell, registry, COM), as `unsafe extern` blocks for edition 2024 |
| `tools/scan-pentium.sh`, `tools/check-imports.sh` | Quill's `win9x/tools/` | Renamed for `SETUP.EXE`; the objdump is looked up under its versioned names; `WIN32_REQUIRE_TOOLS=1` turns a missing tool into an error. `imports.allow` is this program's own list |
| `tools/wine-e2e.sh` | The idea of Quill's `win9x/tools/wine-e2e.sh` | Written for this program: it installs and uninstalls two projects and compares pages, files, receipt, shortcut and registry |

This repository's own code is `setup/src/{main,gui,platform,shell,text}.rs` and, outside this folder, `crates/li-retro` (the flow VM and the `Gui` trait) and `crates/li-retro-build` (the compiler's Windows flavour).

## What differs from Quill

Quill's program is a word processor that paints its own page. This one is an installer on real controls, so it imports comctl32, shell32, advapi32 and ole32 as well. The two share the technique, not the code.

## Refreshing

Compare `win32.json` with `rustc --print target-spec-json -Zunstable-options --target i686-pc-windows-msvc` on a newer toolchain: the soft-float warning the build prints (`target feature soft-float must be enabled`) will become an error in a later Rust, and the `rustc-abi` and `features` entries are where it is fixed. Then rebuild with `win32/check.sh`.
