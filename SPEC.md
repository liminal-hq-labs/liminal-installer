# Liminal Installer specification

The file formats, operations and interfaces behind the [architecture](docs/architecture.md). Examples use the Lieutenant Fizz project. The CLI is called `liminst`. When this document and the code disagree, decide which is right and fix the other in the same change.

## Status

Everything in this specification is in progress. Nothing is released, and any part may change between commits.

- **Engine** (plan, stage, commit, record): in progress. Install, update, modify, repair, uninstall and launch work end to end on Linux through the runner (§14).
- **CLI** (`check`, `preview`, `build`, `keygen`, `feed`, `theme`, `studio`): in progress. `check`, `preview`, the Linux `build` (themed `.run` and `.lip`) and the DOS `build` (`SETUP.DAT`, `zip`, `floppy-1440`) work, and feeds and themes are signed.
- **Renderers**: `tui` is in progress; `tui-retro` works for DOS (`SETUP.EXE` in Rust, see [docs/retro-dos.md](docs/retro-dos.md)); `win16`, `win32-classic`, `canvas` and `web` are in progress as scaffolds or designs.
- **Studio** and its MCP server: in progress. GraphQL is deferred (D26).
- **Example installers** for Planboard and Lt. Fizz: in progress.
- **Feeds, the web stub and the Hub**: in progress.
- **Delta updates, a theme gallery and macOS notarization in the CI step**: in progress as designs only.

## 1 · Conventions

- Hand-written files are TOML. Files the Studio writes (flows, layouts) are JSON, each with a `$schema` and a `version`.
- Paths in a project use `/` and are relative to `installer/`. Install-time paths use placeholders: `{install}`, `{user_data}`, `{system_data}`, `{start_menu}`, `{desktop}`, `{temp}`.
- Versions are semantic versions. A product's version comes from its build (for example `Cargo.toml` or a tag), not from the installer project.
- Targets are named `os-arch` plus a renderer, such as `dos-x86:tui-retro` or `linux-x86_64:canvas`. An `os-arch` may be declared once per renderer; declaring the same pair twice is an error.

## 2 · Project layout

```text
installer/
	installer.toml        product, publisher, targets, signing
	components.toml       what gets installed
	flows/
		install.flow.json   one flow per lifecycle
		update.flow.json
		modify.flow.json
		uninstall.flow.json
		launch.flow.json
	layouts/*.layout.json layouts shared by every theme
	scripts/*.lua         project hooks
	themes/<name>/        theme.toml, assets and the layouts only this theme has; or a reference to a published theme
	feed/channels.toml    where releases are announced
```

## 3 · installer.toml

```toml
[product]
id        = "hq.liminal.lt-fizz"
name      = "Lieutenant Fizz — Episode One"
publisher = "Liminal HQ"
version   = { from = "git-tag" }
licence   = "LICENCE.TXT"

[defaults]
scope = "user"        # user | machine | ask
theme = "fizz-setup"

[[target]]
id       = "dos-x86"
renderer = "tui-retro"
theme    = "fizz-dos"
outputs  = ["floppy-1440", "zip"]

[[target]]
id       = "windows-x86_64"
renderer = "canvas"
outputs  = ["setup-exe", "msix"]

[signing]
feed_key = "keys/feed.pub"   # pinned into every build
```

`feed_key` is the public half of a key made by `liminst keygen`. A relative path is looked up beside `installer.toml`, then in the project folder. `build` reads it and carries it in the `.lip` (§14), where the installed runner keeps it. A product with no `feed_key` cannot update from a feed unless the person names a key.

## 4 · Components

```toml
[[component]]
id       = "game"
name     = "Game program"
required = true
files    = [{ from = "dist/{target}/FIZZ.EXE", to = "{install}/" }]
shortcut = [{ name = "Lieutenant Fizz", target = "{install}/FIZZ.EXE", in = "start_menu" }]

[[component]]
id      = "sound"
name    = "Sound drivers"
targets = ["dos-x86", "win16-x86"]
files   = [{ from = "dist/sound/*.DRV", to = "{install}/SOUND/" }]
```

A component may also declare `file_types`, `url_schemes`, `services` (systemd user units, launchd agents, Windows services), `path` additions and `registrations` (such as an agent plugin folder). Each turns into plan operations (§8).

## 5 · Flows

A flow is a directed graph of steps. Every path must reach a `finish` step, and every flow has exactly one `commit` step; the CLI refuses a flow that breaks either rule.

