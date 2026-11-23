// Checks that the Vite dev server and Tauri's `devUrl` agree on one fixed port.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import config from '../vite.config';

const tauriConfig = JSON.parse(
	readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'),
) as { identifier: string; build: { devUrl: string } };

describe('dev server', () => {
	it('serves on the port Tauri loads, and fails rather than moving', () => {
		expect(config.server?.strictPort).toBe(true);
		expect(config.server?.port).toBe(1420);
		expect(new URL(tauriConfig.build.devUrl).port).toBe(String(config.server?.port));
	});

	it('uses the house identifier, with no separators in the app name', () => {
		expect(tauriConfig.identifier).toBe('ca.liminalhq.installerstudio');
	});
});
