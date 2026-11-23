# Lieutenant Fizz example

Lieutenant Fizz is the retro example: a DOS text-mode target (`fizz-dos`) beside Windows 3.1, Windows 95 and Linux targets (`fizz-win95`). It shows a sound-card page that only retro renderers reach, and Lua hooks for detection and disk-space checks.

## Layout

- `installer/installer.toml` and `installer/components.toml` describe the product and what it installs.
- `installer/flows/` holds one flow per lifecycle: install, update, modify, uninstall and launch.
- `installer/layouts/` holds the page layouts the flows name. Each node may carry per-renderer overrides.
- `installer/themes/` holds one folder per theme, each with a `theme.toml` and its assets.
- `installer/dist/` holds tiny payload stand-ins. The `.EXE` and Mach-O files are text, and the Linux executables are shell scripts.

## Checking it

`cargo nextest run -p li-examples-test` loads every example with `li-model` and checks the flows, layouts, themes and payload paths. Open the folder in Studio with `apps/studio` to edit it.

Built-in actions in flows are written `builtin:<name>`: `check-feed` and `launch-app`.
