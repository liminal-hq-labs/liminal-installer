# Decisions

Each decision is numbered and stays here even when superseded, with a note on what replaced it. Open questions move here once they are answered.

| # | Decision | Choice | Why |
|---|---|---|---|
| D1 | Home | A Liminal HQ Labs experiment in `liminal-hq-labs/liminal-installer`, public from the first commit | Try the idea in the open before it earns a place in Liminal HQ |
| D2 | Licence | `Apache-2.0 OR MIT` | House licence, compatible with both ecosystems |
| D3 | Dependency updates | No Dependabot configuration until the project opts in | Labs repositories stay quiet by default. See the [Labs charter](https://github.com/liminal-hq-labs/.github/blob/main/docs/labs-charter.md#dependabot-is-opt-in) |
| D4 | DOS floor | A 386 or later with a DOS extender | Keeps the retro runtime small enough for a floppy without the cost of real-mode 8086 support |
| D5 | `canvas` controls | The renderer draws its own controls, with native accessibility hooks added later; only the scaffold exists now | One look on every platform from a small binary, without waiting on per-toolkit controls |
| D6 | Hub feeds | The Hub reads multiple feeds, each with its own pinned key | A publisher's key only ever vouches for that publisher's feed |
| D7 | Delta updates | Deferred | Whole-file updates are enough to ship the first version, and the receipt format leaves room for chunks later |
| D8 | Theme sharing | Themes are shared through a theme section in any feed | Reuses the signing and pinning that feeds already have, with no separate gallery to run |
| D9 | Final name | The working name stays `liminst` until one is chosen from the Names board | The name and CLI command change together, once, rather than twice |
| D10 | Release automation | A tag-triggered `release.yml` builds `liminst` and the engine runner natively on each platform's runner, packages the examples with each platform's own tools, and publishes a GitHub release with `SHA256SUMS`. Signing and notarisation steps run only when their secrets exist | Native runners avoid cross-compiling the platform tools (WiX, `productbuild`, `makeappx`). Optional secrets keep forks and early tags working. FreeBSD is best effort because its runner is a virtual machine |
| D11 | The runner | A small separate binary, `liminal-engine`, carried in the `.run` next to the `.lip`, and kept in `{install}/.liminal/runner/` afterwards | The CLI stays a build tool, the stub stays plain shell, and update, repair and uninstall need nothing that was not installed |
| D12 | Project in the payload | A built `.lip` bundles the project under `_project/`; an update's `.lip` brings its own project, but the feed key stays the one pinned at install | The runner walks the real flows, layouts and scripts from one verified file, a release may change them, and a feed cannot swap the key that vouches for it |
| D13 | Signatures | Detached Ed25519 over exact bytes, as hex, in `<file>.sig`; keys are one line of hex; `liminst keygen` makes them | One simple scheme for feeds and themes that shell tools and other languages can check without a container format |
| D14 | No terminal | The runner falls back to headless answers when stdin or stdout is not a terminal; URLs are fetched with `curl` | A `.run` works over SSH, in CI and in containers without extra flags, and the runner avoids bundling an HTTP and TLS stack until it needs one |
