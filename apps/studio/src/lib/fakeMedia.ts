// A test fake for the media part of the `Api`: the demo's media over an undo history of its own.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { demoMedia } from './demoMedia';
import { MEDIA_DOC } from './media';
import type { MediaApi } from './api';
import type { ChangeEvent, HistoryView, MediaInfo } from './types';

/** What the fake needs beside `MediaApi` for the app to undo, redo and hear of changes. */
export interface FakeMedia extends MediaApi {
	history(): Promise<HistoryView>;
	undo(): Promise<HistoryView>;
	redo(): Promise<HistoryView>;
	onChange(handler: (e: ChangeEvent) => void): Promise<() => void>;
	/** The calls made, as `name(args)` text, oldest first. */
	readonly calls: string[];
	/** The `setMedia` calls among them: the edits a person made. */
	edits(): string[];
	/** The file `pickReadme` answers with; null cancels. */
	picked: string | null;
}

/** A fake media backend that starts with `initial` (none by default) and keeps its own history. */
export function fakeMedia(initial: MediaInfo[] = []): FakeMedia {
	const core = demoMedia(initial);
	const listeners = new Set<(e: ChangeEvent) => void>();
	const calls: string[] = [];
	let entries: { label: string; before: string; after: string }[] = [];
	let cursor = 0;

	const view = (): HistoryView => ({
		entries: entries.map((e, i) => ({
			index: i,
			label: e.label,
			source: 'ui',
			doc: MEDIA_DOC,
			applied: i < cursor,
		})),
		can_undo: cursor > 0,
		can_redo: cursor < entries.length,
	});
	const emit = (kind: ChangeEvent['kind'], label: string) =>
		listeners.forEach((l) =>
			l({
				kind,
				doc: MEDIA_DOC,
				label,
				source: 'ui',
				can_undo: cursor > 0,
				can_redo: cursor < entries.length,
			}),
		);
	const record = <T>(label: string, change: () => T): T => {
		const before = core.snapshot();
		const result = change();
		entries = [...entries.slice(0, cursor), { label, before, after: core.snapshot() }];
		cursor = entries.length;
		emit('edit', label);
		return result;
	};
	const step = (dir: -1 | 1): HistoryView => {
		const entry = dir < 0 ? entries[cursor - 1] : entries[cursor];
		if (!entry) throw new Error(`nothing to ${dir < 0 ? 'undo' : 'redo'}`);
		core.restore(dir < 0 ? entry.before : entry.after);
		cursor += dir;
		emit(dir < 0 ? 'undo' : 'redo', entry.label);
		return view();
	};

	const fake: FakeMedia = {
		calls,
		edits: () => calls.filter((c) => c.startsWith('setMedia')),
		picked: '/project/installer/media/README.TXT',
		listMedia: async () => {
			calls.push('listMedia()');
			return core.list();
		},
		getMediaPlan: async (id, overrides) => {
			calls.push(`getMediaPlan(${id}${overrides ? `, ${JSON.stringify(overrides)}` : ''})`);
			return core.plan(id, overrides);
		},
		addMedia: async (kind, label) => {
			calls.push(`addMedia(${kind})`);
			return record(label, () => core.add(kind));
		},
		setMedia: async (id, edit, label) => {
			calls.push(`setMedia(${id}, ${JSON.stringify(edit)})`);
			return record(label, () => core.set(id, edit));
		},
		removeMedia: async (id, label) => {
			calls.push(`removeMedia(${id})`);
			record(label, () => core.remove(id));
		},
		pickReadme: async () => fake.picked,
		history: async () => view(),
		undo: async () => step(-1),
		redo: async () => step(1),
		onChange: async (handler) => {
			listeners.add(handler);
			return () => listeners.delete(handler);
		},
	};
	return fake;
}
