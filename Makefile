# Local gate: the same checks CI runs.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

.PHONY: validate fmt clippy test headers js i18n bindings markdown e2e-linux dos win32 win16

validate: fmt clippy test headers js bindings

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings
	cargo clippy -p li-lpk --no-default-features --locked -- -D warnings

test:
	cargo nextest run --workspace --locked

headers:
	bash scripts/check-headers.sh

# The Studio UI checks need Bun; without it they are skipped with a note.
js:
	@if command -v bun >/dev/null 2>&1; then \
		bun install --frozen-lockfile && bun run format:check && bun run typecheck && bun run check:i18n && bun run test:js; \
	else echo "bun not found: skipping the JavaScript checks"; fi

# The message catalogue and the components agree (also part of `js`).
i18n:
	bun run check:i18n

# Regenerates the ts-rs bindings and fails if they differ from the committed ones.
bindings:
	bash scripts/check-bindings-drift.sh

markdown:
	docker run --rm -v "$$PWD:/workdir" davidanson/markdownlint-cli2:v0.17.2 "**/*.md"

e2e-linux:
	bash scripts/e2e-linux.sh

# The DOS runtime: fetch DOSBox Staging and CWSDPMI (DOS_VENDOR, default dos/vendor), check, build and run it headless, then install
# and uninstall Lt. Fizz under DOSBox. Not part of `validate`, which needs no DOS tools. Without the tools the DOSBox steps skip.
dos:
	bash dos/tools/fetch-tools.sh || echo "dos: the DOS tools could not be fetched; the DOSBox steps are skipped"
	bash dos/check.sh
	bash dos/tools/e2e.sh

# The Win32 runtime for Windows 95 and later: format and lints, then (with lld-link and llvm-objdump) build SETUP.EXE and check that it holds only
# Pentium instructions, imports only allowlisted ANSI functions and is a Windows 95 PE; then the host tests of the Win32 flow and, with Wine and
# Xvfb, a real install and uninstall of Lt. Fizz and Planboard. Not part of `validate`, which needs none of these tools.
win32:
	bash win32/check.sh
	cargo nextest run -p li-retro -p li-retro-build --locked
	cargo nextest run -p li-cli -E 'binary(win32)' --locked
	bash win32/tools/wine-e2e.sh

# The Windows 3.1 runtime: the host tests of its crates (the flow against DOS's, the compiler, the pack), then retro/win16's checks (format,
# lints, the packer's tests, the build, the NE check and the size report), then, when Wine is installed, an install and uninstall of Lt. Fizz
# and Planboard under it. Not part of `validate`: it needs the nightly build-std setup and Wine. Windows 3.11 under DOSBox-X is a separate,
# manual run (retro/win16/tools/e2e-w31.sh) because it needs a Windows 3.11 tree the repository cannot carry.
win16:
	cargo test --locked -p li-retro -p li-retro-build
	bash retro/win16/check.sh
	bash retro/win16/tools/e2e-wine.sh
