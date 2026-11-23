# Liminal Installer specification

The file formats, operations and interfaces behind the [architecture](docs/architecture.md). Examples use the Lieutenant Fizz project. The CLI is called `liminst`. When this document and the code disagree, decide which is right and fix the other in the same change.

## Status

Everything in this specification is in progress. Nothing is released, and any part may change between commits.

- **Engine** (plan, stage, commit, record): in progress. Install, update, modify, repair, uninstall and launch work end to end on Linux through the runner (§14).
- **CLI** (`check`, `preview`, `build`, `keygen`, `feed`, `theme`, `studio`): in progress. `check`, `preview` and the Linux `build` (themed `.run` and `.lip`) work, and feeds and themes are signed.
- **Renderers**: `tui` is in progress; `tui-retro`, `win16`, `win32-classic`, `canvas` and `web` are in progress as scaffolds or designs.
- **Studio** and its MCP and GraphQL API: in progress.
- **Example installers** for Planboard and Lt. Fizz: in progress.
- **Feeds, the web stub and the Hub**: in progress.
- **Delta updates, a theme gallery and macOS notarization in the CI step**: in progress as designs only.

## 1 · Conventions

- Hand-written files are TOML. Files the Studio writes (flows, layouts) are JSON, each with a `$schema` and a `version`.
- Paths in a project use `/` and are relative to `installer/`. Install-time paths use placeholders: `{install}`, `{user_data}`, `{system_data}`, `{start_menu}`, `{desktop}`, `{temp}`.
- Versions are semantic versions. A product's version comes from its build (for example `Cargo.toml` or a tag), not from the installer project.
- Targets are named `os-arch` plus a renderer, such as `dos-x86:tui-retro` or `linux-x86_64:canvas`.

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
	scripts/*.lua         project hooks
	themes/<name>/        or a reference to a published theme
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
| `action` | Runs a script or a built-in action without a page, such as launching the app. |
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

A layout is a tree of nodes. Each node may carry `@renderer` overrides; a renderer uses its override where one exists and the base node otherwise.

| Node | Notes |
|---|---|
| `window`, `backdrop`, `dialog` | Containers. `backdrop` fills the screen, as in the classic setup look. |
| `stack`, `grid` | Layout in rows, columns or cells. |
| `text`, `image`, `billboard` | Content. Billboards rotate while files copy. |
| `meter` | A live gauge fed by the engine: `file` (the file being copied right now), `total` (progress across the whole package) or `free_space` (free space left on the target drive, with a “Low” warning when the install would leave it nearly full). Classic themes show all three together on the backdrop, as 90s setups did; each theme can switch them off. |
| `button`, `checklist`, `radio`, `field`, `path`, `progress` | Controls, bound to state with `bind`. |
| `custom` | `web` and `canvas` only: theme-drawn content such as a game. Other renderers skip it. |

Table 2. Layout nodes.

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

Modules: `li.env` (read), `li.fs` (read anywhere, write only through the plan), `li.ui` (notes and dialogs), `li.plan`, `li.net` (needs `net`), `li.retro.sound` and `li.retro.video` (retro targets only).

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

Receipts are kept in `{install}/.liminal/receipt.json` and indexed per user and per machine, so the Hub can find every installed product. The retro runtime writes the same data as `INSTALL.LIR`.

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
| `studio --mcp` | Serve the project to the Studio and agents |

Table 7. `liminst` commands.

`check` loads the project with the engine and reports the CLI's checks (flows, themes, layouts, accessible names, components) with the engine's condition, component and hook checks. `preview` prints the engine's Plan phase against a simulated machine, so it shows the same operations, hashes and script hooks an install runs. Exit codes: 0 success, 1 the project has errors, 2 the command could not run.

`build` outputs, by name: `lip` (the payload and bundled project, §14), `run` (with `--engine PATH`, a themed self-extracting installer that carries the runner and the `.lip`; without it, a `.run` that only extracts the files and a receipt), and the native formats `deb`, `rpm`, `arch`, `msi`, `msix` and `pkg`. A themed `run` also writes its `.lip`, which is what `feed add` publishes. `feed add --sign-key FILE` writes `<feed>.sig`; without a key it removes a signature that no longer matches. `theme pack --sign-key FILE` writes `<archive>.sig`, the hex signature over the archive.

Example 2. A CI job.

```yaml
- uses: liminal-hq/installer-action@v1
  with:
    targets: windows-x86_64,linux-x86_64,dos-x86
    sign-key: ${{ secrets.FEED_KEY }}
    feed: feed/stable.json
```

## 13 · Studio API for agents

The Studio serves MCP and GraphQL on `127.0.0.1`, protected by a token it prints at start, as the Planboard server does. Every change goes through the same undo history as a person's.

| Tool | Purpose |
|---|---|
| `get_project`, `list_targets` | Read the project and its variations |
| `get_flow`, `update_flow` | Read and patch a flow graph |
| `get_layout`, `update_layout` | Read and patch a layout, for one renderer or all |
| `read_script`, `write_script`, `test_script` | Work on scripts and run their tests |
| `run_preview`, `get_preview` | Dry-run a target; get the plan, problems and a screenshot of each step |
| `check_project` | The same checks as `liminst check` |

Table 8. MCP tools.

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
