# The DOSBox-X image the Windows 95 test bed runs in. Build: docker build -t liminal-win95-dosbox-x:local -f scripts/win95/dosbox-x.Dockerfile scripts/win95
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends dosbox-x ca-certificates && rm -rf /var/lib/apt/lists/*
