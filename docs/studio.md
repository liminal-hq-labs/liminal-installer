# Studio desktop shell

How the Studio window is put together, and how to drive it while developing. `SPEC.md` has the behaviour and `decisions.md` the reasons (D15 to D18).

## What draws what

`apps/studio/ui` is a Vite page. The screens (Overview, Flow, Layouts, Scripts, Dry-run preview) are plain TypeScript. Around them is a small React layer in `src/chrome/` that draws the shared window chrome from `vendor/studio-chrome`.

- `menuModel.ts` declares every command once, with its label, shortcut and enabled state, and builds both the flat app menu and the menu bar from it. The same table answers key presses.
- `chromeStore.ts` holds what the chrome shows (menu state, title, settings) outside React. The `menuBar` and `theme` settings come from `settings.json` in the app's config directory through `read_settings` and `write_settings`, and every window follows the `studio://settings-changed` event.
- `dialogWindow.ts` opens a dialog, waits for its answer and closes it. In the desktop shell each dialog is a child window: `open_window` in `src-tauri/src/windows.rs` loads the same page with `?window=about`, `?window=preferences` or `?window=confirm`, centred on the main window and modal for About and confirmations. `InfoWindow.tsx` draws it under a title bar with only Close and sends the answer back by event. A browser tab, which has no windows to open, draws the same content in an in-page dialog.
- In a browser tab the page draws its own header and no chrome. In development, add `?chrome` to the URL to draw the chrome there for a look.

The chrome copy is refreshed from Waypoint by the steps in `apps/studio/ui/vendor/studio-chrome/PROVENANCE.md`. The design tokens are regenerated with `pnpm tokens` from `src/styles/tokens.json`, the design system export.

## Driving the Studio in dev

Start the shell with the dev config, which turns on the global Tauri object and registers the debug-only MCP bridge:

```bash
cd apps/studio/ui
pnpm install
pnpm tauri:dev
```

Then connect the Tauri MCP server. `driver_session` with `action: "start"` finds the app (the bridge listens on port 9223, or the next free port, and prints the port in the shell's output). Use `webview_screenshot`, `webview_dom_snapshot`, `webview_keyboard` and `webview_execute_js` with `windowId` set to `main`, `about`, `preferences` or `confirm-<id>`, and `manage_window` with `list` to see which windows are open. For example, press `F1` in `main`, and an `about` window appears for `webview_screenshot` to capture.

What keeps this out of release builds:

- `tauri.conf.json` leaves `withGlobalTauri` unset. Only `tauri.conf.dev.json`, passed by `pnpm tauri:dev`, sets it.
- `tauri-plugin-mcp-bridge` is a dependency only off Windows, and `lib.rs` registers it under `#[cfg(all(debug_assertions, not(windows)))]`.
- `capabilities/dev-automation.json` grants `mcp-bridge:default` to the main window and every child window on Linux and macOS, and grants nothing when the plugin is not registered.
- `ui/scripts/dev-automation.test.ts` fails the UI tests if any of those change, or if a child window is added to `windows.rs` without being added to both capability files.

A new kind of child window needs its label in `capabilities/default.json` and `capabilities/dev-automation.json`.
