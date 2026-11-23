# Studio desktop shell

How the Studio window is put together, and how to drive it while developing. `SPEC.md` has the behaviour and `decisions.md` the reasons (D15 to D28).

## What draws what

`apps/studio/ui` is a Vite page. The screens (Overview, Flow, Layouts, Scripts, Dry-run preview) are plain TypeScript. Around them is a small React layer in `src/chrome/` that draws the shared window chrome from `vendor/studio-chrome`.

- `menuModel.ts` declares every command once, with its label, shortcut and enabled state, and builds both the flat app menu and the menu bar from it. The same table answers key presses.
- `chromeStore.ts` holds what the chrome shows (menu state, title, settings) outside React. The settings come from `settings.json` through `read_settings` and `write_settings` (a merge), and every window follows the `studio://settings-changed` event. `src/lib/settings.ts` and `src-tauri/src/settings.rs` hold the same schema and the same limits, and a test fails if they differ.
- `dialogWindow.ts` opens a dialog, waits for its answer and closes it. Preferences is a window of its own (opened with `open_window` and not waited for); About and confirmations are dialogs. In the desktop shell each is a child window: `open_window` in `src-tauri/src/windows.rs` loads the same page with `?window=about`, `?window=preferences` or `?window=confirm`, centred on the main window and modal for About and confirmations. `InfoWindow.tsx` draws it under a title bar with only Close and sends the answer back by event. A browser tab, which has no windows to open, draws the same content in an in-page dialog.
- Keyboard shortcuts, Connect an agent and New project are child windows of their own (`shortcuts`, `agent`, `new-project` in `windows.rs` and both capability files). Their bodies are in `DialogPages.tsx`. Keyboard shortcuts is built from `shortcutTable()` in `menuModel.ts`. Connect an agent reads `mcp_info`, `mcp_error` and `mcp_token`. New project calls `new_project`, which runs the `li_studio::scaffold` code `liminst init` also uses, then answers `created:<folder>` so the main window opens it.
- `App.dispatch` and `App.openPath` resolve once the action is done, with true on success. `main.ts` awaits a save before it goes on to quit, open or reload, and stops when the save fails.
- `src/app.test.ts` runs `startApp` against a fake `Api` under happy-dom. `src-tauri/src/api_contract.rs` parses the UI sources and checks that every `invoke` and `call` names a registered command and sends exactly its argument names.
- In a browser tab the page draws its own header and no chrome. In development, add `?chrome` to the URL to draw the chrome there for a look.

The chrome copy is refreshed from Waypoint by the steps in `apps/studio/ui/vendor/studio-chrome/PROVENANCE.md`. The design tokens are regenerated with `bun run tokens` from `src/styles/tokens.json`, the design system export.

## Typed bindings and errors

The Rust types that cross the Tauri boundary or the MCP API (`Flow`, `Layout`, `Node`, `Plan`, `Operation` from `li-model`; `Preview`, `Problem`, `ScriptReport`, `ProjectSummary`, `ChangeEvent`, `ErrorPayload` and the rest from `li-studio`; `ScriptDiagnostic` from `li-script`) derive `ts_rs::TS` and are written to `packages/protocol/src/generated/`, the Bun workspace package `@liminal-hq/installer-protocol`. `li-model` and `li-script` hold the derive behind a `ts` feature that `li-studio` turns on, so the runner never builds ts-rs. `.cargo/config.toml` sets `TS_RS_EXPORT_DIR`; `bash scripts/generate-protocol-bindings.sh` regenerates the files and `bash scripts/check-bindings-drift.sh` (the `Bindings drift` job, and `make bindings`) deletes them, regenerates and fails on a difference. `src/lib/types.ts` re-exports them, and nothing under `src` declares a type that Rust sends. The shell's own types, `Settings` and `AppInfo`, are not generated: `settings.ts` and `settings.rs` stay in step by the test described above.

Every command returns `Result<T, StudioError>`. `StudioError` serialises as `{ kind, message }` (`ErrorPayload` and `ErrorKind` in the bindings), and the MCP server uses the same object (see `SPEC.md`). `src/lib/api.ts` parses a rejection into a `StudioError` class (an `Error` with `kind`) in `invokeCommand`, which `api.ts`, `host.ts` and `settings.ts` call; a string, a plain `Error` or an unknown kind becomes `internal`. Toasts show `errorMessage(e)`, and a caller branches with `isStudioError(e, kind)`: opening a project that fails with `invalid_input` stays in the recent list so it can be fixed, and any other failure drops it.

## Preferences

`src/preferences/` holds the window's content: `usePreferences.ts` reads the settings, shows a change at once, writes it as a patch, follows `studio://settings-changed`, and on a refusal puts the value back and keeps the reason for the row. `escapeGuard.ts` keeps Escape from closing the window while a box holds text that was typed and not applied: the first Escape puts the old text back and the next closes. `pages.tsx` has one page per section and `sections.tsx` their order. Text boxes and number boxes apply when left or on Enter. `src/lib/host.ts` is what the pages need from the shell besides settings: `app_info`, `reveal` (the settings file or the log folder), `mcp_token` and `mcp_regenerate_token` (the token is fetched only when asked for), the dialog plugin's pickers, and confirmations in a window of their own.

