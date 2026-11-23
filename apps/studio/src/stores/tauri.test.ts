// Tests the Tauri store: loading, importing, changing settings and forgetting.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { StudioError, type Api } from '../lib/api';
import { tauriViewFixture } from '../lib/tauriFixture';
import type { TauriImportResult } from '../lib/types';
import { createTauri } from './tauri';

const result = (patch: Partial<TauriImportResult> = {}): TauriImportResult => ({
	view: tauriViewFixture({ imported: true }),
	first_import: false,
	changes: [{ field: 'product.version', from: '1.4.1', to: '1.4.2' }],
	filled: [],
	kept: ['name'],
	components: [],
	warnings: [],
	...patch,
});

function make(api: Partial<Api> = {}) {
	const fake = {
		getTauri: vi.fn(async () => tauriViewFixture()),
		importTauri: vi.fn(async () => result()),
		setTauriSettings: vi.fn(async () => tauriViewFixture({ windows: 'wrap' })),
		...api,
	} as unknown as Api;
	return { tauri: createTauri(fake), api: fake };
}

describe('the Tauri store', () => {
	it('loads the view', async () => {
		const { tauri } = make();
		const loading = tauri.load();
		expect(tauri.store.getState().loading).toBe(true);
		await loading;
		const s = tauri.store.getState();
		expect([s.loading, s.error, s.view?.config]).toEqual([
			false,
			null,
			'src-tauri/tauri.conf.json',
		]);
	});

	it('keeps a failed load as a message and leaves the view it had', async () => {
		const getTauri = vi
			.fn()
			.mockResolvedValueOnce(tauriViewFixture())
			.mockRejectedValueOnce(new StudioError('io', 'tauri.conf.json: denied'));
		const { tauri } = make({ getTauri });
		await tauri.load();
		await tauri.load();
		const s = tauri.store.getState();
		expect(s.error).toBe('tauri.conf.json: denied');
		expect(s.view).not.toBeNull();
		expect(s.loading).toBe(false);
	});

	it('imports a path with a label, keeps the result and takes the view from it', async () => {
		const { tauri, api } = make();
		expect(await tauri.importConfig('/p/src-tauri/tauri.conf.json')).toBe(true);
		expect(api.importTauri).toHaveBeenCalledWith(
			'/p/src-tauri/tauri.conf.json',
			'Import tauri.conf.json',
		);
		const s = tauri.store.getState();
		expect(s.lastImport?.kept).toEqual(['name']);
		expect(s.view?.imported).toBe(true);
		expect(s.busy).toBe(false);
	});

	it('re-imports the project’s own config with null', async () => {
		const { tauri, api } = make();
		await tauri.importConfig(null);
		expect(api.importTauri).toHaveBeenCalledWith(null, 'Import tauri.conf.json again');
	});

	it('shows an import that failed and keeps the last good result', async () => {
		const importTauri = vi
			.fn()
			.mockResolvedValueOnce(result())
			.mockRejectedValueOnce({ kind: 'invalid_input', message: 'not valid JSON5' });
		const { tauri } = make({ importTauri });
		await tauri.importConfig(null);
		expect(await tauri.importConfig('/bad')).toBe(false);
		const s = tauri.store.getState();
		expect(s.error).toBe('not valid JSON5');
		expect(s.lastImport).not.toBeNull();
		expect(s.busy).toBe(false);
	});

	it('sends a settings change and takes the new view', async () => {
		const { tauri, api } = make();
		expect(await tauri.setSettings({ windows: 'wrap' })).toBe(true);
		expect(api.setTauriSettings).toHaveBeenCalledWith(
			{ windows: 'wrap' },
			'Install the Tauri app on Windows as wrap',
		);
		expect(tauri.store.getState().view?.windows).toBe('wrap');
		await tauri.setSettings({ webview2: {} }, 'Custom label');
		expect(api.setTauriSettings).toHaveBeenLastCalledWith({ webview2: {} }, 'Custom label');
	});

	it('shows a settings change that failed', async () => {
		const { tauri } = make({
			setTauriSettings: vi.fn(async () => {
				throw new StudioError('project', 'the project has no [tauri] section');
			}),
		});
		expect(await tauri.setSettings({ windows: 'wrap' })).toBe(false);
		expect(tauri.store.getState().error).toBe('the project has no [tauri] section');
	});

	it('does not let a slow, older load overwrite a newer result', async () => {
		let release!: () => void;
		const slow = new Promise<void>((r) => (release = r));
		const getTauri = vi.fn(async () => {
			await slow;
			return tauriViewFixture({ windows: 'native' });
		});
		const { tauri } = make({ getTauri });
		const loading = tauri.load();
		await tauri.setSettings({ windows: 'wrap' });
		release();
		await loading;
		expect(tauri.store.getState().view?.windows).toBe('wrap');
	});

	it('forgets everything on clear, and a load in flight does not bring it back', async () => {
		let release!: () => void;
		const slow = new Promise<void>((r) => (release = r));
		const { tauri } = make({
			getTauri: vi.fn(async () => {
				await slow;
				return tauriViewFixture();
			}),
		});
		const loading = tauri.load();
		tauri.clear();
		release();
		await loading;
		expect(tauri.store.getState()).toMatchObject({ view: null, lastImport: null, error: null });
	});
});
