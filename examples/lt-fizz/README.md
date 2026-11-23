# Lieutenant Fizz example

Lieutenant Fizz is the retro example: a DOS text-mode target (`fizz-dos`) beside Windows 3.1, Windows 95 and Linux targets (`fizz-win95`). It shows a sound-card page that only retro renderers reach, and the built-in actions the DOS runtime runs for sound detection (`builtin:detect-sound`) and the disk-space check (`builtin:check-disk-space`). `installer/scripts/sound-setup.lua` is the same detection as a Lua script, for runners that run scripts; the DOS runtime has no Lua.

## Layout

- `installer/installer.toml` and `installer/components.toml` describe the product and what it installs.
- `installer/flows/` holds one flow per lifecycle: install, update, modify, uninstall and launch.
- `installer/layouts/` holds the page layouts the flows name. Each node may carry per-renderer overrides.
- `installer/themes/` holds one folder per theme, each with a `theme.toml` and its assets.
- `installer/themes/fizz-dos/layouts/` and `installer/themes/fizz-win95/layouts/` hold each theme's own install-flow layouts, drawn after the DOS text-mode and Windows 95 designs. `installer/layouts/` holds the plain fallback and the update and uninstall pages.
- `installer/dist/` holds tiny payload stand-ins. The `.EXE` and Mach-O files are text, and the Linux executables are shell scripts.

## Checking it

`cargo nextest run -p li-examples-test` loads every example with `li-model` and checks the flows, layouts, themes and payload paths. Open the folder in Studio with `apps/studio` to edit it.

Built-in actions in flows are written `builtin:<name>`: `check-feed`, `launch-app`, `detect-sound` and `check-disk-space`. Build the DOS target with `liminst build -t dos-x86` (see [docs/retro-dos.md](../../docs/retro-dos.md)).
