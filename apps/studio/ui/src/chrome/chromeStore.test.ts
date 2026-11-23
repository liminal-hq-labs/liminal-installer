// Tests the chrome store, which keeps the menu bar choice in the settings.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { browserSettings, defaultSettings, normalizeSettings, type KeyValueStore, type Settings, type SettingsPort } from '../lib/settings';
import { createChromeStore } from './chromeStore';

const memory = (): KeyValueStore & { data: Map<string, string> } => {
	const data = new Map<string, string>();
	return { data, getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
};

const throwing: KeyValueStore = {
	getItem() {
		throw new Error('blocked');
	},
	setItem() {
		throw new Error('blocked');
	},
};

describe('settings in the browser', () => {
	it('fills in defaults for anything missing or wrong', () => {
		expect(normalizeSettings(undefined)).toEqual(defaultSettings);
		expect(normalizeSettings({ menuBar: 'yes', theme: 'neon' })).toEqual(defaultSettings);
		expect(normalizeSettings({ menuBar: true, theme: 'afterglow-light' })).toEqual({ menuBar: true, theme: 'afterglow-light' });
	});

	it('keeps a write for the next reader', async () => {
		const storage = memory();
		await browserSettings(storage).write({ menuBar: true });
		expect((await browserSettings(storage).read()).menuBar).toBe(true);
	});

	it('survives storage that throws or is missing', async () => {
		expect(await browserSettings(throwing).read()).toEqual(defaultSettings);
		const port = browserSettings(throwing);
		await expect(port.write({ menuBar: true })).resolves.toMatchObject({ menuBar: true });
		expect((await port.read()).menuBar).toBe(true);
		expect(await browserSettings(undefined).read()).toEqual(defaultSettings);
	});
});

/** A port like the desktop's: a shared file, with a change reaching every listener. */
function sharedPort(initial: Settings = defaultSettings): SettingsPort & { current: Settings } {
	const listeners = new Set<(s: Settings) => void>();
	const port = {
		current: initial,
		read: async () => port.current,
		write: async (patch: Partial<Settings>) => {
			port.current = { ...port.current, ...patch };
			for (const l of listeners) l(port.current);
			return port.current;
		},
		onChange: async (l: (s: Settings) => void) => {
			listeners.add(l);
			return () => void listeners.delete(l);
		},
	};
	return port;
}

describe('menu bar setting', () => {
	it('starts hidden and shows the saved choice after init', async () => {
		const store = createChromeStore(sharedPort({ ...defaultSettings, menuBar: true }));
		expect(store.get().menu.menuBar).toBe(false);
		await store.init();
		expect(store.get().menu.menuBar).toBe(true);
	});

	it('writes the toggle to the settings, where a new store finds it', async () => {
		const port = sharedPort();
		const store = createChromeStore(port);
		await store.init();
		await store.toggleMenuBar();
		expect(port.current.menuBar).toBe(true);
		expect(store.get().menu.menuBar).toBe(true);
		const next = createChromeStore(port);
		await next.init();
		expect(next.get().menu.menuBar).toBe(true);
		await next.toggleMenuBar();
		expect(port.current.menuBar).toBe(false);
	});

	it('follows a change made in another window, such as Preferences', async () => {
		const port = sharedPort();
		const main = createChromeStore(port);
		await main.init();
		await port.write({ menuBar: true, theme: 'afterglow-light' });
		expect(main.get().menu.menuBar).toBe(true);
		expect(main.get().settings.theme).toBe('afterglow-light');
	});

	it('leaves the state alone when a write is refused', async () => {
		const port = sharedPort();
		port.write = async () => {
			throw new Error('refused');
		};
		const store = createChromeStore(port);
		await store.init();
		await expect(store.toggleMenuBar()).rejects.toThrow('refused');
		expect(store.get().menu.menuBar).toBe(false);
	});

	it('starts from the defaults when the settings cannot be read', async () => {
		const port = sharedPort();
		port.read = async () => {
			throw new Error('unreadable');
		};
		const store = createChromeStore(port);
		await store.init();
		expect(store.get().settings).toEqual(defaultSettings);
	});
});

describe('chrome store', () => {
	it('notifies on a change and not on a repeat', async () => {
		const store = createChromeStore(sharedPort());
		const listener = vi.fn();
		const stop = store.subscribe(listener);
		store.setMenu({ canUndo: true });
		store.setMenu({ canUndo: true });
		store.setTitle('A');
		store.setTitle('A');
		expect(listener).toHaveBeenCalledTimes(2);
		stop();
		store.setTitle('B');
		expect(listener).toHaveBeenCalledTimes(2);
	});

	it('keeps the menu bar flag when the app updates the menu state', async () => {
		const store = createChromeStore(sharedPort());
		await store.init();
		await store.toggleMenuBar();
		store.setMenu({ hasProject: true, screen: 'flow' });
		expect(store.get().menu).toMatchObject({ menuBar: true, hasProject: true, screen: 'flow' });
	});
});
