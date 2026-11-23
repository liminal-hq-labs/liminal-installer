// Vite and Vitest settings for the runner's page.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'node:url';

const repoRoot = fileURLToPath(new URL('../../', import.meta.url));

export default defineConfig({
	plugins: [react()],
	clearScreen: false,
	server: {
		// The Studio has 1420; the runner takes the next port so both can run side by side.
		port: 1421,
		strictPort: true,
		// The Rust build writes under src-tauri; a rebuild there is not a page change.
		watch: { ignored: ['**/src-tauri/**'] },
		fs: { allow: [repoRoot] },
	},
	build: { target: 'es2022', outDir: 'dist' },
	test: {
		environment: 'node',
		include: ['src/**/*.test.{ts,tsx}'],
		css: { modules: { classNameStrategy: 'non-scoped' } },
	},
});
