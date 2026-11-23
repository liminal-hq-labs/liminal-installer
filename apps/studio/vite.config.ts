// Vite and Vitest settings for the Studio UI.
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
		port: 1420,
		strictPort: true,
		// The Rust build writes under src-tauri; a rebuild there is not a page change.
		watch: { ignored: ['**/src-tauri/**'] },
		// The browser demo reads the example projects from the repository.
		fs: { allow: [repoRoot] },
	},
	build: { target: 'es2022', outDir: 'dist' },
	test: {
		environment: 'node',
		include: ['src/**/*.test.{ts,tsx}', 'scripts/**/*.test.ts'],
		// Class names stay as written in the stylesheet, so a test can look for `pvStage`.
		css: { modules: { classNameStrategy: 'non-scoped' } },
	},
});
