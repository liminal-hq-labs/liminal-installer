// The state the window chrome shows, kept outside React so the app, the menus and the dialogs share one source.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Settings, SettingsPort } from '../lib/settings';
import { defaultSettings, pushRecent, sameSettings } from '../lib/settings';
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
	/** Where the settings live; Preferences reads and writes through it. */
	readonly port: SettingsPort;
	get(): ChromeState;
	subscribe(listener: () => void): () => void;
	/** Reads the settings and follows changes made in any window. Resolves once the first read is in. */
	init(): Promise<void>;
	/** Merges the parts of the menu state the app knows. The menu bar flag comes from the settings. */
	setMenu(patch: Partial<Omit<MenuState, 'menuBar' | 'recent'>>): void;
	/** Puts a project at the front of the recent list and remembers it as the last one. */
	recordProject(path: string): Promise<void>;
	/** Drops a project from the recent list, such as one that has moved. */
	forgetProject(path: string): Promise<void>;
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
		if (sameSettings(settings, state.settings)) return;
		set({ ...state, settings, menu: { ...state.menu, menuBar: settings.menuBar, recent: settings.recentProjects } });
	};
	return {
		port,
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
		async recordProject(path) {
			const { recentProjects, recentLimit, lastProject } = state.settings;
			const recent = pushRecent(recentProjects, path, recentLimit);
			if (lastProject === path && recent.join('\n') === recentProjects.join('\n')) return;
			await this.updateSettings({ recentProjects: recent, lastProject: path });
		},
		async forgetProject(path) {
			if (!state.settings.recentProjects.includes(path)) return;
			await this.updateSettings({ recentProjects: state.settings.recentProjects.filter((p) => p !== path) });
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