| Type | What it does |
|---|---|
| `page` | Shows a layout and collects fields into the flow's state. |
| `action` | Runs a script or a built-in action without a page, such as launching the app. A built-in is written `"on_enter": "builtin:<name>"`; the names are `check-feed`, `launch-app`, `detect-sound` and `check-disk-space`, and `check` refuses any other. The lifecycle carries out the first two, so the flow runner does not run a script for them. The DOS runtime carries out the last two (`detect-sound` fills `sound.card`, `sound.port`, `sound.irq` and `sound.dma` from `BLASTER` or a port probe; `check-disk-space` on a page refuses Next when the install does not fit the drive); every other runner skips them. The DOS runtime has no scripts: a step on a `tui-retro` target may name only a built-in, `check` reports any script hook as `script-on-retro`, and `build` refuses it. |
| `branch` | Chooses the next step from a condition. |
| `commit` | Turns the plan into changes. Its page shows progress and any theme billboards. |
| `finish` | Ends the flow and writes the receipt. |

Table 1. Step types.

```json
{ "id": "sound-card", "type": "page", "layout": "sound-card",
	"when": "target.renderer in ['tui-retro', 'win16']",
	"on_enter": "scripts/sound-setup.lua#detect",
	"next": "ready" }
```

Conditions use a small expression language over `target`, `components`, `state` and `system` (OS version, free space, existing install). No function calls, so every renderer can evaluate them, the retro runtime included.

## 6 · Themes and layouts

```toml
[theme]
id        = "fizz-setup"
name      = "Fizz Setup"
renderers = ["win16", "win32-classic", "canvas", "web"]
fallback  = "plain"

[pages]                     # pages this theme adds to any flow
intro = { layout = "intro", before = "welcome", skippable = true }

[audio]
music = "assets/theme.mod"  # the engine always adds a mute control
```

A theme may also carry a **stage**, `themes/<name>/layouts/stage.layout.json`: the screen around every page, with a `slot` node where the page's own layout is drawn. The stage holds what every page of the theme shares (the `backdrop` and its wordmark, the status meters, the taskbar label) and a page's layout is only its `dialog` or `window`. A screen is composed when a step is shown: the stage of the target's theme (or of the nearest theme on its `fallback` chain) with the page in its slot, or appended to the stage's root when it has no slot. A page whose root is a `backdrop` draws itself and the stage does not apply to it, which is how one page gets its own copy of the stage. A stage node with `show_on` or `hide_on`, each a list of step ids or step types (`page`, `action`, `branch`, `commit`, `finish`), is drawn only on the steps it names, or on every step but those. Every renderer, `liminst preview` and the Studio see the composed screen.

A layout for a step is found in the target's theme (`themes/<name>/layouts/<id>.layout.json`), then in each theme on its `fallback` chain that the project has, then in the project's shared `layouts/`. `fallback = "plain"` names the built-in plain look every renderer has, so the project need not carry a theme of that name. A layout is a tree of nodes. Each node may carry `@renderer` overrides; a renderer uses its override where one exists and the base node otherwise.

| Node | Notes |
|---|---|
| `window`, `backdrop`, `dialog` | Containers. `backdrop` fills the screen, as in the classic setup look. |
| `stack`, `grid` | Layout in rows, columns or cells. |
| `text`, `image`, `billboard` | Content. Billboards rotate while files copy. |
| `meter` | A live gauge fed by the engine: `file` (the file being copied right now), `total` (progress across the whole package) or `free_space` (free space left on the target drive, with a “Low” warning when the install would leave it nearly full). Classic themes show all three together on the backdrop, as 90s setups did; each theme can switch them off. |
| `slot` | In a theme's stage only: where the page's layout goes. |
| `button`, `checklist`, `radio`, `field`, `path`, `progress` | Controls, bound to state with `bind`. |
| `custom` | `web` and `canvas` only: theme-drawn content such as a game. Other renderers skip it. |

Table 2. Layout nodes.

Nodes read these properties, all optional. A renderer ignores any it does not know.

| Property | On | Meaning |
|---|---|---|
| `region` | any child of a `dialog`, `window` or `backdrop` | Where the node sits: `side` (art and step lists beside the content), `footer` (the button row, with a `note` text on `window` looks), `hints` (a key-hint bar of `text` nodes with a `key`), `status` (meters shown together on the backdrop). The `tui` renderer leaves out `side` and `hints`. |
| `title`, `subtitle`, `credit`, `taskbar`, `fill` | `backdrop` | The wordmark and its second line, the credit line, the taskbar label of a classic look and the DOS fill character. |
| `icon`, `caption` | `window` | A title-bar icon (an `art` value) and a line of text under the window. |
| `art`, `panel`, `ground` | `image` | Pixel or vector art drawn as inline SVG, and the colours of a side panel behind it. |
| `captions`, `slides` | `billboard` | `captions` are `{title, text, art}` slides shown in turn; `slides` names the theme assets. |
| `source`, `label`, `caption`, `low_below`, `warn_label`, `place` | `meter` | What it measures (`file`, `total` or `free_space`), its label and caption, the fraction of the drive under which free space is “Low” (default 0.05) and the word shown then (default “Low”), and where it is drawn: `backdrop` (the default for a meter in the stage or with `region = "status"`) or `dialog`, inline in the page. `tui` reads `meter` too. |
| `show_on`, `hide_on` | any node in a stage | Step ids or step types the node is drawn on, or left out of. |

