# Retro runtime spike (Windows 3.1 and the C toolchain)

The DOS `tui-retro` runtime is no longer written in C: it is Rust, in [`dos/`](../dos) and `crates/li-retro` (see [docs/retro-dos.md](../docs/retro-dos.md) and D37). This folder keeps the Open Watcom spike for the Windows 3.1 (`win16`) runtime, which stays C.

A feasibility check for the DOS runtime in [SPEC.md](../SPEC.md) (target `dos-x86`, floor set by D4): can CI build a 386 protected-mode `SETUP.EXE` and run it headless?

`bash retro/build.sh` builds the image in `Dockerfile` (Open Watcom v2 snapshot plus DOSBox-X from Debian), compiles `setup.c` with `wcl386 -bt=dos -l=pmodew` into `out/SETUP.EXE`, runs it under DOSBox-X with a dummy video driver, and checks that the redirected output contains `SETUP OK`. Pass `--no-run` to build only.

- **Extender:** PMODE/W is linked into the executable, so the file runs on a bare DOS with a 386 or later. DOS/4GW needs its stub bound separately, and the `-l=dos4g` output did not start on its own.
- **Toolchain pin:** the image downloads the dated snapshot named by `OW_TAG`. Open Watcom v2 publishes pre-release snapshots only, so bump the tag deliberately.
- **Size:** the snapshot is about 150 MB, so the first build is slow. Later builds reuse the Docker layer cache.
- **Not covered:** floppy images, the retro Lua subset and the text-mode renderer. This only proves the toolchain and the headless run.
