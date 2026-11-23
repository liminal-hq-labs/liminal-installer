# Architecture

One installer project per product, built in that product's CI, that installs, updates, repairs, removes and launches it on every platform. It can look like anything: a 1991 DOS setup screen, Windows 95, or a quiet Afterglow window.

“Liminal Installer” is the project name for now, and the CLI is `liminst` (see D9 in [decisions.md](decisions.md)). This document changes with the design: when a change alters what the system does, the matching section here changes in the same pass. The [specification](../SPEC.md) holds the exact file formats.

## Table of contents

- [Goals](#goals)
- [Non-goals](#non-goals)
- [The pieces](#the-pieces)
- [The installer project](#1--the-installer-project)
- [The engine](#2--the-engine)
- [Renderers](#3--renderers)
- [Two runtimes](#4--two-runtimes)
- [Themes](#5--themes)
- [Building in CI](#6--building-in-ci)
- [Feeds, the web stub and the Hub](#7--feeds-the-web-stub-and-the-hub)
- [Design Studio and agents](#8--design-studio-and-agents)
- [Security and trust](#9--security-and-trust)

## Goals

- **One project, every target.** DOS, Windows 3.1, Windows 95 to 11, Linux, macOS and FreeBSD from one set of files in the product's own repository.
- **Themes can do anything.** A theme can restyle the wizard, change its pages and order, play music, show billboards or a game while files copy, or open with a fake boot screen.
- **The whole life of an app.** Install, update, modify, repair, uninstall and launch, all from the same project and the same themed shell.
- **Native where it counts.** Each platform gets a renderer built from its own toolkit, and the builder also writes the platform's own package formats.
- **CI first.** A Rust CLI validates, previews and builds in any pipeline; the Studio is a convenience, never a requirement.
- **Agents are first-class.** The Studio exposes the project over MCP, as Planboard does, so an agent can build layouts and flows while you watch the preview.
- **A public product.** Built for Liminal HQ's own products first, but documented and licensed (Apache-2.0 OR MIT) for anyone to use.

## Non-goals

- Replacing the system package managers. We write their formats; we don't compete with them.
- A store with accounts or payments. Feeds list products; they don't sell them.
- Running code that the publisher didn't sign. Feeds, payloads and themes are all signed.

## The pieces

Work moves through three stages. Each stage is its own program, and the files between them are plain text you can read in a diff.

1. **Author.** The **Design Studio** (Tauri) and any **agent** over MCP edit the **installer project**: the manifest, components, flows, themes, layouts and scripts in the product's `installer/` folder. The result is committed to the product's repository.
2. **Build.** The **CLI** runs in the product's CI. It checks the project, compiles flows and scripts for each runtime, packs the payload, and writes the outputs: themed installers, native packages and feed entries. The result is signed artefacts and feed entries.
3. **Run.** On the person's machine, the **engine** plans and carries out the work and a **renderer** draws the theme. The **web stub** and the **Hub** read **feeds** to find what to download.

## 1 · The installer project

The project lives with the product it installs, in `installer/` at the root of the product's repository, so it is versioned, reviewed and branched with the code. It holds:

- `installer.toml`: the product's identity, version source, publisher, signing references and targets.
- `components.toml`: what gets installed. Each component lists files from the build, shortcuts, file types, registrations and services, and whether it is required.
- `flows/`: one flow per lifecycle (install, update, modify, uninstall, launch). A flow is a graph of steps; a step can be shown only where a condition holds, like the Sound card page on DOS and Win16.
- `themes/`: each theme is a folder of layouts, assets, sounds and theme scripts. A project can carry several themes and choose one per target.
- `scripts/`: Lua hooks for project logic, each asking for the capabilities it needs.

The Studio writes these files; nothing lives only inside the Studio. Hand-written files are TOML; Studio-written graphs and layouts are JSON with published schemas, so agents and people can edit either.

## 2 · The engine

The engine is a Rust library and a small host program, the `liminal-engine` runner (see §14 of the [specification](../SPEC.md)). Every lifecycle operation runs in four phases:

1. **Plan.** The flow runs and collects choices. Nothing on the machine changes; the result is an ordered list of operations, each with its undo.
2. **Stage.** Payloads are downloaded or unpacked to a staging folder and checked against their hashes and signatures.
3. **Commit.** Operations run in order. Each one is journalled before it runs, so a crash or a cancel can be rolled back.
4. **Record.** A **receipt** is written: what was installed, where, from which version and feed, and how to undo each part. Update, modify, repair and uninstall all start from the receipt.

The dry-run preview in the Studio is the same engine running Plan and Stage against a simulated disk and registry, then showing what Commit would do.

### Elevation

The interface always runs as the person. When a plan needs system-wide changes, a separate privileged helper starts (through UAC, polkit, Authorization Services or `doas`/`sudo`), receives the signed plan, and runs only the operations in it. Per-user installs need no elevation at all and are the default where the platform allows.

## 3 · Renderers

A theme describes an experience; a renderer draws it with whatever is native to the platform. A theme lists the renderers it supports and gives per-renderer layout overrides where it needs them. If a renderer has no layout for a step, it falls back to a plain version of the step and the Studio shows a warning.

| Renderer | Runs on | Built with | Good for |
|---|---|---|---|
| `tui-retro` | DOS 5+ on a 386 or later, with a DOS extender | Retro runtime (C), text-mode video | Blue-screen setup, real period installs |
| `tui` | Any terminal | Rust, crossterm | Servers, SSH, headless installs |
| `win16` | Windows 3.1 | Retro runtime (C), GDI and USER | Floppy-era Windows setup |
| `win32-classic` | Windows 95 to 11 | Rust, plain Win32 controls | Bevelled 90s dialogs, tiny downloads |
| `canvas` | Windows 10/11, Linux, macOS, FreeBSD | Rust, its own drawing (winit with a 2D renderer), no webview | Rich custom windows with a small binary |
| `web` | Modern desktops | Tauri webview, HTML, CSS and JS | Anything goes: video, games, wild layouts |

Table 1. Renderers in the first version.

## 4 · Two runtimes

Modern targets use the Rust engine. DOS and Windows 3.1 can't run it, so they get a **retro runtime**: a small C program built with Open Watcom that reads a compiled form of the same project.

- The CLI compiles the flow to a compact bytecode, flattens layouts into text-cell screens (DOS) or dialog templates (Win16), and converts assets to the target's palette and formats.
- Scripts run on a small Lua-compatible interpreter. The Studio checks scripts against the retro subset and warns before a build fails.
- The retro runtime writes its receipt in 8.3 form (`INSTALL.LIR`), so the same uninstall and repair logic applies.

## 5 · Themes

A theme is a signed package: `theme.toml`, layouts, assets, sounds and theme scripts. Themes are separate from products, so one theme can dress any product and one product can carry many themes. A theme can:

- restyle and rearrange the standard pages, or add its own pages to any flow;
- run its own steps before, between and after the project's, such as a fake boot screen, an intro or a game during Copy files;
- bring fonts, music and sound effects, with a mute control the engine always provides;
- declare which renderers it supports, with fallbacks for the rest.

A theme can't add install operations; only the project can. That keeps a theme from a gallery safe to try.

## 6 · Building in CI

The CLI is a single Rust binary for Linux, macOS, Windows and FreeBSD hosts, with ready-made steps for GitHub Actions and Forgejo/Gitea Actions. The product's own CI builds its own outputs, using each platform's tools on runners that have them.

| Platform | Themed installer | Native packages |
|---|---|---|
| DOS | Floppy images or a ZIP with `SETUP.EXE` | None |
| Windows 3.1 | Floppy images with `SETUP.EXE` | None |
| Windows | Setup `.exe` (engine and payload) | MSI (WiX), MSIX |
| Linux | Self-extracting `.run` | .deb, .rpm, Arch package |
| macOS | Installer `.app` in a disk image | `.pkg` (notarized) |
| FreeBSD | Self-extracting `.run` | `pkg` |

Table 2. Outputs by platform.

Native packages are built from the same manifest: the CLI writes their control files, spec files, PKGBUILDs or WiX sources and runs the platform's tool. They install without our interface, but they write the same receipt, so the Hub and the themed installer can update, repair or remove either kind. On Windows the themed installer can also install the MSI silently underneath it, for organizations that require MSI.

## 7 · Feeds, the web stub and the Hub

A **feed** is a signed JSON file that a publisher hosts anywhere. It lists products, channels (stable, beta), versions, platforms, artefacts with sizes and hashes, and the themes available for each.

- The **web stub** is a small themed installer that carries no payload. It reads one product's entry in a feed and downloads only what the person chooses.
- The **Hub** is a themed launcher that reads one or more feeds and lists their products, with install, update, launch and remove. It is the engine's lifecycle with a library around it. Each feed has its own pinned key.

Feed keys are pinned at build time. A feed signed with another key is refused, even from the expected address.

## 8 · Design Studio and agents

The Studio is a Tauri app built with the Liminal HQ design system. It opens an installer project from disk and offers Overview, Flow, Layouts, Scripts and Dry-run preview screens, with Feeds and Builds to follow.

- The **layout editor** draws each step with the real renderer code, compiled for the Studio, so what you see is what ships.
- The **preview** runs the engine against a simulated machine. For retro targets it can also start a disk image in 86Box or DOSBox-X and run the real build there.
- An **MCP server** on localhost, protected by a token, let an agent read the project, change flows and layouts, run previews and read their results. Agent changes appear live and can be undone like your own.

## 9 · Security and trust

- Payloads, feeds and themes are signed with Ed25519 keys. Platform signing (Authenticode, Apple notarization, distribution keys) is added on top where it exists.
- Scripts run in a sandbox and get only the capabilities they declare. Network access is never granted by default.
- Privileged work happens only in the helper, and only for the operations in the signed plan.
- No telemetry. The engine contacts nothing but the feeds and download addresses the project names.
- Builds aim to be reproducible: the same project and inputs give the same bytes.
