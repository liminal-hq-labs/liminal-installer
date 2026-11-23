# Neutral example

Neutral is the smallest example: the default theme, two components and no per-theme pages.

## Layout

- `installer/installer.toml` and `installer/components.toml` describe the product and what it installs.
- `installer/flows/` holds one flow per lifecycle: install, update, modify, uninstall and launch.
- `installer/layouts/` holds the page layouts the flows name. Each node may carry per-renderer overrides.
- `installer/themes/` holds one folder per theme, each with a `theme.toml` and its assets.
- `installer/themes/installshield-classic/` is a product-agnostic InstallShield-style theme for `win32-classic` and `win16`: a blue gradient backdrop, a bevelled setup dialog with a side panel, sunken lists and the status meters. Copy it to give a project the classic setup look.
- `installer/dist/` holds tiny payload stand-ins. The `.EXE` and Mach-O files are text, and the Linux executables are shell scripts.

## Checking it

`cargo nextest run -p li-examples-test` loads every example with `li-model` and checks the flows, layouts, themes and payload paths. Open the folder in Studio with `apps/studio` to edit it.

Built-in actions in flows are written `builtin:<name>`: `check-feed` and `launch-app`.
