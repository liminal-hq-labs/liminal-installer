// Vite and Vitest settings for the Studio UI.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { defineConfig } from 'vite';
import { fileURLToPath } from 'node:url';

const repoRoot = fileURLToPath(new URL('../../../', import.meta.url));

export default defineConfig({
	clearScreen: false,
	server: {
		port: 1420,
		strictPort: true,
		// The browser demo reads the example projects from the repository.
		fs: { allow: [repoRoot] },
	},
	build: { target: 'es2022', outDir: 'dist' },
	test: { environment: 'node', include: ['src/**/*.test.{ts,tsx}', 'scripts/**/*.test.ts'] },
});
