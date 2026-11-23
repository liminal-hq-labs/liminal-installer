# Planboard example

Planboard installs an app, a command-line tool and documentation on Linux, Windows and macOS, drawn by the `afterglow` theme through the `canvas`, `web` and `tui` renderers.

## Layout

- `installer/installer.toml` and `installer/components.toml` describe the product and what it installs.
- `installer/flows/` holds one flow per lifecycle: install, update, modify, uninstall and launch.
- `installer/layouts/` holds the page layouts the flows name. Each node may carry per-renderer overrides.
- `installer/themes/` holds one folder per theme, each with a `theme.toml` and its assets.
- The layouts in `installer/layouts/` draw the Afterglow design: a dark rounded window with a step list, component cards and an accent progress bar.
- `installer/dist/` holds tiny payload stand-ins. The `.EXE` and Mach-O files are text, and the Linux executables are shell scripts.

## Checking it

`cargo nextest run -p li-examples-test` loads every example with `li-model` and checks the flows, layouts, themes and payload paths. Open the folder in Studio with `apps/studio` to edit it.

Built-in actions in flows are written `builtin:<name>`: `check-feed` and `launch-app`.
