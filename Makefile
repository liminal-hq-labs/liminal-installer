# Local gate: the same checks CI runs.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

.PHONY: validate fmt clippy test headers markdown e2e-linux

validate: fmt clippy test headers

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings

test:
	cargo nextest run --workspace --locked

headers:
	bash scripts/check-headers.sh

markdown:
	docker run --rm -v "$$PWD:/workdir" davidanson/markdownlint-cli2:v0.17.2 "**/*.md"

e2e-linux:
	bash scripts/e2e-linux.sh
