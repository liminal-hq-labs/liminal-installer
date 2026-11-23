// The state the window chrome shows, kept outside React so the app, the menus and the dialogs share one source.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Settings, SettingsPort } from '../lib/settings';
import { defaultSettings } from '../lib/settings';
import type { DialogSpec, DialogResult } from './dialogWindow';
import { initialMenuState, type MenuState } from './menuModel';

/** A dialog drawn in the page, for the browser demo that has no windows to open. */
export interface InPageDialog {
	spec: DialogSpec;
	done(result: DialogResult): void;
}

export interface ChromeState {
	menu: MenuState;
	settings: Settings;
	/** The title bar's centre text. */
	title: string;
	dialog: InPageDialog | null;
}

export interface ChromeStore {
	get(): ChromeState;
	subscribe(listener: () => void): () => void;
	/** Reads the settings and follows changes made in any window. Resolves once the first read is in. */
	init(): Promise<void>;
	/** Merges the parts of the menu state the app knows. The menu bar flag comes from the settings. */
	setMenu(patch: Partial<Omit<MenuState, 'menuBar'>>): void;
	setTitle(title: string): void;
	setDialog(dialog: InPageDialog | null): void;
	/** Writes the `menuBar` setting the other way round. */
	toggleMenuBar(): Promise<void>;
	/** Writes any settings. A refusal leaves them as they were and is thrown to the caller. */
	updateSettings(patch: Partial<Settings>): Promise<void>;
}

export function createChromeStore(port: SettingsPort): ChromeStore {
	let state: ChromeState = { menu: { ...initialMenuState }, settings: { ...defaultSettings }, title: '', dialog: null };
	const listeners = new Set<() => void>();
	const set = (next: ChromeState) => {
		state = next;
		for (const l of [...listeners]) l();
	};
	const applySettings = (settings: Settings) => {
		if (settings.menuBar === state.settings.menuBar && settings.theme === state.settings.theme) return;
		set({ ...state, settings, menu: { ...state.menu, menuBar: settings.menuBar } });
	};
	return {
		get: () => state,
		subscribe(listener) {
			listeners.add(listener);
			return () => void listeners.delete(listener);
		},
		async init() {
			await port.onChange(applySettings);
			applySettings(await port.read().catch(() => ({ ...defaultSettings })));
		},
		setMenu(patch) {
			const menu = { ...state.menu, ...patch };
			if ((Object.keys(menu) as (keyof MenuState)[]).every((k) => menu[k] === state.menu[k])) return;
			set({ ...state, menu });
		},
		setTitle(title) {
			if (title !== state.title) set({ ...state, title });
		},
		setDialog(dialog) {
			set({ ...state, dialog });
		},
		async toggleMenuBar() {
			await this.updateSettings({ menuBar: !state.settings.menuBar });
		},
		async updateSettings(patch) {
			applySettings(await port.write(patch));
		},
	};
}
