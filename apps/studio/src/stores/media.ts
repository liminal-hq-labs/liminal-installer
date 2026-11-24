// The media store: the project's media and their plans, the one picked, and the actions that change them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import { t } from '../i18n/messages';
import { errorMessage, type Api } from '../lib/api';
import type { MediaEdit, MediaInfo, MediaKind, MediaPlan } from '../lib/types';

/** The tabs of a medium's detail pane. */
export type MediaTab = 'map' | 'sizes' | 'targets' | 'label' | 'autorun';

export interface MediaState {
	/** The project's media, in the order of `installer.toml`. */
	items: MediaInfo[];
	/** The plan of each medium, by id. */
	plans: Record<string, MediaPlan>;
	/** The id of the medium shown, or null with none. */
	selected: string | null;
	tab: MediaTab;
	/** The media have been read at least once for this project. */
	loaded: boolean;
	loading: boolean;
	/** An add, an edit or a removal is in flight. */
	busy: boolean;
	/** Why the last load or change failed. */
	error: string | null;
}

export const initialMediaState: MediaState = {
	items: [],
	plans: {},
	selected: null,
	tab: 'map',
	loaded: false,
	loading: false,
	busy: false,
	error: null,
};

export type MediaStore = StoreApi<MediaState>;

export interface Media {
	readonly store: MediaStore;
	/** Reads the media and their plans. A newer read wins over an older one still in flight. */
	load(): Promise<void>;
	/** Reads them again when they were read before, as a change from elsewhere (an undo) needs. */
	refresh(): Promise<void>;
	select(id: string): void;
	setTab(tab: MediaTab): void;
	/** Adds a medium of `kind` and picks it. True when it worked. */
	add(kind: MediaKind): Promise<boolean>;
	/** Removes a medium; the caller has asked the person. True when it worked. */
	remove(id: string): Promise<boolean>;
	/** Changes one medium; `label` names the history entry. True when it worked. */
	edit(id: string, edit: MediaEdit, label: string): Promise<boolean>;
	/** Forgets everything, as closing the project or opening another does. */
	clear(): void;
}

export function createMedia(api: Api): Media {
	const store = createStore<MediaState>(() => ({ ...initialMediaState }));
	let generation = 0;

	const plansFor = async (items: MediaInfo[]): Promise<Record<string, MediaPlan>> =>
		Object.fromEntries(
			await Promise.all(items.map(async (m) => [m.id, await api.getMediaPlan(m.id)] as const)),
		);

	/** The id to show after a change: the one that was, else the first. */
	const keep = (items: MediaInfo[], wanted: string | null): string | null =>
		items.some((m) => m.id === wanted) ? wanted : (items[0]?.id ?? null);

	async function load(): Promise<void> {
		const mine = ++generation;
		store.setState({ loading: true });
		try {
			const items = await api.listMedia();
			const plans = await plansFor(items);
			if (mine !== generation) return;
			store.setState((s) => ({
				items,
				plans,
				selected: keep(items, s.selected),
				loaded: true,
				loading: false,
				error: null,
			}));
		} catch (e) {
			if (mine === generation) store.setState({ loading: false, error: errorMessage(e) });
		}
	}

	async function change<T>(action: () => Promise<T>, then: (result: T) => Partial<MediaState>) {
		store.setState({ busy: true, error: null });
		try {
			const result = await action();
			const items = await api.listMedia();
			const plans = await plansFor(items);
			generation++;
			store.setState((s) => {
				const next = then(result);
				return {
					items,
					plans,
					selected: keep(items, next.selected === undefined ? s.selected : next.selected),
					...next,
					loaded: true,
					busy: false,
					error: null,
				};
			});
			return true;
		} catch (e) {
			store.setState({ busy: false, error: errorMessage(e) });
			return false;
		}
	}

	return {
		store,
		load,
		refresh: async () => {
			if (store.getState().loaded) await load();
		},
		select: (id) => store.setState({ selected: id }),
		setTab: (tab) => store.setState({ tab }),
		add: (kind) =>
			change(
				() => api.addMedia(kind, t('history.mediaAdd', { kind: t(kindMessage(kind)) })),
				(added) => ({ selected: added.id }),
			),
		remove: (id) =>
			change(
				() => api.removeMedia(id, t('history.mediaRemove', { id })),
				() => ({}),
			),
		edit: (id, edit, label) =>
			change(
				() => api.setMedia(id, edit, label),
				() => ({}),
			),
		clear() {
			generation++;
			store.setState({ ...initialMediaState });
		},
	};
}

/** The message that names a kind of medium. */
export const kindMessage = (kind: MediaKind) =>
	kind === 'cdrom' ? ('media.kind.cdrom' as const) : ('media.kind.floppy' as const);
