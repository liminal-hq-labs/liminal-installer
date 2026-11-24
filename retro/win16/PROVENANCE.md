# Provenance of retro/win16's toolchain

The packer, the import generator, the linker script and the runtime in `src/lib.rs` are copies from [`liminal-hq-labs/rust-win16`](https://github.com/liminal-hq-labs/rust-win16), the project that grew out of this folder's spike. They are copied rather than fetched, as `dos/` is, so a build needs nothing from the network.

**Source.** `rust-win16`, branch `feat/multi-segment`, commit `4c0bac4` ("feat: code in several segments, and Rust on a 32-bit stack", rust-win16 pull request 3). Its `docs/packer.md` describes how the packer and the stubs work.

## What is copied, and what changed here

| File | From | Changed here |
|---|---|---|
| `tools/ne_pack.py`, `tools/ne_dump.py`, `tools/ne_check.py`, `tools/ne_addr2sym.py`, `tools/rust_demangle.py`, `tools/gen_imports.py` | `tools/` | No |
| `tests/test_ne_pack.py`, `tests/test_segments.py` | `tests/` | No |
| `link.ld`, `win16.json`, `.cargo/config.toml` | the same paths | No |
| `tools/imports.tbl` | `tools/imports.tbl` | Nine rows added: `GetDOSEnvironment`, `GetTickCount`, `GetFocus`, `GetWindowText`, `GetWindowTextLength`, `MoveWindow`, `MessageBeep`, `GetDialogBaseUnits` and `DefDlgProc`. `src/imports.rs` is regenerated from it |
| `src/lib.rs` | `src/lib.rs` | The command line is kept (`CMDLINE`, from INITTASK's ES:BX), and a `panic-location` feature names the source line in the panic box |
| `src/probe.rs`, `src/wizard.rs` | the same paths | No |

Everything else in `retro/win16` (`src/bin/`, `res/`, `build.sh`, `check.sh`, the Wine and Windows 3.11 scripts) is this project's own. The inline assembler in `src/bin/lisetup/dosio.rs` follows the rule in `docs/packer.md`: a direct call into DOS or Windows moves to the 16-bit stack and back, and syncs the stack alias.

## Refreshing

Compare the files above with the same paths in `rust-win16` at the commit you want, copy the unchanged ones over, merge `lib.rs` and `imports.tbl` by hand (keeping the lines in the table), regenerate `src/imports.rs` with `bash build.sh --regen`, run `bash check.sh`, and record the new commit here.
