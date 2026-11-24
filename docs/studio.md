# Studio desktop shell

How the Studio window is put together, and how to drive it while developing. `SPEC.md` has the behaviour and `decisions.md` the reasons (D15 to D38).

## What draws what

`apps/studio` is the house layout: `index.html`, `vite.config.ts` and `src/` (React 19) with `src-tauri/` beside them. The shared window chrome is the workspace package `packages/chrome`. The Studio's own folders:

- `src/app/` has `startApp.tsx` (mounts the app, returns the `App` a shell drives), `Shell.tsx` (sidebar, status bar, history popover, toast), `router.tsx` (the seven routes on memory history) and `StudioContext.tsx` (the hooks that read the stores).
- `src/stores/` has the zustand stores (project, preview, script draft, toasts) and `studio.ts`, where `createStudio` holds the actions. The backend arrives as `api`, so `src/app/*.test.tsx` runs the whole app against a fake one under happy-dom.
- `src/screens/` has Project, Flow, Layouts, Scripts, Dry-run, Tauri and Media (with its parts in `src/screens/media/`), each with its CSS Module. `src/preview/` draws a layout tree in the four looks. `src/ui/` has the shared CSS Modules (cards, buttons, fields, text colours).
- `src/chrome/` is the layer around the chrome package, described below. `src/i18n/` is the message catalogue. `src/theme/` has the tokens. `src/services/logger.ts` forwards `console.*` to `tauri-plugin-log`.

Imports of the chrome are by deep path (`@liminal-hq/studio-chrome/TitleBar`), never a barrel.

- `menuModel.ts` declares every command once, with its label message, shortcut and enabled state, and builds both the flat app menu and the menu bar from it. The same table answers key presses.
- `chromeStore.ts` holds what the chrome shows (menu state, title, settings) in a zustand store. The settings come from `settings.json` through `read_settings` and `write_settings` (a merge), and every window follows the `studio://settings-changed` event. `src/lib/settings.ts` and `src-tauri/src/settings.rs` hold the same schema and the same limits, and a test fails if they differ.
- `dialogWindow.ts` opens a dialog, waits for its answer and closes it. Preferences is a window of its own (opened with `open_window` and not waited for); About and confirmations are dialogs. In the desktop shell each is a child window: `open_window` in `src-tauri/src/windows.rs` loads the same page with `?window=about`, `?window=preferences` or `?window=confirm`, centred on the main window and modal for About and confirmations. `InfoWindow.tsx` draws it under a title bar with only Close and sends the answer back by event. A browser tab, which has no windows to open, draws the same content in an in-page dialog.
- Keyboard shortcuts, Connect an agent and New project are child windows of their own (`shortcuts`, `agent`, `new-project` in `windows.rs` and both capability files). Their bodies are in `DialogPages.tsx`. Keyboard shortcuts is built from `shortcutTable()` in `menuModel.ts`. Connect an agent reads `mcp_info`, `mcp_error` and `mcp_token`. New project calls `new_project`, which runs the `li_studio::scaffold` code `liminst init` also uses, then answers `created:<folder>` so the main window opens it.
- `App.dispatch` and `App.openPath` resolve once the action is done, with true on success. `main.tsx` awaits a save before it goes on to quit, open or reload, and stops when the save fails.
- `src/app/startApp.test.tsx` and `scriptDraft.test.tsx` run `startApp` against a fake `Api`. `src-tauri/src/api_contract.rs` parses the UI sources and checks that every `invoke` and `call` names a registered command and sends exactly its argument names.
- In a browser tab the page draws its own header and no chrome. In development, add `?chrome` to the URL to draw the chrome there for a look.

The chrome copy is refreshed from Waypoint by the steps in `packages/chrome/PROVENANCE.md`. The design tokens are regenerated with `bun run tokens` from `src/theme/tokens.json`, the design system export, into `src/theme/tokens.css` (`--lis-*`), followed by `src/theme/chrome-map.css`, which maps the chrome's `--lic-*` variables and the editor's syntax colours onto them.

## Messages and locales

