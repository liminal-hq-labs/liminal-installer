# Local gate: the same checks CI runs.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

.PHONY: validate fmt clippy test headers js markdown e2e-linux

validate: fmt clippy test headers js

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
		bun install --frozen-lockfile && bun run format:check && bun run typecheck && bun run test:js; \
	else echo "bun not found: skipping the JavaScript checks"; fi

markdown:
	docker run --rm -v "$$PWD:/workdir" davidanson/markdownlint-cli2:v0.17.2 "**/*.md"

e2e-linux:
	bash scripts/e2e-linux.sh
