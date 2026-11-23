# Plume Notes: a Tauri example

Plume Notes is a pretend Tauri 2 app, installed from its own `tauri.conf.json`. It shows `[tauri]`, `liminst import tauri`, WebView2, both Windows modes and per-product updates. Nothing here builds a real app: `installer/dist/` holds stand-ins.

## Layout

- `src-tauri/tauri.conf.json` and `tauri.windows.conf.json` are the fixture. The base asks for `downloadBootstrapper` and an NSIS `installMode` of `both`; the Windows overlay switches WebView2 to `embedBootstrapper` with a minimum version, so the overlay's hash is part of the drift check too. It also has resources, a sidecar, one file association, one deep link and an updater.
- `installer/installer.toml` is the project. `[tauri]` points at the config, `windows = "native"` is the default, and `[updates]` is `both`, with a feed, a channel and a 24 hour check interval.
- `installer/components.toml` holds the generated block between the `liminst import tauri` markers: `tauri-app` for Linux and macOS and `tauri-app-windows` for Windows.
- `installer/tauri.json` is the snapshot, with a hash of each config file.
- `installer/dist/` has a binary, a sidecar and resources for each target, the Tauri setup (`Plume Notes-setup.exe`, for `wrap`) and the WebView2 installers under `dist/webview2/`.

## Try it

```sh
liminst check -C examples/tauri-notes
liminst preview -C examples/tauri-notes --target windows-x86_64 --only plan
```

The check passes with one warning: with a native install the Tauri updater runs Tauri's own setup over the files the engine placed (see [docs/tauri-apps.md](../../docs/tauri-apps.md#updates)).

To see drift, change the version in `src-tauri/tauri.conf.json` and run `liminst check`: it fails with `tauri-drift`. Accept the change with the import (it reads the config `[tauri]` names), which prints what changed and fills `[product] version`:

```sh
liminst import tauri -C examples/tauri-notes
```

To see `wrap`, set `windows = "wrap"` in `[tauri]` and preview again: the plan is the WebView2 step, a copy of the setup and `tauri.wrap`, with none of the app's own files.

`cargo nextest run -p li-engine --test tauri` runs both modes, every WebView2 mode, the update rules and the drift check against a simulated Windows machine.