Every visible string is in `src/i18n/messages.ts` and read with `t('area.key')` (typed keys, `{name}` tokens) or `tn('area.key', count)` (a `.one` and an `.other` message). A comment `// translator:` above a key is context for whoever translates it. `en-CA` is the source. Add `?locale=en-XA` (accented, 30% longer, in brackets: text that is not from the catalogue shows up unmarked, and clipped text loses its closing bracket) or `?locale=ar-XB` (right to left) to the address of a development server to look at the layout; a release build is always English. The pseudo-locales are generated in `src/i18n/catalogues/`.

`bun run check:i18n` (`apps/studio/scripts/check-i18n.ts`, part of `make validate` and the `Messages` job in `.github/workflows/ci.yml`) fails when a component has a literal in JSX text or in `title`, `label`, `placeholder`, `alt`, `aria-label`, `description`, `actionLabel` or `moreLabel`; when a pseudo-locale has other keys or `{tokens}` than English; when a `.one` has no `.other`; or when a message is never used. Test files are exempt. Text that a backend or a reference owns is not in the catalogue: the flow validation messages that mirror `Flow::validate`, the browser demo's sample data and the script API reference in `src/editor/luaDocs.ts`.

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

`tauri-plugin-log` writes a log file in the log folder (Trace in a debug build, Info in a release build) and takes the webview's `console` output through `src/services/logger.ts`, prefixed with the window label. `tauri-plugin-window-state` remembers the size of the `main` and `preferences` windows only; About and confirmations have a fixed size.

## The script editor

`src/editor/` holds the Scripts screen's editor. `scriptEditor.ts` creates one CodeMirror view, and `src/screens/ScriptEditorView.tsx` mounts it: it creates the view once when the screen shows a script, sends typing out through `studio.setDraft`, takes a draft that changed from outside (discard, reload) back in, and applies the Editor settings without a new view. The view is made again when the screen is shown again or another script is opened (so the undo history starts afresh), with the draft kept in the script store. `luaLanguage.ts` (colouring from the legacy Lua mode, folding by block) and `theme.ts` use `--lis-*` design tokens and the `--lis-ed-*` syntax tokens in `chrome-map.css`, so the theme changes live. `luaDocs.ts` writes the script API once; `completion.ts`, `hover.ts` and `docElement.ts` read it. `diagnostics.ts` asks `checkScriptSyntax` after a pause and maps lines and columns to spans. `format.ts` is the formatter, over the scanner in `luaLex.ts`. `phrases.ts` maps the editor library's own English phrases to catalogue messages.

The text is `scriptDraft` in the script store; the actions that save, discard, select and test-run are in `src/stores/studio.ts`. `startApp` takes an optional `api` and `settleUnsaved`, which the tests use with a fake backend (`src/app/scriptDraft.test.tsx`). The browser demo's `checkScriptSyntax` is a stand-in that balances blocks; the desktop shell compiles with Lua.

## Driving the Studio in dev

The dev server is Vite on port 1420 with `strictPort`, and `devUrl` in `tauri.conf.json` names the same port; `scripts/dev-server.test.ts` fails if they differ. Vite does not watch `src-tauri`.

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
- `scripts/dev-automation.test.ts` fails the UI tests if any of those change, or if a child window is added to `windows.rs` without being added to both capability files.

A new kind of child window needs its label in `capabilities/default.json` and `capabilities/dev-automation.json`. The Preferences window keeps the label `preferences`, so both already cover it.

## How layouts are drawn

`src/preview/PreviewTree.tsx` turns a layout tree into React elements in one of five looks (`src/preview/model.ts` picks the look and holds the sample values): `dos` (`tui-retro` and `tui`), `win95` (`win32-classic` and `win16`), and `afterglow`, `neutral` and `win11` (`canvas` and `web`, by theme: `neutral`, `fizz-modern` or a root `style`). The three modern looks carry the `.pvModern` class, which holds the tokens they share (radii, shadow, spacing, easing), and fill in the same colour names (`--bg`, `--panel`, `--text`, `--accent`, `--track` and so on), so the shared rules draw their pages; `win11` overrides what Fluent does differently. The Layouts screen and the Dry-run screen both draw it, so they cannot drift. `src/preview/preview.module.css` holds the looks; each stage is composed at its artboard's pixel size (960 by 600 for DOS, 1024 by 768 for Windows 95, 1100 by 720 for Windows 11, 1024 by 720 for the others) and the stage box scales it to the card. The DOS look uses VT323 from `@fontsource/vt323`, imported in `main.tsx`, because the page's content security policy allows no remote fonts.

