// The Tauri store: what the project's Tauri import and settings look like now, and the actions that change them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import { t } from '../i18n/messages';
import { errorMessage, type Api } from '../lib/api';
import { baseName, settingsLabel } from '../lib/tauri';
import type { TauriImportResult, TauriSettingsChange, TauriView } from '../lib/types';

export interface TauriState {
	/** The project's Tauri state, or null before the first load and while no project is open. */
	view: TauriView | null;
	/** A load is in flight. */
	loading: boolean;
	/** An import or a settings change is in flight. */
	busy: boolean;
	/** Why the last load, import or change failed. */
	error: string | null;
	/** What the last import did, for the Import card. */
	lastImport: TauriImportResult | null;
}

export const initialTauriState: TauriState = {
	view: null,
	loading: false,
	busy: false,
	error: null,
	lastImport: null,
};

export type TauriStore = StoreApi<TauriState>;

export interface Tauri {
	readonly store: TauriStore;
	/** Reads the view again. A newer read wins over an older one still in flight. */
	load(): Promise<void>;
	/** Imports `path`, or the project's own `[tauri]` config again when null. True when it worked. */
	importConfig(path: string | null): Promise<boolean>;
	/** Changes the Tauri settings; `label` names the history entry. True when it worked. */
	setSettings(change: TauriSettingsChange, label?: string): Promise<boolean>;
	/** Forgets everything, as closing the project or opening another does. */
	clear(): void;
}

export function createTauri(api: Api): Tauri {
	const store = createStore<TauriState>(() => ({ ...initialTauriState }));
	let generation = 0;

	async function load(): Promise<void> {
		const mine = ++generation;
		store.setState({ loading: true });
		try {
			const view = await api.getTauri();
			if (mine === generation) store.setState({ view, loading: false, error: null });
		} catch (e) {
			if (mine === generation) store.setState({ loading: false, error: errorMessage(e) });
		}
	}

	async function importConfig(path: string | null): Promise<boolean> {
		const label = path
			? t('history.importTauri', { file: baseName(path) })
			: t('history.reimportTauri');
		store.setState({ busy: true, error: null });
		try {
			const result = await api.importTauri(path, label);
			generation++;
			store.setState({ view: result.view, lastImport: result, busy: false, error: null });
			return true;
		} catch (e) {
			store.setState({ busy: false, error: errorMessage(e) });
			return false;
		}
	}

	async function setSettings(change: TauriSettingsChange, label?: string): Promise<boolean> {
		store.setState({ busy: true, error: null });
		try {
			const view = await api.setTauriSettings(change, label ?? settingsLabel(change));
			generation++;
			store.setState({ view, busy: false, error: null });
			return true;
		} catch (e) {
			store.setState({ busy: false, error: errorMessage(e) });
			return false;
		}
	}

	function clear(): void {
		generation++;
		store.setState({ ...initialTauriState });
	}

	return { store, load, importConfig, setSettings, clear };
}
