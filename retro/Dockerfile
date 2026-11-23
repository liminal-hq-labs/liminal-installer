# Open Watcom v2 build image for the DOS 386 extender runtime, with DOSBox-X for headless smoke tests.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

FROM debian:trixie-slim

# A dated snapshot, so the compiler does not change under the build. Bump it on purpose.
ARG OW_TAG=2026-11-01-Build

RUN apt-get update \
	&& apt-get install -y --no-install-recommends ca-certificates curl xz-utils dosbox-x \
	&& rm -rf /var/lib/apt/lists/*

RUN mkdir -p /opt/watcom \
	&& curl -fsSL "https://github.com/open-watcom/open-watcom-v2/releases/download/${OW_TAG}/ow-snapshot.tar.xz" \
		| tar -xJ -C /opt/watcom

ENV WATCOM=/opt/watcom
ENV PATH=/opt/watcom/binl64:/opt/watcom/binl:$PATH:/opt/watcom/binw
ENV INCLUDE=/opt/watcom/h
ENV EDPATH=/opt/watcom/eddat

WORKDIR /work
