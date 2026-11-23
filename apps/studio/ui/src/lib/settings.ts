// The Studio's settings: a file in the desktop shell, the browser's storage in the browser demo.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export const THEMES = ['afterglow', 'afterglow-dark', 'afterglow-light'] as const;
export type ThemeId = (typeof THEMES)[number];

export interface Settings {
	/** Whether the conventional menu bar shows under the title bar. */
	menuBar: boolean;
	theme: ThemeId;
}

export const defaultSettings: Settings = { menuBar: false, theme: 'afterglow' };

/** Where the settings live and how a window hears about a change made in another window. */
export interface SettingsPort {
	read(): Promise<Settings>;
	/** Merges `patch` into the saved settings and resolves with the result. Rejects with the reason when refused. */
	write(patch: Partial<Settings>): Promise<Settings>;
	/** Calls `listener` after any window changes the settings; resolves to an unsubscribe function. */
	onChange(listener: (settings: Settings) => void): Promise<() => void>;
}

/** The settings in `value`, with the default for every field that is missing or of the wrong type. */
export function normalizeSettings(value: unknown): Settings {
	const v = (value && typeof value === 'object' ? value : {}) as Record<string, unknown>;
	return {
		menuBar: typeof v.menuBar === 'boolean' ? v.menuBar : defaultSettings.menuBar,
		theme: THEMES.includes(v.theme as ThemeId) ? (v.theme as ThemeId) : defaultSettings.theme,
	};
}

export const SETTINGS_EVENT = 'studio://settings-changed';

export async function tauriSettings(): Promise<SettingsPort> {
	const { invoke } = await import('@tauri-apps/api/core');
	const { listen } = await import('@tauri-apps/api/event');
	return {
		read: async () => normalizeSettings(await invoke('read_settings')),
		write: async (patch) => normalizeSettings(await invoke('write_settings', { patch })),
		onChange: (listener) => listen<Settings>(SETTINGS_EVENT, (e) => listener(normalizeSettings(e.payload))),
	};
}

/** The narrow part of `Storage` that is used. */
export interface KeyValueStore {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

const KEY = 'liminal-installer-studio.settings';

/** Settings kept in browser storage. Blocked or broken storage means the defaults, and a write still holds for the session. */
export function browserSettings(storage: KeyValueStore | undefined): SettingsPort {
	const listeners = new Set<(s: Settings) => void>();
	let current: Settings | undefined;
	const load = (): Settings => {
		try {
			const raw = storage?.getItem(KEY);
			return normalizeSettings(raw ? JSON.parse(raw) : {});
		} catch {
			return { ...defaultSettings };
		}
	};
	return {
		read: async () => (current ??= load()),
		async write(patch) {
			const next = normalizeSettings({ ...(current ??= load()), ...patch });
			current = next;
			try {
				storage?.setItem(KEY, JSON.stringify(next));
			} catch {
				// Storage can be blocked or full; the choice then lasts until the page closes.
			}
			for (const l of [...listeners]) l(next);
			return next;
		},
		async onChange(listener) {
			listeners.add(listener);
			return () => void listeners.delete(listener);
		},
	};
}
