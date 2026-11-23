# Liminal Installer

<p align="center"><img src="assets/hero.svg" alt="Liminal Installer — Themeable installers, updaters and launchers for every platform, from one project" width="100%"></p>

<p align="center">
  <a href="https://github.com/liminal-hq-labs/liminal-installer/actions/workflows/ci.yml"><img src="https://github.com/liminal-hq-labs/liminal-installer/actions/workflows/ci.yml/badge.svg" alt="CI status"></a>
  <img src="https://img.shields.io/badge/status-experimental-a3e635" alt="Status: experimental">
  <img src="https://img.shields.io/badge/licence-Apache--2.0%20OR%20MIT-3fb950" alt="Licence: Apache-2.0 OR MIT">
</p>

Themeable installers, updaters and launchers for every platform, from one project.

Describe your product once, in an `installer/` folder next to your code. Liminal Installer installs, updates, repairs, removes and launches it on DOS, Windows, Linux, macOS and FreeBSD, and the installer can look like anything from a 1991 blue setup screen to a quiet modern window.

> **Status:** experimental. It is being built in stacked pull requests. The engine, the CLI, the `tui` renderer, the Studio and example installers for Planboard and Lt. Fizz are in progress, and there are no releases yet.

## What it's for

- Shipping one product to many platforms from a single set of plain-text files in its own repository.
- Giving an installer a personality: a retro DOS setup, a Windows 95 wizard or an Afterglow window.
- Checking and previewing an installer in CI before anything is built, without touching a real machine.
- Updating, repairing and uninstalling cleanly, because every install records a receipt that can undo it.
- Letting an agent build layouts and flows through the Studio while you watch the preview.

## Install and run

From source, with a recent Rust toolchain:

```sh
cargo build --release -p li-cli
liminst check -C examples/planboard
liminst preview -C examples/planboard --target linux-x86_64
```

The `liminst` binary is in `target/release/`.

## Documentation

- [SPEC.md](SPEC.md) — behaviour and interfaces
- [docs/walkthrough.md](docs/walkthrough.md) — build, install, update and open the Studio
- [docs/tauri-apps.md](docs/tauri-apps.md) — installing Tauri apps without a second installer
- [docs/architecture.md](docs/architecture.md) — how the pieces fit together
- [docs/decisions.md](docs/decisions.md) — decisions and why
- [AGENTS.md](AGENTS.md) — contributor and agent rules

## Contributing

Issues and pull requests are welcome. The house rules are in [AGENTS.md](AGENTS.md); run the checks it lists before opening a pull request.

## Part of Liminal HQ Labs

Liminal Installer is a [Liminal HQ Labs](https://github.com/liminal-hq-labs) experiment. Experiments from Liminal HQ — early, rough and open. If it proves itself, it [graduates](https://github.com/liminal-hq-labs/.github/blob/main/docs/labs-charter.md#graduation) to [Liminal HQ](https://github.com/liminal-hq).

## Licence

Licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or the [MIT licence](LICENSE-MIT), at your option.