`src/lib/appearance.ts` turns the Appearance settings into `data-theme`, `data-density`, `data-motion` and `--text-scale` on every window. Follow system reads the colour scheme and reduced-motion setting from `tauri-plugin-system-appearance`, as Planboard does, and from the browser's media queries outside the shell and where the plugin has no answer. `tauri-plugin-window-manager` is not used: the Studio has no window of its own to place beyond what `open_window` does.

`src-tauri/src/agent.rs` starts, stops and moves the MCP server from `mcpEnabled` and `mcpPort`. `write_settings` refuses a taken port with the reason and leaves the running server alone. `liminst studio --mcp` (in `crates/li-cli`) runs the same `McpServer` with no window, over an `EngineBackend` studio opened on `-C`. It prints `Studio MCP server: <url>` and `Token: <token>` on the first two lines of standard output and waits for Ctrl+C; the history is in memory and writes go to disk. A failed start of the desktop shell's server is kept by `agent.rs` as the reason shown on the agent card and in Preferences under Agents.

`tools.rs` runs `<tool> --version` for the runner and Docker paths and opens the file manager.

### Where files live

The shell uses Tauri's `app_config_dir()` for the settings and the window sizes, and `app_log_dir()` for logs, under the bundle identifier `ca.liminalhq.installerstudio`:

| Platform | Config (`settings.json`, `.window-state.json`) | Logs |
|---|---|---|
| Linux | `$XDG_CONFIG_HOME/ca.liminalhq.installerstudio` (`~/.config/…`) | `$XDG_DATA_HOME/ca.liminalhq.installerstudio/logs` (`~/.local/share/…`) |
| macOS | `~/Library/Application Support/ca.liminalhq.installerstudio` | `~/Library/Logs/ca.liminalhq.installerstudio` |
| Windows | `%APPDATA%\ca.liminalhq.installerstudio` | `%LOCALAPPDATA%\ca.liminalhq.installerstudio\logs` |

The identifier was `hq.liminal.installer-studio` before the Studio joined the Liminal HQ house pattern (`ca.liminalhq.<app>`, no separators), and the directories above are named after it. On the first run under the new identifier, `settings::migrate_legacy_for` copies `settings.json` and `.window-state.json` from the old config directory when the new one has no `settings.json`; the old directory is left alone, and logs are not carried over.

The Studio writes no log file yet, so the log folder is created when it is first shown. `tauri-plugin-window-state` remembers the size of the `main` and `preferences` windows only; About and confirmations have a fixed size.

## The script editor

`src/editor/` holds the Scripts screen's editor, written without React. `scriptEditor.ts` creates one CodeMirror view and keeps it alive: `views/script.ts` moves its element into each redraw and restores focus. `luaLanguage.ts` (colouring from the legacy Lua mode, folding by block) and `theme.ts` use `--lh-*` design tokens and the `--ed-*` syntax tokens in `app.css`, so the theme changes live. `luaDocs.ts` writes the script API once; `completion.ts`, `hover.ts` and `docElement.ts` read it. `diagnostics.ts` asks `checkScriptSyntax` after a pause and maps lines and columns to spans. `format.ts` is the formatter, over the scanner in `luaLex.ts`. `phrases.ts` is the one catalogue of the editor's strings, with `setLanguage` to replace them.

The text is `state.scriptDraft`; `lib/scriptActions.ts` owns saving, discarding, selecting and test runs, and `app.ts` only wires them. `startApp` takes an optional `api` and `settleUnsaved`, which the tests use with a fake backend (`src/app.script.test.ts`). The browser demo's `checkScriptSyntax` is a stand-in that balances blocks; the desktop shell compiles with Lua.

## Driving the Studio in dev

The dev server is Vite on port 1420 with `strictPort`, and `devUrl` in `tauri.conf.json` names the same port; `ui/scripts/dev-server.test.ts` fails if they differ. Vite does not watch `src-tauri`.

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

## How layouts are drawn

`src/lib/previewRender.ts` turns a layout tree into HTML in one of four looks: `dos` (`tui-retro` and `tui`), `win95` (`win32-classic` and `win16`), `afterglow` and `neutral` (`canvas` and `web`, by theme). The Layouts screen and the Dry-run screen both call it, so they cannot drift. `src/styles/preview.css` holds the looks; each stage is composed at its artboard's pixel size (960 by 600 for DOS, 1024 by 768 for Windows 95, 1024 by 720 for the others) and `fitStages` scales it to the card. The DOS look uses VT323 from `@fontsource/vt323`, imported in `main.ts`, because the page's content security policy allows no remote fonts.

Dry-run draws from `layout_tree`, `state`, `vars`, `renderer` and `theme` in the preview, and choosing a target runs the preview again. Picture art is the node's own `art` property drawn as inline SVG. A `tui` target also gets the text version, under the screen. The browser demo (`demoApi.ts`) builds the same payload from the `lt-fizz` example.

A theme's own layouts, in `themes/<id>/layouts/`, show in the Layouts screen's picker under the theme, and the preview uses a renderer that theme supports. The looks are checked against the artboards in `Fizz-DOS`, `Fizz-Win95`, `Planboard-Afterglow` and `Neutral-Default`.
