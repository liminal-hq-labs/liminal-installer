# Local gate: the same checks CI runs.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

.PHONY: validate fmt clippy test headers js i18n bindings markdown e2e-linux dos

validate: fmt clippy test headers js bindings

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings

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
