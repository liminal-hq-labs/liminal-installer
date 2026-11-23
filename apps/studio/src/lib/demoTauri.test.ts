// Tests the browser demo's stand-in for the Tauri import.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { demoTauri } from './demoTauri';

describe('the demo Tauri state', () => {
	it('starts one import behind, and an import brings it up to date', () => {
		const demo = demoTauri();
		const before = demo.view();
		expect(before.stale).toBe(true);
		expect(before.pending).toEqual([{ field: 'product.version', from: '1.4.1', to: '1.4.2' }]);
		const result = demo.importConfig();
		expect(result.changes).toHaveLength(1);
		expect(result.view.stale).toBe(false);
		expect(result.view.identity.find((r) => r.field === 'version')?.project).toBe('1.4.2');
	});

	it('keeps an overridden field through an import', () => {
		const demo = demoTauri();
		demo.set({ overrides: ['version'] });
		const result = demo.importConfig();
		expect(result.kept).toEqual(['version']);
		expect(result.view.identity.find((r) => r.field === 'version')?.project).toBe('1.4.1');
	});

	it('reads an empty WebView2 override as no override, and restores from a snapshot', () => {
		const demo = demoTauri();
		const start = demo.snapshot();
		demo.set({ webview2: { mode: 'offlineInstaller' } });
		expect(demo.view().webview2?.effective.mode).toBe('offlineInstaller');
		expect(demo.view().webview2?.if_missing).toBe('offline');
		demo.set({ webview2: {} });
		expect(demo.view().webview2?.override).toBeNull();
		demo.set({ windows: 'wrap' });
		demo.restore(start);
		expect(demo.view().windows).toBe('native');
	});
});
