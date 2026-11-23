// Tests that the Tauri commands are called by the names and arguments the shell registers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createApi } from './api';

const invoke = vi.fn();
const open = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => undefined }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: (...args: unknown[]) => open(...args) }));

describe('the Tauri commands', () => {
	beforeEach(() => {
		invoke.mockReset().mockResolvedValue({});
		open.mockReset();
		vi.stubGlobal('window', { __TAURI_INTERNALS__: {} });
	});

	it('reads the view with no arguments', async () => {
		const api = await createApi();
		await api.getTauri();
		expect(invoke).toHaveBeenCalledWith('get_tauri', undefined);
	});

	it('imports with config and label, and null for the project’s own config', async () => {
		const api = await createApi();
		await api.importTauri('/p/tauri.conf.json', 'Import');
		expect(invoke).toHaveBeenLastCalledWith('import_tauri', {
			config: '/p/tauri.conf.json',
			label: 'Import',
		});
		await api.importTauri(null, 'Again');
		expect(invoke).toHaveBeenLastCalledWith('import_tauri', { config: null, label: 'Again' });
	});

	it('changes settings with change and label', async () => {
		const api = await createApi();
		await api.setTauriSettings({ webview2: {} }, 'Reset');
		expect(invoke).toHaveBeenCalledWith('set_tauri_settings', {
			change: { webview2: {} },
			label: 'Reset',
		});
	});

	it('picks a file with a JSON filter and starts in the folder it is given', async () => {
		open.mockResolvedValueOnce('/p/tauri.conf.json').mockResolvedValueOnce(null);
		const api = await createApi();
		expect(await api.pickTauriConfig('/p')).toBe('/p/tauri.conf.json');
		expect(open).toHaveBeenCalledWith(
			expect.objectContaining({
				directory: false,
				defaultPath: '/p',
				filters: [expect.objectContaining({ extensions: expect.arrayContaining(['json']) })],
			}),
		);
		expect(await api.pickTauriConfig()).toBeNull();
	});
});
