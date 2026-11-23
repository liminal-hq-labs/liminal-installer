# CLAUDE.md

A summary of [AGENTS.md](AGENTS.md), which is canonical. Keep the two in sync.

## What this is

Liminal Installer is a Liminal HQ Labs experiment: Themeable installers, updaters and launchers for every platform, from one project. Experimental, so anything may change. Behaviour lives in `SPEC.md`, decisions in `docs/decisions.md`.

## Rules to remember

- **Canadian English.** Real em dashes. Sentence case. No hard-wrapped markdown prose. Ship the result, not the journey: no "this PR" or reviewer names in code or docs.
- **Conventional Commits.** Body in markdown with bold labels and no headings, hard-wrapped, written to a file and committed with `git commit -F`. End with a `Co-Authored-By` trailer.
- **Git.** Topic branches, merge commits only, branches deleted on merge. Never push, tag, publish or dispatch workflows unless asked. Never `git add -A` or `git commit -a`. `PLAN.md` stays out of git.
- **Formatting.** Tabs everywhere except YAML. Rust uses rustfmt with hard tabs (`rustfmt.toml`); the Studio UI's TypeScript, CSS and JSON use Prettier (`.prettierrc`), never the vendored chrome or generated bindings.
- **Headers.** SPDX header on every source, script, YAML, TOML and SVG file: summary line, blank, "(c) Copyright 2026 Liminal HQ, Scott Morris", `SPDX-License-Identifier: Apache-2.0 OR MIT`.
- **Architecture.** Core first: `li-model` and `li-engine` have no UI dependency. Plan before commit, and every operation records its undo. Themes cannot add operations. Scripts hold only the capabilities they declare. `SPEC.md` first. Vendored chrome in `packages/chrome` is refreshed only by its `PROVENANCE.md`.
- **Toolchain.** Rust 1.98.0 (`rust-toolchain.toml`), `cargo nextest`. `make validate` is the single local gate. Markdown, shell and workflow checks run in Docker.
- **Wire types.** Rust types the UI or MCP clients receive derive `ts_rs::TS`; their TypeScript is generated into `packages/protocol` and committed, never hand-edited. Failures are `{ kind, message }` (`StudioError`).
- **CI.** One job per check. Shared images come from `liminal-hq/.github`. Dependabot is off unless the project opts in.
- **Docs.** Update `SPEC.md` and `docs/decisions.md` in the same change as the behaviour.

## Commands

```sh
bash scripts/check-headers.sh
make validate                    # fmt, clippy, nextest, headers, UI checks (needs bun), bindings drift
cargo nextest run --workspace --locked --profile ci   # as CI runs it
bash scripts/check-banner.sh     # add --shots /tmp/shots for review frames
bash scripts/e2e-linux.sh        # install, update, repair, uninstall in Debian (docker)
bun run format:check             # Prettier; `bun run format` fixes
bun run typecheck && bun run test:js
bash scripts/check-bindings-drift.sh   # ts-rs bindings in packages/protocol; regenerate with scripts/generate-protocol-bindings.sh
(cd apps/studio/src-tauri && cargo check && cargo clippy -- -D warnings)
docker run --rm -v "$PWD:/workdir" davidanson/markdownlint-cli2:v0.17.2 "**/*.md"
```
