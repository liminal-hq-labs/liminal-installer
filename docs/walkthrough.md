# Walkthrough

A hands-on tour: build the two example installers, install, update, repair and remove them, then open the Studio. Every command here was run on Linux; Windows and macOS targets build only in CI.

You need Rust (the version pinned in `rust-toolchain.toml`), pnpm and Node for the Studio, and Docker for the container check.

## 1. Build the tools

```sh
cargo build -p li-cli -p li-runner
export PATH="$PWD/target/debug:$PATH"
liminst --help
```

`liminst` is the builder. `liminal-engine` is the runner that a built installer carries.

## 2. Look at a project

An installer project is the `installer/` folder in a product's repository. The examples are under `examples/`.

```sh
ls examples/planboard/installer
liminst check -C examples/planboard
liminst check -C examples/lt-fizz
```

`check` loads every flow, layout, script and theme for every target and reports problems. `examples/neutral` is the plain default.

## 3. Preview without installing

```sh
liminst preview -C examples/planboard --target linux-x86_64
liminst preview -C examples/lt-fizz --target dos-x86
```

Preview prints the plan (what an install would do) and draws each step as text. Lt. Fizz on `dos-x86` is the blue DOS look, with a Sound card page that other targets skip.

## 4. Build an installer

```sh
liminst build -C examples/planboard --target linux-x86_64 \
  --engine target/debug/liminal-engine -o /tmp/pb --version 0.9.0
ls /tmp/pb
```

You get a self-extracting `.run` and the `.lip` payload, plus `.deb` and RPM sources. The `.run` carries the runner and the whole project, so it can run the real install flow.

## 5. Install it

```sh
sh /tmp/pb/planboard-0.9.0-linux-x86_64.run --prefix /tmp/pb-app
```

In a terminal this draws the Afterglow pages with the tui renderer: Tab and arrows to move, Space to toggle, Enter to press a button, Esc to cancel. Add `-- --headless` after the prefix to skip the pages and take the defaults:

```sh
sh /tmp/pb/planboard-0.9.0-linux-x86_64.run --prefix /tmp/pb-app -- --headless
```

Look at what it wrote:

```sh
find /tmp/pb-app -maxdepth 2
cat /tmp/pb-app/.liminal/receipt.json
```

The receipt records every operation, so update, repair and uninstall all start from it. The installer also writes a `.desktop` entry under `~/.local/share/applications`; uninstalling removes it.

## 6. Publish a signed update

The examples pin a demo public key in `installer/keys/feed.pub`. To sign your own feed, make a key pair and replace the pinned key before building the first release.

```sh
liminst keygen -C examples/planboard --out /tmp/pb-key --force
cp /tmp/pb-key.pub examples/planboard/installer/keys/feed.pub
```

Then build the two versions, rebuild 0.9.0 and install it as in step 5 so it pins your key, and publish 0.9.1:

```sh
liminst build -C examples/planboard --target linux-x86_64 \
  --engine target/debug/liminal-engine -o /tmp/pb2 --version 0.9.1 --output run
mkdir -p /tmp/feed && cp /tmp/pb2/*.lip /tmp/feed/
liminst feed add -C examples/planboard --feed /tmp/feed/feed.json \
  --target linux-x86_64 --file /tmp/feed/planboard-0.9.1-linux-x86_64.lip \
  --url planboard-0.9.1-linux-x86_64.lip --version 0.9.1 --sign-key /tmp/pb-key.key
```

## 7. Update, repair and remove

The installed copy keeps its own runner, so these work after the `.run` is gone:

```sh
R=/tmp/pb-app/.liminal/runner/liminal-engine
$R --headless update --feed /tmp/feed
rm /tmp/pb-app/docs/SHORTCUTS.md
$R --headless repair
$R --headless uninstall
```

`update` checks the feed's signature against the pinned key and refuses a forged or older feed. `repair` checks every file's hash and restores what is missing. `uninstall` undoes the recorded operations in reverse and leaves nothing behind.

## 8. Run the whole cycle in a container

```sh
bash scripts/e2e-linux.sh
```

This builds a project, then installs, updates, repairs and uninstalls it in `debian:stable-slim`.

## 9. Open the Studio

```sh
cd apps/studio/ui
pnpm install
pnpm tauri:dev
```

The first run compiles the Tauri shell, which takes a few minutes. Then:

- **Open project…** from the app menu (the wordmark, or `F10`): choose a folder that holds `installer/`, for example `examples/lt-fizz`.
- **Project, Flow, Layouts, Scripts, Dry-run:** the five screens. The dry-run preview runs the real engine: change a flow or a layout and run it again.
- **Menu bar:** switch it on from the app menu or View. It is saved in the Studio's settings.
- **About and Preferences:** `F1` and `Ctrl+,` open child windows.
- **Undo and redo:** shared by the UI and agents.

An agent can drive the Studio's project over MCP while you watch; see [studio.md](studio.md). In dev, the Tauri MCP server can also drive the window itself.

## 10. Make your own installer

```sh
mkdir my-product && cd my-product
liminst init
mkdir -p installer/payload/linux-x86_64
printf '#!/bin/sh\necho hello\n' > installer/payload/linux-x86_64/my-product
chmod +x installer/payload/linux-x86_64/my-product
liminst check
liminst preview --target linux-x86_64
liminst build --target linux-x86_64 --engine /path/to/liminal-engine --version 1.0.0 --output run
```

The payload folder stands in for your build output; the scaffold's `components.toml` expects `payload/<target>/my-product`. `init` writes `installer/` with a neutral theme. Edit `installer.toml` and `components.toml`, point the files at your build output, and adjust the flows, layouts and scripts in the Studio. In your product's CI, use `.github/actions/installer-action` to build every target; see [releasing.md](releasing.md).

## Known rough edges

- The Studio opens through the native folder dialog; it has not been tried on Windows or macOS.
- Layout text such as `{sound.detected}` shows as a placeholder in previews.
- Only user-scope installs work; there is no elevation yet.
- Windows, macOS, ARM and FreeBSD builds run only in CI, on a tag or a manual dispatch.