Dry-run draws from `layout_tree`, `state`, `vars`, `renderer` and `theme` in the preview, and choosing a target runs the preview again. Picture art is the node's own `art` property drawn as inline SVG. A `tui` target also gets the text version, under the screen. The browser demo (`demoApi.ts`) builds the same payload from the `lt-fizz` example.

A theme's own layouts, in `themes/<id>/layouts/`, show in the Layouts screen's picker under the theme, and the preview uses a renderer that theme supports. The looks are checked against the artboards in `Fizz-DOS`, `Fizz-Win95`, `Planboard-Afterglow` and `Neutral-Default`; the Windows 11 look is checked against Fluent and against Afterglow, so the two modern looks read as relatives.

The `win11` look (`Win11Frame`) draws the stage's `backdrop` as the wallpaper, the wordmark and the credit, the page's `window` as the Mica window, its `side` region as the rail (a pixel-art tile and the step list, where the current step has the accent bar), `region = "status"` meters as the status band when a theme has them (`fizz-modern` has none; its copying page has a slim `progress` node), and the footer as the command bar. Its colours are tokens on `.pvWin11`, light by default and dark under `data-scheme="dark"` or, for `data-scheme="system"`, under `prefers-color-scheme: dark`. The type is Segoe UI Variable with `system-ui` behind it; no font is bundled. The scheme (`previewScheme`: `system`, `light` or `dark`) is held in the preview store, so the Layouts screen, the Dry-run screen, the in-page full screen and the full-screen window (`&scheme=…`) agree, and the Colours switch is shown only for a look with both.

### Stages, the whole screen and the palette

A theme's `stage` layout (`SPEC.md` section 6) holds the backdrop, wordmark, status meters and taskbar every page of the theme shares, and its pages are only a `dialog` or `window`. `src/lib/stage.ts` composes the same screen the Rust side does (`Layout::composed`): `composeScreen` returns the tree and a map from each composed node to its layout (`stage` or `page`) and path, `screenRows` lists it for the tree, and `detachStage` and `attachStage` give a page its own copy of the stage or take it back. The Layouts screen draws and edits that whole screen. Stage rows are tinted and tagged "Theme"; selecting one shows a notice with the number of pages a change reaches and a "Make this page's own copy" button. The project store keeps `stageLayout` and `layoutScope` beside `layout` and `layoutPath`, and `saveLayout` writes whichever layout an edit belongs to.

`src/lib/palette.ts` is the add-node palette: every node kind in groups, each with a hint, plus presets (button row, side art, billboard, checklist, radio, field, path, step progress, key hints, the three status meters). A preset has a default scope, the stage for meters and the page otherwise, and the Add control lets the person choose. The inspector shows a meter's source, label, warning level and placement as fields.

### Clicking and running the preview

`PreviewTree` takes an `interact` option. In `edit` mode (the Layouts screen) every element that comes from a node carries `data-node-path`, one delegated click handler selects the nearest one, the pointer's node is outlined (`data-hover`) and the selected node too (`data-selected`). In `run` mode (the Dry-run screen and the full-screen view) the controls answer: `useRun` (`src/preview/useRun.ts`) holds the choices over the step's state, `sim.ts` turns the chosen components and folder into the space, plan summary and free-space meter (sample sizes: 16 MB per file, and sample drives `C:` roomy, `D:` nearly full, `A:` a floppy), and `usePlayback.ts` is the clock for the Play, Pause, Reset and speed controls that animate the copy meters and the billboard.

Full screen is the `preview` child window (`PreviewWindow.tsx`, `FullScreenBody.tsx`). `open_window` opens it from View > Full-screen preview (`F11` on the Dry-run screen) with `?window=preview&target=…&step=…` (and `&view=terminal` when the Dry-run screen was showing the real terminal, so the window opens on the step, target and view that were on show); it sizes itself with `fitWindow` to the look's natural stage size, scaled to the screen with the aspect ratio kept, and has the stepper, target selector and play controls. F11 or its button toggles real full screen, and Escape leaves full screen and then closes. For `tui` and `tui-retro` targets the bar also has the View switch (Rich or Terminal (real)), which is held in the preview store (`previewView`) so the Dry-run screen, the in-page view and the window agree. In the terminal view the stage is an 80 by 25 cell session drawn at the largest whole-number zoom that fits the window (`terminalFit`), centred and never stretched, scaled down only when the window is smaller than one cell at natural size; the session is not resized to the window, so the DOS look stays 80 by 25. A chosen Next, Back or Cancel moves through the flow as in the Dry-run screen. **Keys:** while the terminal has focus (it takes focus when it opens) every key it forwards, Esc included, belongs to the installer, so Esc cancels back to the first step; F6 leaves the terminal, and then Esc leaves full screen and closes the window as before. The window and the in-page view ignore a key that was already handled. `preview` is in both capability files, with the window size and full-screen permissions in `default.json`. A browser tab, with no windows to open, shows the same view fixed over the page and asks the browser to go full screen too. The browser demo answers the terminal calls with a framed stand-in screen (`demoTerminal.ts`), so the view can be tried without the renderer.