| `role`, `value`, `max`, `text` | `progress` | `role = "steps"` draws “Step n of m” with a block bar. On the DOS artboard the side column draws the `text` and a block bar of `value` in `max`. |
| `art` (pixel form) | `image` | On the DOS artboard each `[x, y, w, h, "#rrggbb"]` rect becomes cells: the nearest of the 16 VGA colours, two pixel rows to a text row in half and full blocks. |
| `source`, `scroll`, `lines` | `text` | `source` names a project file shown as scrolling text (the DOS runtime reads it at build time; `lines`, default 8, is its height). |
| `enabled_when` | `button` | A condition (§5); the button is disabled until it holds. The DOS runtime honours it. |
| `key`, `text` | `text` in a `hints` region | One hint: `key` (shown in red on the DOS hint bar) and its `text`. |
| `role`, `state`, `key` | `text` | Roles are `heading`, `body`, `caption`, `error`, `document`, `summary`, `title`, `subtitle`, `note`, `step-item` and `task`; `state` is `done`, `current` or `todo`; `key` marks a hint-bar key. |
| `items` or `options`, `cards`, `sunken` | `checklist`, `radio` | A list of `{id, label, description, required, checked, badges}`, or the name of a list: `"components"` or a state field such as `"sound.cards"`. |
| `hotkey`, `browse` | `button`, `path` | The access key underlined (classic) or coloured (DOS), and the label of a Browse button. |

Table 3. Node properties.

The Studio's add-node palette offers every node kind, grouped (containers, content, controls, status meters, other) with a short description, and presets for the usual combinations: a footer button row, side art, a billboard, a checklist of the components, a radio group, a field, a path with Browse, step progress, key hints and the three status meters together. The meters are added to the theme's stage when the theme has one, so every page shows them; a stage node is shared by all pages of the theme and the Layouts screen says so. A meter's `bind` is `engine.meter.<source>`; the engine's progress events feed it, and the Dry-run preview animates the three (free space follows the chosen folder and components, and reads “Low” when the install would leave the drive nearly full).

`art` is `{ width, height, scale, rects }` for pixel art, with each rect `[x, y, w, h, colour]`, or `{ viewBox, shapes }` for vector shapes (`rect`, `path`, `circle` with `fill`, `stroke`, `width`). Text properties may hold `{placeholders}` such as `{product.version}`, `{target.id}` and `{install.path}`; a screen fills them from the run's values, and a name with no value stays as written.

Studio and design looks. A target's renderer and theme pick how the Studio draws a layout: `tui-retro` and `tui` the DOS text look (VT323, a cyan title bar, a `░` backdrop, a double-line dialog with its title in the border and a key-hint bar), `win32-classic` and `win16` the Windows 95 classic look (a blue gradient backdrop with an italic shadowed wordmark, a bevelled dialog with a navy title bar and a teal side panel, sunken lists and the status meters), and `canvas` and `web` the Afterglow look, or the Neutral look for the `neutral` theme or a root with `style = "neutral"`. The example themes are `fizz-dos` and `fizz-win95` (Lieutenant Fizz), `afterglow` (Planboard), `neutral` and `installshield-classic`, a product-agnostic InstallShield-style version of the classic look.

For `tui` and `tui-retro` targets the Studio can also show the real terminal output instead of its own drawing, labelled “Terminal (real)”. The `tui` renderer draws into a ratatui buffer, and `render_cells` returns that buffer cell for cell as a `CellGrid`: each cell has one grapheme (`ch`), a foreground and background colour (`default`, one of the sixteen named ANSI colours, `indexed:N` or `#rrggbb`) and the modifiers bold, dim, italic, underline and reversed. `CellGrid::to_ansi` writes the same grid as an ANSI string. The Studio runs a live session per open screen, in Rust, so the webview keeps no terminal state: opening one takes a target, an install-flow step (or a layout id) and the page state, and each key press returns the new frame, the updated state and what the key did (nothing, a changed value, a chosen button or Esc). In the Dry-run screen a chosen Next, Back or Cancel button moves through the flow. The full-screen preview window offers the same View switch for these targets and opens on the step, target and view that were showing; its terminal is an 80 by 25 cell screen drawn at the largest whole-number zoom that fits. Inside the terminal every key, Esc included, belongs to the installer (Esc cancels back to the first step); `F6` leaves the terminal, after which Esc leaves full screen and closes the window. A session resolves the page as a preview does, and fills `{placeholders}` from the preview's sample values.

