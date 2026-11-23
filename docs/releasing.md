# Releasing

How Liminal Installer is built and published, what needs secrets, and what has only been checked in CI. Decision D10 in [decisions.md](decisions.md) records why.

## Table of contents

- [Workflows](#workflows)
- [The reusable action](#the-reusable-action)
- [Secrets](#secrets)
- [Cutting a release](#cutting-a-release)
- [What is verified where](#what-is-verified-where)

## Workflows

| File | Runs on | What it does |
|---|---|---|
| `.github/workflows/ci.yml` | Pull requests and `main` | Lint, headers, banner, format, clippy and tests. One job per check |
| `.github/workflows/release.yml` | Tags `v*` and manual dispatch | Builds, packages and publishes. Never runs on pull requests |
| `.github/workflows/studio.yml` | Pull requests and `main` that touch `apps/studio`, tags and dispatch | Typecheck, Vitest, `cargo check` and clippy for the Tauri Studio, then `tauri build` on tags and dispatch |

Table 1. Workflows.

`release.yml` has four stages:

- **Build:** `scripts/ci-stage.sh` builds `li-cli` and, when the workspace has it, `li-runner`, then archives the binaries as `liminal-installer-<platform>.tar.gz`. The platforms are `linux-x86_64` (`ubuntu-24.04`), `linux-aarch64` (`ubuntu-24.04-arm`), `windows-x86_64`, `macos-aarch64` (`macos-latest`) and `macos-x86_64` (`macos-15-intel`, the successor to the retired `macos-13`). A FreeBSD job runs in a virtual machine through `cross-platform-actions/action` and is allowed to fail.
- **Package:** `scripts/ci-package.sh` runs `liminst check` and `liminst build` for every `examples/*/installer` project that declares the platform's target, then finishes the native packages. It does nothing while `examples/` is empty.
  - Linux: `.run`, `.deb`, `.rpm` and Arch sources come straight from `liminst build`.
  - Windows: WiX 5 is installed as a dotnet tool and builds an `.msi` from the generated `.wxs`, with `SourceDir` set to the project's `installer/` folder. `makeappx` packs an unsigned `.msix` when the Windows SDK has it.
  - macOS: `pkgbuild` and `productbuild` make the `.pkg` from the generated distribution XML, and `hdiutil` wraps the installer `.app` in a `.dmg`.
- **Sign:** optional steps sign with `signtool`, `productsign` and `notarytool` when their secrets are set.
- **Publish:** only on a tag. Collects every artefact, writes `SHA256SUMS` with `sha256sum`, and runs `gh release create --generate-notes`. The notes are grouped by pull request label through `.github/release.yml`. A tag with a hyphen, such as `v0.2.0-rc.1`, is marked as a pre-release. Only this job has `contents: write`.

## The reusable action

`.github/actions/installer-action` is the composite action behind SPEC Example 2. Use it from another repository as `liminal-hq-labs/liminal-installer/.github/actions/installer-action@<tag>`, or locally with `from-source: true`.

| Input | Meaning |
|---|---|
| `targets` | Comma-separated target ids to build |
| `project-dir` | The folder that holds `installer/` (default `.`) |
| `feed`, `feed-url` | The feed file to update and the URL prefix where built files will be hosted |
| `sign-key` | The feed signing key, passed as a secret. Written to a private temporary file and removed afterwards |
| `version`, `from-source`, `output-dir` | Which `liminst` to use and where outputs go |

Table 2. Action inputs.

It installs `liminst` (from a release, or with `cargo install` when `from-source` is set), runs `liminst check`, runs `liminst build` for each target, and then `liminst feed add` for each `.run` or `.lip` output. `liminst feed add --sign-key` is not implemented yet and exits with an error, so leave `sign-key` empty until it is.

## Secrets

Every secret is optional. A missing secret skips its step, and the workflow never prints a secret value.

| Secret | Used for |
|---|---|
| `WINDOWS_CERT_PFX`, `WINDOWS_CERT_PASSWORD` | Base64 PFX and its password, for signing `.msi` and `.msix` |
| `APPLE_CERT_P12`, `APPLE_CERT_PASSWORD` | Base64 Developer ID certificate and its password, imported into a temporary keychain |
| `APPLE_INSTALLER_IDENTITY` | The "Developer ID Installer" identity that `productsign` uses |
| `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` | Notarisation with `notarytool`, and stapling |
| `FEED_KEY` | Passed by a consumer to the action's `sign-key` |

Table 3. Secrets.

## Cutting a release

1. Merge to `main` with the checks green.
2. Tag the commit: `git tag v0.1.0 && git push origin v0.1.0`.
3. Watch the `Release` run. A failed FreeBSD job does not block publishing; a failed build job does.

To rehearse without publishing, dispatch `Release` by hand. It builds and packages but skips the publish job, because that job requires a tag.

## What is verified where

Checked locally: `actionlint` on every workflow and the action, `shellcheck` on `scripts/ci-stage.sh` and `scripts/ci-package.sh`, header and markdown checks, and the DOS spike in `retro/`.

Only CI can check:

- The Windows, macOS, `ubuntu-24.04-arm` and FreeBSD builds.
- WiX, `makeappx`, `productbuild`, `hdiutil`, signing and notarisation.
- The `gh release create` step and the grouped notes.
- The Studio workflow, until `apps/studio` lands.
- The packaging scripts against real examples, because `examples/` does not exist yet. The macOS `.pkg` and Windows `.msi` are built from an empty payload root and the project's `installer/` folder respectively, so adjust the scripts once examples define their staged files.