## The real terminal view

For `tui` and `tui-retro` targets the Dry-run and Layouts screens have a View switch: Rich (the HTML drawing above) or Terminal (real), which is the `tui` renderer's own output. The renderer runs in Rust. `li_render_tui::render_cells` returns a `CellGrid` (one `Cell` per terminal cell: `ch`, `fg`, `bg`, modifiers) and `to_ansi` writes it as ANSI text. `li_studio::tui` keeps live sessions: `Studio::tui_session_open`, `tui_session_key`, `tui_session_resize` and `tui_session_close`, exposed as the Tauri commands `tui_session_open`, `tui_session_key`, `tui_session_resize` and `tui_session_close` (app commands need no capability entries) and, for agents, the MCP tools `tui_render` and `tui_key`. A session holds the `Screen` and the page state, so the UI keeps none: it sends a key and draws the frame it gets back. `StudioBackend::tui_page` resolves the page as the preview does, in `EngineBackend`, with the theme's stage composed around it by `li_engine::Project::screen_for` (the stage's `slot`, `show_on` and `hide_on`), so the terminal shows the same whole screen as the rich view; the tui renderer draws a stage's `backdrop` and its meters, and a node it has no text form for (such as art) is left out. A stage layout opened on its own is drawn alone, and the Layouts terminal reopens after an edit to the layout or its stage. In Dry-run the session starts from the step's page state, and it is not tied to the interactive `useRun` state, so choices made in the rich view do not carry over.

`src/preview/TerminalPreview.tsx` draws the grid on a canvas and forwards keys; `useTuiSession.ts` owns the session (open, ordered keys, close on change or unmount); `terminalPaint.ts` paints. Box-drawing and block characters are drawn as rectangles, not font glyphs, so frames meet without gaps at any size, and every other character is drawn at the centre of its cell in VT323 with a monospace fallback. The 16 ANSI colours are the VGA ones and the default pair is a dark terminal. A hidden `<pre>` and a live region carry the screen as text and the focused control for screen readers. Tab belongs to the screen, so F6 leaves the terminal. In Dry-run, a chosen Next or any other button goes to the next step, Back to the previous, and Cancel or Esc back to the first. The browser demo draws a stand-in screen, not the renderer's output. `api.ts` has `TuiApi` for the four calls, and `noTerminal` for fakes.

## Media screen

The Media screen (View > Media, `Ctrl+7`, `/media`) is where a person sees and edits the disks and discs a project ships on. `SPEC.md` section 13 has the behaviour, the warning codes and the rules; D83 to D85 have the reasons. It is built in layers, each of which can be replaced without touching the one above.

- **Wire types** are in the `li-media` crate (`MediaInfo`, `MediaEdit`, `MediaPlan`, `VolumePlan`, `DiskEntry`, `ComponentSize`, `MediaWarning` and the rest), derive `ts_rs::TS` behind the `ts` feature and are generated into `packages/protocol`. `li-media` has no UI dependency, so `liminst`, the Studio and the MCP server will all call its planner.
- **The `Api`** has `listMedia`, `getMediaPlan`, `addMedia`, `setMedia`, `removeMedia` and `pickReadme`. Until the planner exists the browser demo answers them from `src/lib/demoMedia.ts`, which plans a medium from the Quill sizes in `src/lib/mediaFixture.ts`, and the desktop shell answers with a "not available yet" error, the way `noTerminal` does for the terminal view. A test fake in `src/lib/fakeMedia.ts` keeps its own undo stack.
- **The store** (`src/stores/media.ts`) holds the media, their plans, the one picked and the tab. An edit is one `setMedia` call with a label, so it is one undoable history entry; the store reads the plan again afterwards, which is how a switch's free-space change reaches the screen.
- **The screen** (`src/screens/MediaScreen.tsx`, with `src/screens/media/`) draws the list, the capacity meter and the tabs.

### What it looks like

The screen has a header with the title, one line of help, Add floppy and Add CD. Below it are two columns. The narrow left one is the media list, one row per medium with its kind, id, label, a pill that says fits, warning or does not fit, the free bytes and "N disks" when it spans. With no media it holds an empty state that explains what a medium is and offers both Add buttons. The wide right column is the picked medium: a heading with its kind, label and capacity, then the capacity card, then the tabs.

The capacity card has the meter, a row of totals (used, common pack, free, reserve) and a second thin bar for "Without sharing" with the bytes it would need and by how much it would overflow. Both bars are rows of block cells in VT323 (`█` used, `▓` common pack, `▒` reserve, `░` free), one per 16 KiB on a floppy, and are hidden from assistive technology; the meter's own text carries the numbers. A CD shows a plain bar with 80 cells.

- **Disk map.** A tree. The root is the volume (`A:\ QUILL`, with its serial), then `SETUP.EXE`, `SETUP.LPK`, `COMMON.LPK` (which opens into its `Cnnnnnnn` entries), `README.TXT` and a folder per target with its own files. Each row shows its size in whole clusters (512 bytes on a floppy, 2048 on a CD) and a role. A details pane beside it shows the picked row's raw, packed and on-disk sizes, its pack and chunk size, its targets and its component. A medium that spans shows one tree per disk side by side, each with its own meter.
- **Sizes.** A table of components (required or switchable, raw, packed, pack, disk, and what the switch would change) and a table of files, both sortable by clicking a heading. A card above them sets the reserve and what happens on overflow.
- **Targets.** A row per retro target of the project: whether it is on this medium, its folder, where its `SETUP.EXE` came from ("found at …" or "estimated: no runtime was found") and its size.
- **Label and README.** The label with a live count against 11 and the rule, the serial (automatic or eight hex digits), on a CD the derived ISO volume id and Joliet label, and the README text in a 76-column box with a line-width and CRLF check, with a button to use a file of one's own.
- **Autorun.** On a CD, the `AUTORUN.INF` text, the target it opens and its icon. Elsewhere, or with no Windows 95 target on the medium, the tab says why it is off.
- **Warnings.** Under the capacity card, one row per warning with its severity and message; a row that has a tab to fix it is a button that opens it.

### Keys and accessibility

- The meter is `role="meter"` with `aria-valuemin`, `aria-valuemax`, `aria-valuenow` and an `aria-valuetext` such as "1,286,656 of 1,457,664 bytes used; 171,008 free; reserve met".
- The disk map is an ARIA `tree`: Up and Down move, Right opens a branch or enters it, Left closes it or goes to the parent, Home and End go to the ends, Enter or Space picks a row for the details pane, and only one row is in the tab order.
- The tabs are a `tablist` that Left and Right (mirrored in right-to-left text) and Home and End move through. The list is a `listbox`; Delete on a row asks to remove it.
- A component's switch is `role="switch"`, and its note, which says what flipping it does, is its `aria-describedby`. A table's sorted heading carries `aria-sort`. The motion setting turns off the one transition the meter has.
- Remove asks in the confirmation child window, never an overlay, with Remove as a danger button and Cancel as the safe one.

### What is not built

Nothing here builds, writes a disk or starts an emulator. Build jobs, the `studio://media-build` events, results with checksums, the emulator buttons, writing to a physical floppy (a `write-floppy` child window that lists only removable 1.44 MB disks, never picks one and asks for the device and the label to be typed) and the Dry-run "Run from" controls are the later steps of the plan in D85. The MCP media tools answer one request at a time, so a client reads a build's progress by asking again.

### Testing

`src/app/mediaScreen.test.tsx` runs the app over the test fake and the Quill fixture: add and remove (with the confirmation), undo and redo, a switch changing the free space, the label rule, the spanning view, the tree's keys and the warnings' links. Screenshots of each tab in `en-CA`, `en-XA` and `ar-XB` come from the dev server in the browser demo (`?locale=`), or from the Tauri MCP when it connects.