Every control has an accessible name, and every renderer supports keyboard-only use. A theme can't turn either off.

## 7 · Scripts

Scripts are Lua 5.4 on modern targets. For DOS and Win16 the CLI compiles them for the retro runtime's Lua-compatible subset and refuses features it lacks. A script names the capabilities it needs; everything else is refused.

| Hook | When | May change |
|---|---|---|
| `detect(page)` | A page opens | Page fields |
| `validate(page)` | Before Next | Nothing; returns problems |
| `on_commit(plan)` | Plan is built | Adds plan operations |
| `on_uninstall(plan)` | Uninstall plan is built | Adds plan operations |

Table 3. Hooks.

Modules: `li.env` (`get`, `os`, `arch`), `li.fs` (read; writes only through the plan), `li.ui` (`note`, `dialog`), `li.plan` and `li.net` (needs `net`). A script names its capabilities in a header comment (`-- capabilities: env, ui`); there is no `require`, and the modules are globals. A page's fields are in `page.fields`. `li.retro.sound` and `li.retro.video` are designs only.

## 8 · Plan operations

A plan is an ordered list of operations. Each records enough to undo itself before it runs.

| Operation | Undo |
|---|---|
| `mkdir`, `copy`, `write_file` | Remove what was added; restore what was replaced from the journal |
| `write_ini`, `write_json` | Restore the earlier contents |
| `shortcut`, `file_type`, `url_scheme` | Remove the registration |
| `registry.set` (Windows) | Restore or delete the value |
| `path.add`, `service.install` | Remove the entry; stop and remove the service |
| `os.register` | Remove the Add/Remove Programs entry or package database record |
| `receipt.write` | Always last; never undone, only replaced |

Table 4. Operations.

The DOS runtime performs `mkdir`, `copy`, `write_ini` and `shortcut` (a launcher batch file), then writes the receipt. File types, URL schemes and `PATH` entries are skipped with a build warning.

## 9 · Lifecycle and receipts

| Command | Starts from | Does |
|---|---|---|
| install | Nothing, or an older receipt | Runs the install flow |
| update | Receipt and feed | Downloads changed files, keeps choices |
| modify | Receipt | Adds or removes components |
| repair | Receipt | Checks hashes, restores what is missing or changed |
| uninstall | Receipt | Undoes the recorded operations in reverse |
| launch | Receipt | Starts the app, offering an update first if the feed has one |

Table 5. Lifecycle commands.

Example 1. A receipt, shortened.

```json
{ "version": 1, "product": "hq.liminal.lt-fizz", "installed": "1.0.0",
	"target": "windows-x86_64:canvas", "scope": "user",
	"feed": "https://example.org/feed.json", "channel": "stable",
	"components": ["game", "levels"],
	"operations": [ { "op": "copy", "to": "{install}/FIZZ.EXE", "sha256": "…" } ] }
```

Receipts are kept in `{install}/.liminal/receipt.json` and indexed per user and per machine, so the Hub can find every installed product. The retro runtime writes the same data as `INSTALL.LIR`: a text file of `LIR 1`, `product=`, `version=`, `target=`, `dir=`, `components=` lines and one `D` (directory made), `F name size crc` (file, with a CRC-32), `I` (INI) or `B` (launcher batch) record per line. It records CRC-32, not SHA-256; uninstall removes the files, then the directories it made when they are empty. See [docs/retro-dos.md](docs/retro-dos.md).

## 10 · Platform integration

| Platform | Apps list | Shortcuts and file types |
|---|---|---|
| Windows 95–11 | Uninstall registry key | Start Menu .lnk, ProgIDs |
| Windows 3.1 | None; uninstall via setup | Program Manager group (DDE) |
| Linux, FreeBSD | Package database for native packages | XDG desktop entries, shared MIME info, icon theme |
| macOS | `pkgutil` receipt for .pkg | /Applications, Launch Services |
| DOS | None | Optional batch launcher |

Table 6. Where each platform's registrations go.

## 11 · Feeds

