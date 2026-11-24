// The browser demo's media: the project's `[[media]]` entries, kept in memory and planned from the fixture.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { StudioError } from './api';
import { defaultLabel, nextMediaId } from './media';
import {
	FIXTURE_PRODUCT,
	newMedia,
	planFromFixture,
	quillCd,
	quillFloppy,
	quillSet,
	validateInfo,
} from './mediaFixture';
import type { MediaEdit, MediaInfo, MediaKind, MediaPlan } from './types';

/** Applies a `MediaEdit` to an entry the way `li-media`'s `MediaEdit::apply` does: a field left out is not touched. */
export function applyEdit(info: MediaInfo, edit: MediaEdit): MediaInfo {
	const { readme, autorun, ...rest } = info;
	const next: MediaInfo = { ...rest, ...definedOnly(edit) } as MediaInfo;
	const nextReadme = edit.readme === undefined ? readme : edit.readme;
	const nextAutorun = edit.autorun === undefined ? autorun : edit.autorun;
	if (nextReadme) next.readme = nextReadme;
	else delete next.readme;
	if (nextAutorun) next.autorun = nextAutorun;
	else delete next.autorun;
	return next;
}

const definedOnly = (edit: MediaEdit): Partial<MediaInfo> => {
	const { readme: _readme, autorun: _autorun, ...rest } = edit;
	return Object.fromEntries(Object.entries(rest).filter(([, v]) => v !== undefined));
};

export interface DemoMedia {
	list(): MediaInfo[];
	/** The plan of one medium, with `overrides` applied when given and nothing kept. */
	plan(id: string, overrides?: MediaEdit): MediaPlan;
	add(kind: MediaKind): MediaInfo;
	set(id: string, edit: MediaEdit): MediaInfo;
	remove(id: string): void;
	/** The entries as text, for an undo history, and the way back. */
	snapshot(): string;
	restore(text: string | undefined): void;
}

/** The demo's media: a floppy, a CD and a two-disk floppy set, all of the Quill payload. */
export function demoMedia(initial?: MediaInfo[]): DemoMedia {
	let items: MediaInfo[] = initial
		? structuredClone(initial)
		: [quillFloppy(), quillCd(), quillSet()];
	const find = (id: string): MediaInfo => {
		const found = items.find((m) => m.id === id);
		if (!found) throw new StudioError('not_found', `no medium called ${id}`);
		return found;
	};
	return {
		list: () => structuredClone(items),
		plan: (id, overrides) => planFromFixture(overrides ? applyEdit(find(id), overrides) : find(id)),
		add(kind) {
			const entry = newMedia(
				kind,
				nextMediaId(kind, items),
				defaultLabel(FIXTURE_PRODUCT.slug, kind),
			);
			items = [...items, entry];
			return structuredClone(entry);
		},
		set(id, edit) {
			const before = find(id);
			const after = applyEdit(before, edit);
			// An edit may not add a label, serial or folder error; one the file already had stays viewable.
			const had = new Set(validateInfo(before).map((w) => `${w.code}:${w.subject}`));
			const worse = validateInfo(after).find(
				(w) => w.severity === 'error' && !had.has(`${w.code}:${w.subject}`),
			);
			if (worse) throw new StudioError('invalid_input', worse.message);
			items = items.map((m) => (m.id === id ? after : m));
			return structuredClone(after);
		},
		remove(id) {
			find(id);
			items = items.filter((m) => m.id !== id);
		},
		snapshot: () => JSON.stringify(items),
		restore(text) {
			items = text === undefined ? [] : (JSON.parse(text) as MediaInfo[]);
		},
	};
}
