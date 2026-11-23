// Vitest settings for the shared renderer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { defineConfig } from 'vitest/config';

export default defineConfig({
	test: {
		environment: 'node',
		include: ['src/**/*.test.{ts,tsx}'],
		// Class names stay as written in the stylesheet, so a test can look for `pvStage`.
		css: { modules: { classNameStrategy: 'non-scoped' } },
	},
});