```json
{ "version": 1, "publisher": "Liminal HQ",
	"products": [ {
		"id": "hq.liminal.planboard", "name": "Planboard",
		"channels": { "stable": "0.9.0" },
		"releases": [ { "version": "0.9.0", "artefacts": [
			{ "target": "linux-x86_64:canvas", "kind": "payload",
				"url": "https://example.org/planboard-0.9.0-linux.lip",
				"size": …, "sha256": "…" } ] } ],
		"themes": ["afterglow"] } ] }
```

A feed is published with a detached signature at `feed.json.sig`: the Ed25519 signature, as hex, over the exact bytes of `feed.json`. Clients check it against the pinned key before reading anything else, and refuse a feed whose version or timestamp goes backwards (the runner remembers the last feed it verified). An artefact's `url` may be an `http(s)` URL, an absolute path, or a path beside the feed. A feed may also carry a `themes` section that lists shareable themes (see D8 in [docs/decisions.md](docs/decisions.md)).

## 12 · CLI

| Command | Purpose |
|---|---|
| `init` | Create `installer/` in a product repository |
| `check` | Validate the project for every target |
| `preview --target t` | Dry run; prints the plan the engine would build, and draws each step |
| `build --target t` | Write the themed and native outputs for a target |
| `keygen` | Make an Ed25519 key pair: `<out>.key` (secret) and `<out>.pub` (public, as hex) |
| `feed add` | Add the built release to a feed; `--sign-key` signs it |
| `theme pack` | Package a theme for sharing; `--sign-key` signs it |
| `studio --mcp` | Serve the project to agents over MCP, with no window; `--port` picks the port |

Table 7. `liminst` commands.

`check` loads the project with the engine and reports the CLI's checks (flows, themes, layouts, accessible names, components) with the engine's condition, component and hook checks. `preview` prints the engine's Plan phase against a simulated machine, so it shows the same operations, hashes and script hooks an install runs. Exit codes: 0 success, 1 the project has errors, 2 the command could not run.

`build` outputs, by name: for a `tui-retro` target, `zip` and `floppy-1440` (`SETUP.EXE`, `SETUP.DAT` and `README.TXT`, also written loose under `dos/`; `--dos-runtime PATH` names a prebuilt `SETUP.EXE`, else one is found or built from `dos/`; a payload too big for a floppy is reported, and disks do not span); for the others, `lip` (the payload and bundled project, §14), `run` (with `--engine PATH`, a themed self-extracting installer that carries the runner and the `.lip`; without it, a `.run` that only extracts the files and a receipt), and the native formats `deb`, `rpm`, `arch`, `msi`, `msix` and `pkg`. A themed `run` also writes its `.lip`, which is what `feed add` publishes. `feed add --sign-key FILE` writes `<feed>.sig`; without a key it removes a signature that no longer matches. `theme pack --sign-key FILE` writes `<archive>.sig`, the hex signature over the archive.

Example 2. A CI job.

```yaml
- uses: liminal-hq/installer-action@v1
  with:
    targets: windows-x86_64,linux-x86_64,dos-x86
    sign-key: ${{ secrets.FEED_KEY }}
    feed: feed/stable.json
```

## 13 · Studio API for agents

The Studio serves MCP on `127.0.0.1`, protected by a token, as the Planboard server does. There is no GraphQL API (D26). The Studio prints the address at start and never the token; Preferences shows it on request, copies it and replaces it, and a client holding the old one is refused from then on. The server runs only while `mcpEnabled` is on. Every change goes through the same undo history as a person's. When the server cannot start, such as when its port is taken, the Studio says why in the agent card and in Preferences under Agents.

`liminst studio --mcp` serves the project given by `-C` the same way with no window, until Ctrl+C. It listens on `127.0.0.1`, on `--port` or a free port. Its first two lines of standard output are `Studio MCP server: <url>` and `Token: <token>`; this is the one place a token is printed, it is new on every run, and it is never logged. The undo history lives in memory for the run, and every change is written to the project's files at once.

| Tool | Purpose |
|---|---|
| `get_project`, `list_targets` | Read the project and its variations |
| `get_flow`, `update_flow` | Read and patch a flow graph |
| `get_layout`, `update_layout` | Read and patch a layout, for one renderer or all |
| `read_script`, `write_script`, `test_script` | Work on scripts and run their tests, optionally against one target |
| `check_script_syntax` | Check Lua source without running it: syntax errors and warnings |
| `run_preview`, `get_preview` | Dry-run a target; get the plan, problems and a screenshot of each step |
| `tui_render`, `tui_key` | Open a live session on a `tui` target's page and press keys in it; get the real terminal screen as text and ANSI (and the cell grid on request) |
| `check_project` | The same checks as `liminst check` |

Table 8. MCP tools.

