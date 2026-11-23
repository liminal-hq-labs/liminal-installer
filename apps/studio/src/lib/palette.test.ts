// Tests the add-node palette: every node kind is offered, grouped and described, and the meter presets are right.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { NODE_KINDS, insertNodes } from './layoutEdit';
import { GROUPS, PALETTE, meterNode, paletteItem } from './palette';
import type { Layout } from './types';

describe('the palette', () => {
	it('offers every node kind of the spec', () => {
		const offered = new Set(
			PALETTE.flatMap((p) =>
				p.build().flatMap(function kinds(n): string[] {
					return [n.kind, ...(n.children ?? []).flatMap(kinds)];
				}),
			),
		);
		for (const kind of NODE_KINDS) expect(offered, kind).toContain(kind);
		const spec = readFileSync(new URL('../../../../SPEC.md', import.meta.url), 'utf8');
		for (const kind of NODE_KINDS.filter((k) => k !== 'slot'))
			expect(spec).toContain(`\`${kind}\``);
	});

	it('groups every item and gives each a label and a hint', () => {
		const groups = new Set(GROUPS.map((g) => g.id));
		expect(new Set(PALETTE.map((p) => p.id)).size).toBe(PALETTE.length);
		for (const p of PALETTE) {
			expect(groups.has(p.group), p.id).toBe(true);
			expect(p.label().length, p.id).toBeGreaterThan(0);
			expect(p.hint().length, p.id).toBeGreaterThan(0);
			expect(
				p.build().map((n) => n.kind),
				p.id,
			).toEqual(expect.arrayContaining([p.kinds[0]!]));
		}
	});

	it('names the presets the spec describes: button row, side art, billboard, checklist, radio, field, path, progress', () => {
		for (const id of [
			'button-row',
			'side-art',
			'billboard',
			'checklist',
			'radio',
			'field',
			'path',
			'steps',
		])
			expect(paletteItem(id), id).toBeDefined();
		const row = paletteItem('button-row')!.build()[0]!;
		expect(row.props?.region).toBe('footer');
		expect(row.children?.map((b) => b.props?.action)).toEqual(['back', 'next', 'cancel']);
		expect(paletteItem('side-art')!.build()[0]!.props?.region).toBe('side');
	});

	it('builds fresh nodes each time', () => {
		const a = paletteItem('side-art')!.build()[0]!;
		const b = paletteItem('side-art')!.build()[0]!;
		expect(a).not.toBe(b);
		expect(a.children![0]!.props!.art).not.toBe(b.children![0]!.props!.art);
	});
});

describe('the meters', () => {
	it('has a meter node for each source, with a warning level on free space', () => {
		const free = meterNode('free_space');
		expect(free).toMatchObject({
			kind: 'meter',
			bind: 'engine.meter.free_space',
			props: { source: 'free_space', low_below: 0.05, place: 'backdrop', region: 'status' },
		});
		expect(meterNode('file').props).not.toHaveProperty('low_below');
		expect(meterNode('total', 'dialog').props).not.toHaveProperty('region');
	});

	it('adds the three classic gauges together as the Status meters preset, hidden on the last page', () => {
		const nodes = paletteItem('status-meters')!.build();
		expect(nodes.map((n) => n.props?.source)).toEqual(['file', 'total', 'free_space']);
		for (const n of nodes) expect(n.props?.hide_on).toEqual(['finish']);
		expect(paletteItem('status-meters')!.scope).toBe('stage');
	});
});

describe('insertNodes', () => {
	const layout: Layout = {
		version: 1,
		id: 'x',
		root: {
			kind: 'dialog',
			children: [{ kind: 'text' }, { kind: 'stack', children: [{ kind: 'button' }] }],
		},
	};
	it('goes inside a container and after anything else', () => {
		const inside = insertNodes(layout, [1], [{ kind: 'text' }]);
		expect(inside.path).toEqual([1, 1]);
		expect(inside.layout.root.children![1]!.children).toHaveLength(2);
		const after = insertNodes(layout, [0], [{ kind: 'meter' }, { kind: 'meter' }]);
		expect(after.path).toEqual([1]);
		expect(after.layout.root.children!.map((c) => c.kind)).toEqual([
			'text',
			'meter',
			'meter',
			'stack',
		]);
		expect(layout.root.children).toHaveLength(2);
	});
});
