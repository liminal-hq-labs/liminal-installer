// Tests composing a theme's stage with a page: node references, step rules and the edits that give a page its own backdrop.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	attachStage,
	composeScreen,
	detachStage,
	isStageId,
	parseRef,
	refKey,
	screenRows,
	shownOn,
	stageIdFor,
} from './stage';
import type { Layout, LayoutNode } from './types';

const stage: LayoutNode = {
	kind: 'backdrop',
	props: { title: 'Fizz' },
	children: [
		{ kind: 'meter', props: { source: 'total', show_on: ['commit'] } },
		{ kind: 'meter', props: { source: 'free_space', hide_on: ['finish'] } },
		{ kind: 'slot' },
	],
};
const page: LayoutNode = {
	kind: 'dialog',
	props: { title: 'Welcome' },
	children: [
		{ kind: 'text', props: { text: 'Hi' } },
		{ kind: 'button', props: { label: 'Next' } },
	],
};
const step = { id: 'welcome', kind: 'page' };

describe('node references', () => {
	it('round-trips a scope and a path through the key a preview element carries', () => {
		expect(refKey({ scope: 'page', path: [1, 0] })).toBe('p:1.0');
		expect(refKey({ scope: 'stage', path: [] })).toBe('s:');
		expect(parseRef('p:1.0')).toEqual({ scope: 'page', path: [1, 0] });
		expect(parseRef('s:')).toEqual({ scope: 'stage', path: [] });
		expect(parseRef('x:1')).toBeNull();
		expect(parseRef(undefined)).toBeNull();
	});

	it('names the stage of a theme layout and nothing for a shared one', () => {
		expect(stageIdFor('fizz-win95/welcome')).toBe('fizz-win95/stage');
		expect(stageIdFor('welcome')).toBeNull();
		expect(isStageId('fizz-win95/stage')).toBe(true);
		expect(isStageId('fizz-win95/welcome')).toBe(false);
	});
});

describe('composeScreen', () => {
	it('puts the page in the slot and maps every node back to its own layout', () => {
		const { root, refs } = composeScreen(stage, page, step);
		expect(root.kind).toBe('backdrop');
		const kinds = root.children!.map((c) => c.kind);
		expect(kinds).toEqual(['meter', 'dialog']);
		expect(refs.get(root)).toEqual({ scope: 'stage', path: [] });
		// The free-space meter was the stage's second child, whatever is drawn before it.
		expect(refs.get(root.children![0]!)).toEqual({ scope: 'stage', path: [1] });
		const dialog = root.children![1]!;
		expect(refs.get(dialog)).toEqual({ scope: 'page', path: [] });
		expect(refs.get(dialog.children![1]!)).toEqual({ scope: 'page', path: [1] });
	});

	it('shows a stage node only where its show_on and hide_on lists allow', () => {
		const total = stage.children![0]!;
		expect(shownOn(total, { id: 'copying', kind: 'commit' })).toBe(true);
		expect(shownOn(total, step)).toBe(false);
		const free = stage.children![1]!;
		expect(shownOn(free, { id: 'done', kind: 'finish' })).toBe(false);
		expect(shownOn(free, { id: 'done', kind: 'finish' })).toBe(false);
		expect(shownOn(free, step)).toBe(true);
		const onCommit = composeScreen(stage, page, { id: 'copying', kind: 'commit' }).root;
		expect(onCommit.children!.map((c) => c.kind)).toEqual(['meter', 'meter', 'dialog']);
		const onDone = composeScreen(stage, page, { id: 'done', kind: 'finish' }).root;
		expect(onDone.children!.map((c) => c.kind)).toEqual(['dialog']);
	});

	it('draws a page alone when there is no stage or the page has its own backdrop', () => {
		expect(composeScreen(null, page, step).root.kind).toBe('dialog');
		const own: LayoutNode = { kind: 'backdrop', children: [page] };
		const { root, refs } = composeScreen(stage, own, step);
		expect(root.children![0]!.kind).toBe('dialog');
		expect(refs.get(root.children![0]!)).toEqual({ scope: 'page', path: [0] });
	});

	it('stands a placeholder dialog in the slot when the stage is shown on its own', () => {
		const { root, refs } = composeScreen(stage, null, step);
		const placeholder = root.children!.find((c) => c.kind === 'dialog')!;
		expect(refs.get(placeholder)).toEqual({ scope: 'stage', path: [2] });
	});
});

describe('screenRows', () => {
	it('lists the stage and page nodes in document order with their own paths and depths', () => {
		const rows = screenRows(stage, page, step);
		expect(rows.map((r) => `${refKey(r.ref)}:${r.depth}:${r.node.kind}`)).toEqual([
			's::0:backdrop',
			's:1:1:meter',
			'p::1:dialog',
			'p:0:2:text',
			'p:1:2:button',
		]);
	});
});

describe('giving a page its own copy of the stage', () => {
	const layout = (root: LayoutNode): Layout => ({ version: 1, id: 'fizz/welcome', root });
	it('detaches the stage into the page and attaches it back', () => {
		const own = detachStage(layout(stage), layout(page), step);
		expect(own.root.kind).toBe('backdrop');
		expect(own.root.children!.map((c) => c.kind)).toEqual(['meter', 'dialog']);
		expect(own.root.props?.title).toBe('Fizz');
		const back = attachStage(own);
		expect(back.root.kind).toBe('dialog');
		expect(back.root.props?.title).toBe('Welcome');
		expect(attachStage(layout(page))).toEqual(layout(page));
	});
});