The Studio runs the engine itself, not a stand-in. `check_project` is the Studio's own checks plus the engine's (condition syntax, components named in conditions, hooks and built-in actions), run over the files on disk, which every change writes through. `run_preview` takes a target as `os-arch:renderer` and returns the engine's Plan phase against a simulated machine, with the problems found and one screen per step. The flow is walked as that target would walk it: a step whose `when` is false is listed as skipped and has no screen, a `branch` takes the first branch whose condition holds (else `next`), and a page's `detect` hook runs first against sample state. Each step carries `layout_tree`, the layout as the target's renderer draws it (its `@renderer` overrides applied, list references such as `items = "components"` expanded, `LICENCE.TXT` read in) and the page `state`; the preview also carries the target's `renderer` and `theme`, the `product` and sample `vars` for `{placeholders}`. The Studio draws each step from these as the rich screen of the target's look. `tui` targets also carry the renderer's text dump as a SVG, shown as a collapsible text version, and that dump fills `{placeholders}` from the same values. A step with no layout for the target has none. A theme's own layouts, in `themes/<id>/layouts/`, are listed and edited as `theme/id`. `tui_render` takes a `tui` target, a `page` (an install-flow step or a layout id), optional `state`, `keys` to press first, a size (`cols` and `rows`, 80 by 24 by default) and `close`; it returns a `session` id, the screen as `text` and `ansi`, the `focus` (what the focused control is, as a screen reader would say it) and the page `state`. `tui_key` presses `key` or `keys` in that session and also returns the `outcome` of the last key (`none`, `changed`, `activate` with the button's action, or `cancel`); `close` frees the session, and the oldest of 32 open sessions is dropped when more are opened. Both accept `grid` to add the cell grid. Keys are single characters or names such as `Tab`, `BackTab`, `Enter`, `Escape`, `ArrowDown` and `Space`. `test_script` runs the source it is given, which may be unsaved, in the script sandbox against a sample page (`install.path`, `components`) and a sample plan, calling each hook the script defines. With a `target` (`os-arch:renderer`) the sample is that target's: `li.env.os()` and `li.env.arch()` answer with the target's own, `li.env.get` also knows `target`, `install` and `version`, and `components` is what the target gets. Without one, a fixed sample is used. The report lists the target, the hooks present, the modules the script's capabilities open, any capability denial or script error as a problem, a line for each hook's result, each field `detect` set, each problem `validate` returned and each operation a hook added, and one entry per hook that ran (`tests`: the hook, whether it passed, a one-line summary and what it listed).

`check_script_syntax` compiles the source in an empty Lua state without running it, so no hook is called and nothing in the script executes. A syntax error comes alone, with its line and the column of the token Lua stopped near. Otherwise it returns warnings, each with a line, column, span and code: `undeclared-capability` (a `li.*` module used without the capability that opens it, reported at its first use), `unknown-capability` (an error, in the header), `unknown-module`, `unavailable-module` (`li.retro.*`, designs only), `sandbox-global` (`require`, `pcall`, `io`, `os` and the other globals the sandbox removes), `hook-parameter` (`on_commit` and `on_uninstall` take `plan`; `detect` and `validate` take `page`) and `retro-subset`. The retro Lua subset lacks `li.net`, `utf8`, `string.pack`, `string.unpack`, `string.packsize`, `goto` and `<close>` variables.

**Errors.** A failed Studio call, whether a Tauri command or an MCP tool, carries `{ "kind": …, "message": … }`. `message` is for a person; `kind` is for code and is one of `not_found` (a document, target, tool or file does not exist), `invalid_input` (an argument or document is not acceptable), `io` (a file could not be read or written), `project` (no project is open, or there is nothing to undo), `script` (a script could not be tested), `backend` (the engine failed), `agent` (the agent server could not do it) and `internal` (the shell failed). A Tauri command rejects with that object. A JSON-RPC error carries it in `error.data`, so an unknown method is `not_found` and a missing `name` is `invalid_input`. A tool that fails returns a result with `isError: true`, the message as text and the same object in `structuredContent`. The Rust types that cross these boundaries are exported to TypeScript with ts-rs into `packages/protocol` (`@liminal-hq/installer-protocol`), which the Studio UI imports.

### The Studio window

