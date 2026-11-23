// Tests the pure layout tree edits.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	addChild,
	describeNode,
	flattenTree,
	getNode,
	moveNode,
	parsePropValue,
	propToText,
	removeNode,
	resolveForRenderer,
	setProp,
	toggleOverride,
} from './layoutEdit';
import type { Layout } from './types';

const layout = (): Layout => ({
	version: 1,
	id: 'welcome',
	root: {
		kind: 'window',
		props: { title: 'Welcome' },
		children: [
			{ kind: 'text', props: { text: 'Hello' } },
			{ kind: 'button', props: { label: 'Next' } },
		],
	},
});

describe('tree edits', () => {
	it('finds nodes by path', () => {
		expect(getNode(layout().root, [1])!.kind).toBe('button');
		expect(getNode(layout().root, [5])).toBeUndefined();
	});

	it('sets and clears props without mutating the input', () => {
		const l = layout();
		const out = setProp(l, [0], 'text', 'Bye');
		expect(getNode(out.root, [0])!.props!.text).toBe('Bye');
		expect(getNode(l.root, [0])!.props!.text).toBe('Hello');
		expect(getNode(setProp(out, [0], 'text', '').root, [0])!.props).toBeUndefined();
	});

	it('adds children and reports their path', () => {
		const { layout: out, path } = addChild(layout(), [], 'meter');
		expect(path).toEqual([2]);
		expect(getNode(out.root, path)!.props).toMatchObject({ source: 'total' });
	});

	it('removes nodes but never the root', () => {
		expect(getNode(removeNode(layout(), [0]).root, [0])!.kind).toBe('button');
		expect(removeNode(layout(), [])).toEqual(layout());
	});

	it('moves nodes among siblings and stops at the ends', () => {
		const down = moveNode(layout(), [0], 1);
		expect(down.path).toEqual([1]);
		expect(getNode(down.layout.root, [0])!.kind).toBe('button');
		expect(moveNode(layout(), [0], -1).path).toEqual([0]);
	});

	it('describes nodes and flattens the tree in order', () => {
		expect(describeNode(layout().root)).toBe('window · Welcome');
		expect(flattenTree(layout().root).map((n) => [n.depth, n.node.kind])).toEqual([
			[0, 'window'],
			[1, 'text'],
			[1, 'button'],
		]);
	});
});

describe('renderer overrides', () => {
	it('copies the node when an override is switched on and removes it when off', () => {
		const on = toggleOverride(layout(), [0], 'tui-retro', true);
		expect(getNode(on.root, [0])!.overrides!['tui-retro']!.props).toEqual({ text: 'Hello' });
		const off = toggleOverride(on, [0], 'tui-retro', false);
		expect(getNode(off.root, [0])!.overrides).toBeUndefined();
	});

	it('edits the override, not the base, when a renderer is given', () => {
		const on = toggleOverride(layout(), [0], 'tui-retro', true);
		const edited = setProp(on, [0], 'text', 'HELLO', 'tui-retro');
		expect(getNode(edited.root, [0])!.props!.text).toBe('Hello');
		expect(getNode(edited.root, [0])!.overrides!['tui-retro']!.props!.text).toBe('HELLO');
	});

	it('resolves overrides through the whole tree', () => {
		const l = setProp(toggleOverride(layout(), [0], 'tui-retro', true), [0], 'text', 'HELLO', 'tui-retro');
		const retro = resolveForRenderer(l.root, 'tui-retro');
		expect(retro.children![0]!.props!.text).toBe('HELLO');
		expect(retro.children![0]!.overrides).toBeUndefined();
		expect(resolveForRenderer(l.root, 'canvas').children![0]!.props!.text).toBe('Hello');
	});
});

describe('prop text', () => {
	it('round-trips strings, booleans, numbers and structures', () => {
		expect(propToText('a')).toBe('a');
		expect(propToText({ id: 1 })).toBe('{"id":1}');
		expect(parsePropValue('hello', 'old')).toBe('hello');
		expect(parsePropValue('true', undefined)).toBe(true);
		expect(parsePropValue('12', undefined)).toBe(12);
		expect(parsePropValue('plain text', undefined)).toBe('plain text');
		expect(parsePropValue('[1, 2]', [0])).toEqual([1, 2]);
		expect(parsePropValue('{oops', {})).toBe('{oops');
	});
});
