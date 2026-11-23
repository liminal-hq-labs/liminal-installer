# Studio desktop shell

How the Studio window is put together, and how to drive it while developing. `SPEC.md` has the behaviour and `decisions.md` the reasons (D15 to D21).

## What draws what

`apps/studio/ui` is a Vite page. The screens (Overview, Flow, Layouts, Scripts, Dry-run preview) are plain TypeScript. Around them is a small React layer in `src/chrome/` that draws the shared window chrome from `vendor/studio-chrome`.

- `menuModel.ts` declares every command once, with its label, shortcut and enabled state, and builds both the flat app menu and the menu bar from it. The same table answers key presses.
- `chromeStore.ts` holds what the chrome shows (menu state, title, settings) outside React. The settings come from `settings.json` through `read_settings` and `write_settings` (a merge), and every window follows the `studio://settings-changed` event. `src/lib/settings.ts` and `src-tauri/src/settings.rs` hold the same schema and the same limits, and a test fails if they differ.
- `dialogWindow.ts` opens a dialog, waits for its answer and closes it. Preferences is a window of its own (opened with `open_window` and not waited for); About and confirmations are dialogs. In the desktop shell each is a child window: `open_window` in `src-tauri/src/windows.rs` loads the same page with `?window=about`, `?window=preferences` or `?window=confirm`, centred on the main window and modal for About and confirmations. `InfoWindow.tsx` draws it under a title bar with only Close and sends the answer back by event. A browser tab, which has no windows to open, draws the same content in an in-page dialog.
- In a browser tab the page draws its own header and no chrome. In development, add `?chrome` to the URL to draw the chrome there for a look.

The chrome copy is refreshed from Waypoint by the steps in `apps/studio/ui/vendor/studio-chrome/PROVENANCE.md`. The design tokens are regenerated with `bun run tokens` from `src/styles/tokens.json`, the design system export.

## Preferences

`src/preferences/` holds the window's content: `usePreferences.ts` reads the settings, shows a change at once, writes it as a patch, follows `studio://settings-changed`, and on a refusal puts the value back and keeps the reason for the row. `escapeGuard.ts` keeps Escape from closing the window while a box holds text that was typed and not applied: the first Escape puts the old text back and the next closes. `pages.tsx` has one page per section and `sections.tsx` their order. Text boxes and number boxes apply when left or on Enter. `src/lib/host.ts` is what the pages need from the shell besides settings: `app_info`, `reveal` (the settings file or the log folder), `mcp_token` and `mcp_regenerate_token` (the token is fetched only when asked for), the dialog plugin's pickers, and confirmations in a window of their own.

`src/lib/appearance.ts` turns the Appearance settings into `data-theme`, `data-density`, `data-motion` and `--text-scale` on every window. Follow system reads the colour scheme and reduced-motion setting from `tauri-plugin-system-appearance`, as Planboard does, and from the browser's media queries outside the shell and where the plugin has no answer. `tauri-plugin-window-manager` is not used: the Studio has no window of its own to place beyond what `open_window` does.

`src-tauri/src/agent.rs` starts, stops and moves the MCP server from `mcpEnabled` and `mcpPort`. `write_settings` refuses a taken port with the reason and leaves the running server alone. `tools.rs` runs `<tool> --version` for the runner and Docker paths and opens the file manager.

### Where files live

The shell uses Tauri's `app_config_dir()` for the settings and the window sizes, and `app_log_dir()` for logs, under the bundle identifier `hq.liminal.installer-studio`:

| Platform | Config (`settings.json`, `.window-state.json`) | Logs |
|---|---|---|
| Linux | `$XDG_CONFIG_HOME/hq.liminal.installer-studio` (`~/.config/…`) | `$XDG_DATA_HOME/hq.liminal.installer-studio/logs` (`~/.local/share/…`) |
| macOS | `~/Library/Application Support/hq.liminal.installer-studio` | `~/Library/Logs/hq.liminal.installer-studio` |
| Windows | `%APPDATA%\hq.liminal.installer-studio` | `%LOCALAPPDATA%\hq.liminal.installer-studio\logs` |

The Studio writes no log file yet, so the log folder is created when it is first shown. `tauri-plugin-window-state` remembers the size of the `main` and `preferences` windows only; About and confirmations have a fixed size.

## Driving the Studio in dev

Start the shell with the dev config, which turns on the global Tauri object and registers the debug-only MCP bridge:

```bash
bun install
bun run tauri:dev
```

Then connect the Tauri MCP server. `driver_session` with `action: "start"` finds the app (the bridge listens on port 9223, or the next free port, and prints the port in the shell's output). Use `webview_screenshot`, `webview_dom_snapshot`, `webview_keyboard` and `webview_execute_js` with `windowId` set to `main`, `about`, `preferences` or `confirm-<id>`, and `manage_window` with `list` to see which windows are open. For example, press `F1` in `main`, and an `about` window appears for `webview_screenshot` to capture.

What keeps this out of release builds:

- `tauri.conf.json` leaves `withGlobalTauri` unset. Only `tauri.conf.dev.json`, passed by `bun run tauri:dev`, sets it.
- `tauri-plugin-mcp-bridge` is a dependency only off Windows, and `lib.rs` registers it under `#[cfg(all(debug_assertions, not(windows)))]`.
- `capabilities/dev-automation.json` grants `mcp-bridge:default` to the main window and every child window on Linux and macOS, and grants nothing when the plugin is not registered.
- `ui/scripts/dev-automation.test.ts` fails the UI tests if any of those change, or if a child window is added to `windows.rs` without being added to both capability files.

A new kind of child window needs its label in `capabilities/default.json` and `capabilities/dev-automation.json`. The Preferences window keeps the label `preferences`, so both already cover it.