The Studio draws the shared Liminal HQ window chrome in its desktop window: a 40px title bar with the gradient wordmark, an app menu, window buttons and an optional menu bar. The app menu is one flat list of groups (New project…, Open project…, Open recent, Reload; Save; Undo, Redo; the five screens and Show menu bar; Run check, Run dry-run preview; Preferences…, Keyboard shortcuts, Connect an agent…, About, Quit) opened from the wordmark, `F10` or a lone `Alt`. The menu bar (File, Edit, View, Project, Help) holds the same commands and takes `F10`, `Alt` and `Alt` plus a letter while it shows. Preferences, Keyboard shortcuts, Connect an agent, New project, About and confirmations are child windows of the main window. New project (`Ctrl+N`) asks for a name, id, publisher and a folder, writes the same files as `liminst init` into a folder named after the project, and opens it; it refuses an id that is not reverse-DNS and a folder that already holds a project. Keyboard shortcuts lists the table every shortcut is matched against. Connect an agent shows the endpoint, copies the URL, the token or a sample `mcpServers` client configuration, and says why the server is off when it failed to start. Opening another project drops the previous project's selections and loaded documents. The status bar counts errors and warnings (a click opens the Overview's checks), and its History count leaves out undone entries. A browser tab, used for development only, draws a plain header instead. Preferences, opened with `Ctrl+,`, the app menu or File, is a child window with a side nav that becomes a select when narrow. See "Studio preferences" below for the settings.

**Studio preferences.** Every change applies at once and is written to `settings.json` in the app's config directory (`app_config_dir()`: `$XDG_CONFIG_HOME/ca.liminalhq.installerstudio` on Linux, `~/Library/Application Support/ca.liminalhq.installerstudio` on macOS, `%APPDATA%\ca.liminalhq.installerstudio` on Windows) as a merge. A write that is refused leaves the value as it was and the row says why. Every open window follows a change at once, the theme included. The file has a `version` (1); a file written before it, with only `menuBar` and `theme`, is read as version 1, a field that is missing or invalid takes its default, and keys the Studio does not know are kept. The size of the main and Preferences windows is kept in `.window-state.json` beside it. A settings file left in the config directory of the earlier identifier (`hq.liminal.installer-studio`) is copied across once, when the new directory has none.

**The window and its text.** The Studio's page is React 19. Every string a person sees comes from a message catalogue (`en-CA`, with the accented `en-XA` and the right-to-left `ar-XB` pseudo-locales in a development build, chosen with `?locale=` in the address), so the layout can be checked against longer and mirrored text. The five screens (Overview, Flow, Layouts, Scripts, Dry run) are routes in the main window, and every other window (Preferences, About, Keyboard shortcuts, Connect an agent, New project, confirmations) is the same page with a `?window=` label. The Scripts screen keeps the draft when another screen is shown, and its undo history starts afresh when the screen is shown again.

**The script editor.** The Scripts screen is a CodeMirror 6 editor with Lua 5.4 colouring that follows the theme, line numbers, active-line highlight, bracket matching and closing, folding by block, undo and redo, and find and replace. Completion and hover text come from the `li-script` API: the modules `li.env`, `li.fs`, `li.ui`, `li.plan` and `li.net` (each member shows the capability it needs), the hooks `detect(page)`, `validate(page)`, `on_commit(plan)` and `on_uninstall(plan)`, `page.fields`, `plan:add`, and the capability names in a `-- capabilities:` header. Findings from `check_script_syntax` are drawn as squiggles and gutter markers after a pause in typing. The editor's text is a draft held in the app: it survives redraws and other screens, File > Save saves it from any screen, "Discard changes" in the unsaved-changes confirmation clears it, and choosing another script with unsaved edits asks first. Keys: Tab indents (Esc then Tab moves focus out), Ctrl+S saves, Ctrl+Enter runs the test, Shift+Alt+F formats. Format re-indents by block and bracket depth with tabs, trims trailing space, uses line feeds and ends the file with one newline, and never touches the inside of a long string or comment. Test run shows "Running…", then "Tested at HH:MM:SS · n ms" with "(unsaved text)" or "(saved file)", toasts pass or fail, marks the report out of date when the draft or the target changes, and prints a console with a line for each hook, its errors and a summary. The target picker chooses whose sample the test uses. The footer shows "Ln n, Col n" and "Lua 5.4 · retro subset checked".

Table 7a. Settings and defaults.

