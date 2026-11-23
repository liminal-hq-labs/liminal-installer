# Lieutenant Fizz example

Lieutenant Fizz is the retro example: a DOS text-mode target (`fizz-dos`) beside Windows 3.1, Windows 95 and Linux targets (`fizz-win95`), and a modern Windows 11 target (`fizz-modern`). It shows a sound-card page that only retro renderers reach, and the built-in actions the DOS runtime runs for sound detection (`builtin:detect-sound`) and the disk-space check (`builtin:check-disk-space`). `installer/scripts/sound-setup.lua` is the same detection as a Lua script, for runners that run scripts; the DOS runtime has no Lua.

## Layout

- `installer/installer.toml` and `installer/components.toml` describe the product and what it installs.
- `installer/flows/` holds one flow per lifecycle: install, update, modify, uninstall and launch.
- `installer/layouts/` holds the page layouts the flows name. Each node may carry per-renderer overrides.
- `installer/themes/` holds one folder per theme, each with a `theme.toml` and its assets.
- `installer/themes/fizz-dos/layouts/`, `installer/themes/fizz-win95/layouts/` and `installer/themes/fizz-modern/layouts/` hold each theme's own install-flow layouts and its `stage`, drawn after the DOS text-mode, Windows 95 and Windows 11 designs. `installer/layouts/` holds the plain fallback and the update and uninstall pages, which the Windows 11 theme draws inside its own stage.
- `fizz-modern` is a theme stage (a wallpaper backdrop and a `slot`) plus page layouts that are each a `window` with a step rail, so every page shares the Mica window. The copying page carries one slim progress bar for the overall progress; the file, total and free-space status meters belong to the InstallShield-style looks, not here. It is the `windows-x86_64:web` target (the `web` renderer runtime is a later task, so for now the Studio draws it and `liminst preview -t windows-x86_64:web` prints the text fallback). It follows the system's light or dark scheme and uses the `variant` and `control` properties for its hero art, the “Open README” link and the component switches (see `SPEC.md`, section 6). The pixel-art Lt. Fizz stays as the art; the type is Segoe UI Variable with a system fallback, and no font is bundled.
- `installer/dist/` holds tiny payload stand-ins. The `.EXE` and Mach-O files are text, and the Linux executables are shell scripts.

## Checking it

`cargo nextest run -p li-examples-test` loads every example with `li-model` and checks the flows, layouts, themes and payload paths. Open the folder in Studio with `apps/studio` to edit it. Two targets share `windows-x86_64` (`win32-classic` and `web`), so name the one you mean as `windows-x86_64:web` when running `liminst preview`.

Built-in actions in flows are written `builtin:<name>`: `check-feed`, `launch-app`, `detect-sound` and `check-disk-space`. Build the DOS target with `liminst build -t dos-x86` (see [docs/retro-dos.md](../../docs/retro-dos.md)).
