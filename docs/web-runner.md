# The web runner

The `web` renderer is a Tauri window that runs an installer project. It is the GUI counterpart of `liminal-engine`: the same engine, flows, layouts, scripts, receipts and rollback, with each page drawn by the React renderer the Studio uses for its previews. [SPEC.md §14](../SPEC.md#14--the-runner) has the commands; this page explains how the pieces fit.

## Parts

| Part | Is |
|---|---|
| `packages/renderer` (`@liminal-hq/installer-renderer`) | The React renderer: the Afterglow, Fizz classic, InstallShield-style, Neutral and Windows 11 looks, the node renderers, the theme stage composition and the status meters. Source only; the Studio and the runner both import it. It has no Studio state, and its few messages are `rendererMessages`, spread into each host's catalogue |
| `crates/li-render-web` | The engine side. `WebUi` is a `li_engine::Ui` that sends each page as an event and waits for the answer; `Session` runs one lifecycle command on its own thread; `run_headless` and `liminal-runner-headless` play the person with no window. It has no Tauri dependency, so its tests run in the ordinary workspace |
| `crates/li-runner` | The lifecycle commands. `execute` takes a `UiProvider`, so the terminal and the window drive the same code |
| `apps/runner` | The Tauri 2 shell and its page. The shell holds no installer logic: it forwards events, answers, cancels and changes |
| `packages/protocol` | The generated types of the events (`RunEvent`, `PageView`, `ProgressView`, `RunInfo`, `PageAnswer`, `PageUpdate`, `ShellError`) |

## The flow

1. The shell reads the command line. `--headless` never opens a window. Otherwise it opens a hidden window, and the page registers its listener and calls `runner_start`.
2. `Session::spawn` runs the command (`li_runner::execute`) on its own thread. The first thing the flow's UI sends is `info`: the product, mode, target, look, window size, the install directory, the components and the theme's stage. The shell sizes the window to the look, centres it and shows it.
3. Each page of the real flow is a `page` event: the layout tree with the theme's stage composed around it, overrides applied and `items = "components"` expanded; the page's values under the keys its controls bind (`install.path`, `components.selected`); the `{placeholder}` values; what validation rejected; and the paths of the nodes `enabled_when` switches off. The flow waits for `runner_answer`.
4. A change on the page calls `runner_update` with the page's values and gets back the new switched-off nodes and values (`install.size`, free space), so Next is a real disabled button until the licence is accepted.
5. The commit step sends a page that does not wait, then `progress` events (throttled to one every 30 ms) that move the meters. `runner_cancel` sets a flag; the next progress call returns false and the engine rolls back.
6. The run ends with `done`, `cancelled` or `failed`. The shell exits with 0, 3 or 1. Closing the window mid-run cancels it and waits for the rollback before exiting.

The engine's `install_dir` and `components` are the page's `install.path` and `components.selected`; `WebUi` maps them both ways.

## Modes

The runner takes the commands of `liminal-engine`:

| Command | Flow | Notes |
|---|---|---|
| `install` | the install flow | Default when no command is given. Finds `payload.lip` beside the runner (a staged bundle) or one folder up (the `.run`'s `engine/`) |
| `update` | the update flow | Verifies the signed feed, as the terminal runner does |
| `modify` | the modify flow, if any | Else only the work runs |
| `repair` | none | The renderer draws its own progress and end screens |
| `uninstall` | the uninstall flow | |

A project with no flow for a mode gets the renderer's own screens, drawn inside the theme's stage. After an install the runner keeps itself, the `.lip` and the pinned key in `{install}/.liminal/runner/`, under its own file name, so later modes need nothing that was not installed.

## Window and look

A target whose renderer is `web` has a look picked by its theme and its stage's `style`: `fizz-modern` or `style = "win11"` is Windows 11, the `neutral` theme is Neutral, anything else is Afterglow. The window opens at the look's stage size (Afterglow and Neutral 1024 by 720, Windows 11 1100 by 720), and the stage scales to the window as it is resized. The table is in `li_render_web::look` and the renderer's `STAGE`, and the two are kept the same.

## Keyboard and accessibility

- Enter presses the page's default button, unless a control has focus; Ctrl+Enter presses it from anywhere, a choice included. Esc presses Cancel on a page (not during the commit, where only the Cancel button cancels). Alt+Left presses Back. A button that is switched off is a disabled `<button>`, so none of these press it.
- A new page puts focus on its first input or choice, else its default button.
- Choices are `role=radio` and `role=checkbox` with `aria-checked`; meters are `role=meter` with values; windows and dialogs carry their label. The main region is named for the product, and a polite live region says each page's title, what validation rejected, and the commit's progress by tenths.
- Every string the runner makes itself is in `apps/runner/src/i18n/messages.ts` and read with `t()`; the project's own pages are its own text.

## Headless and tests

`liminal-runner --headless` and the smaller `liminal-runner-headless` binary (in `li-render-web`, no Tauri) answer every page from `--answer key=value`, as `liminal-engine --headless` does, with one difference: they press Next only if the page allows it. A page whose Next is switched off (a licence not accepted) fails the run with a message naming the answer it needs. `--events FILE` writes every event the window would have got, one JSON object per line.

- `crates/li-render-web/tests/examples.rs` installs the Lt. Fizz (Windows 11) and Planboard (Afterglow) web targets into a scratch `HOME`, checks the look, window, pages, commit page, progress and the licence text, then uninstalls; it also repairs, modifies and updates through a signed feed.
- `apps/runner` tests drive the page with `createTestApi`, a scripted stand-in for the shell: pages, answers, the double press, cancel, the end and the failure, and the keys.

## Building a bundle

```sh
liminst build -t windows-x86_64:web --web-runner path/to/liminal-runner -o build/fizz
```

stages `build/fizz/web/`: the runner (`liminal-runner.exe` for a Windows target), `payload.lip`, `project/installer/…` and `manifest.json` (product, target, look, window, sizes and SHA-256 hashes). A `web` target builds this output by default, and any other output it lists as well; `--output` with names that omit `web` skips it. Without `--web-runner` the folder holds `liminal-runner.placeholder`, which says how to get a runner.

Building the runner is slow and needs the Tauri system libraries (WebKitGTK on Linux), so it is not part of `make validate`:

```sh
bun run runner:build                  # tauri build --no-bundle; writes liminal-runner under the target directory
liminst build -t windows-x86_64:web --build-web-runner   # runs that first, then stages the bundle
bun run runner:dev                    # the shell against the Vite dev server on port 1421
```

`bun run --cwd apps/runner tauri build` makes the platform installers of the runner itself. The runner builds for the platform it runs on; a Windows bundle needs a Windows build of the runner.

## Not done

- The finish page's "launch" choice is drawn and kept in the answers, but the runner does not start the product from it yet.
- A dark or light colour scheme switch is the system's; the runner has no setting for it.
- Free space for a folder the person types is measured when the page is answered, not as each key lands.