| Section | Setting | Default | Effect |
|---|---|---|---|
| General | `menuBar` | off | The menu bar under the title bar |
| General | `reopenLastProject` | off | Opens `lastProject` on start when none is given on the command line |
| General | `confirmDiscard` | on | Asks before Reload, Open project or closing throws away unsaved script edits; off discards them without asking |
| General | `recentLimit`, `recentProjects` | 10 (5 to 20), empty | File > Open recent, newest first, with Clear recent projects |
| Appearance | `theme` | `afterglow` | `afterglow` (dark), `afterglow-light`, `afterglow-dark` (neutral dark) or `system`, which follows the desktop between `afterglow` and `afterglow-light` |
| Appearance | `density` | `comfortable` | `compact` tightens lists, tables and toolbars |
| Appearance | `reduceMotion` | `system` | `system`, `on` or `off` |
| Appearance | `textSize` | 100 (90 to 130) | Scales the text of the app and the settings pane |
| Editor | `editorFontSize`, `tabWidth`, `wrapLines`, `lineNumbers`, `formatOnSave` | 13 (10 to 24), 4 (1 to 8), on, on, off | The Scripts screen, live; format on save runs the Lua formatter (see "The script editor") |
| Preview | `previewTarget`, `previewAutoRun`, `showSkippedSteps` | first target, off, on | The target a dry run starts on; running it again when a flow or layout changes; listing steps skipped for the target |
| Agents | `mcpEnabled`, `mcpPort` | on in a debug build and off in a release build, 0 | Whether the MCP server runs and where; 0 picks a free port and a port in use refuses the change |
| Projects | `projectsFolder` | none | Where Open project… starts |
| Projects | `runnerPath`, `dockerPath` | none | The `liminal-engine` runner and Docker; each must answer `--version` before it is saved |

Advanced reveals the settings file and the log folder, shows the version and resets every setting to its default after a confirmation (recent projects included; keys the Studio does not know stay).

## 14 · The runner

`liminal-engine` is the small program a themed installer hands over to. It reads a `.lip`, loads the project bundled in it, and runs the real flows with the `tui` renderer.

A `.lip` is the container of `li-pack`: the files the target's components name, at their project-relative paths, plus a bundle under `_project/`:

| Entry | Holds |
|---|---|
| `_project/installer/…` | `installer.toml`, `components.toml`, `flows/`, `themes/` and `scripts/` |
| `_project/target` | The target id the payload was packed for, such as `linux-x86_64` |
| `_project/version` | The product version of the build |
| `_project/feed.pub` | The pinned feed key, as hex, when `signing.feed_key` is set |

Table 9. A bundled `.lip`.

The themed `.run` is the shell stub of `li-package` followed by a gzip tar of `engine/liminal-engine` and `payload.lip`. The stub verifies the checksum, extracts to the prefix (`--prefix`, else `/opt/<slug>` as root, else `$XDG_DATA_HOME/<slug>`) and runs `engine/liminal-engine install --from <prefix>`, passing on everything after `--`.

| Command | Does |
|---|---|
| `install [--from DIR] [--lip FILE] [--component ID]… [--feed F] [--channel C]` | Runs the install flow. The install directory is the flow's `install_dir` answer, else `--install-dir`, else `--from`. `--feed` is kept in the receipt for later updates |
| `update [--feed URL\|PATH] [--feed-key FILE]` | Verifies the feed, downloads the release's `.lip`, checks its size and SHA-256 against the feed, and updates with the new `.lip`'s project, keeping the installed components |
| `modify [--component ID]…` | Installs exactly these components (the default is those installed now) |
| `repair` | Redoes every recorded operation whose file is missing or changed |
| `uninstall` | Undoes the receipt in reverse, then removes the runner's own files |
| `launch [--feed F]` | Starts the first shortcut's target; if the feed has a newer release it says so first, then starts the product |

Table 10. Runner commands.

- **Flows.** `install` needs an install flow. `update`, `modify` and `uninstall` run the project's flow of that name when it has one, with the work at its `commit` step, and otherwise just do the work. `repair` and `launch` have no flow.
- **Interactive and headless.** With a terminal each page is drawn with the `tui` renderer (`--look retro|plain`, else `retro` for `tui-retro` targets); the commit page redraws as `Progress` events arrive, and Esc on it cancels and rolls back. With `--headless`, or with no terminal, every page is answered from `--answer key=value` (a value is JSON when it parses, else text; `components=a,b` selects components) and Next is pressed.
- **Where it lives.** After an install the runner copies itself, the `.lip` and the pinned key to `{install}/.liminal/runner/`, and removes what the stub extracted. Later commands find the install from their own location, or from `--install-dir`. `uninstall` removes that folder, then `.liminal` and the install directory if they are empty.
- **Feeds.** A feed is a `feed.json` path, the folder that holds it, or a URL (fetched with `curl`). The detached `feed.json.sig` must verify against the key pinned at install (or `--feed-key`) before anything is read; a feed older than the last one verified is refused.
- **Scope.** The runner installs for the person running it, with the user-scope directories of `li-engine`'s `RealMachine`. Elevation is not implemented yet.
- **Exit codes.** 0 success, 1 failure (the engine rolled every change back), 2 bad arguments, 3 the person cancelled.
