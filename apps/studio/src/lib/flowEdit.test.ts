// Tests the pure flow graph edits and layout.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	addStep,
	connect,
	disconnect,
	layoutGraph,
	removeStep,
	renameStep,
	updateStep,
	validateFlow,
} from './flowEdit';
import type { Flow } from './types';

const base = (): Flow => ({
	version: 1,
	id: 'install',
	steps: [
		{ id: 'welcome', type: 'page', layout: 'welcome', next: 'copy' },
		{ id: 'copy', type: 'commit', layout: 'copying', next: 'done' },
		{ id: 'done', type: 'finish' },
	],
});

describe('validateFlow', () => {
	it('accepts a valid flow', () => {
		expect(validateFlow(base())).toEqual([]);
	});

	it('reports the same problems as Flow::validate', () => {
		const f = base();
		f.steps.push({ id: 'welcome', type: 'page', next: 'nowhere' });
		f.steps.push({ id: 'second', type: 'commit', next: 'second' });
		const messages = validateFlow(f).map((p) => p.message);
		expect(messages).toContain('duplicate step id `welcome`');
		expect(messages).toContain('a flow needs exactly one commit step, found 2');
		expect(messages).toContain('step `welcome` points at unknown step `nowhere`');
		expect(messages).toContain('step `second` cannot reach a finish step');
	});

	it('flags dead ends and an empty flow', () => {
		const f = base();
		delete f.steps[0]!.next;
		expect(validateFlow(f).map((p) => p.message)).toContain(
			'step `welcome` has no next step and is not a finish step',
		);
		expect(validateFlow({ ...f, steps: [] })[0]!.message).toBe('flow has no steps');
	});
});

describe('edits', () => {
	it('splices a new step after the selected one', () => {
		const { flow, id } = addStep(base(), 'page', 'welcome');
		expect(id).toBe('page');
		expect(flow.steps.map((s) => s.id)).toEqual(['welcome', 'page', 'copy', 'done']);
		expect(flow.steps[0]!.next).toBe('page');
		expect(flow.steps[1]!.next).toBe('copy');
	});

	it('gives new steps unique ids and does not mutate the input', () => {
		const f = base();
		const a = addStep(f, 'page', 'welcome');
		const b = addStep(a.flow, 'page', 'welcome');
		expect(b.id).toBe('page-2');
		expect(f.steps).toHaveLength(3);
	});

	it('removes a step and bridges its neighbours', () => {
		const f = removeStep(base(), 'copy');
		expect(f.steps.map((s) => s.id)).toEqual(['welcome', 'done']);
		expect(f.steps[0]!.next).toBe('done');
	});

	it('bridges branches and moves the start when removing', () => {
		const f: Flow = {
			version: 1,
			id: 'x',
			start: 'a',
			steps: [
				{ id: 'a', type: 'branch', branches: [{ when: 'c', next: 'b' }], next: 'b' },
				{ id: 'b', type: 'page', next: 'z' },
				{ id: 'z', type: 'finish' },
			],
		};
		const out = removeStep(removeStep(f, 'b'), 'a');
		expect(out.start).toBe('z');
		const mid = removeStep(f, 'b');
		expect(mid.steps[0]!.branches).toEqual([{ when: 'c', next: 'z' }]);
		expect(mid.steps[0]!.next).toBe('z');
	});

	it('connects and disconnects next edges and branches', () => {
		let f = connect(base(), 'welcome', 'done');
		expect(f.steps[0]!.next).toBe('done');
		f = connect(f, 'welcome', 'copy', -1);
		expect(f.steps[0]!.branches).toEqual([{ when: 'true', next: 'copy' }]);
		f = connect(f, 'welcome', 'done', 0);
		expect(f.steps[0]!.branches![0]!.next).toBe('done');
		f = disconnect(disconnect(f, 'welcome', 0), 'welcome');
		expect(f.steps[0]!.branches).toBeUndefined();
		expect(f.steps[0]!.next).toBeUndefined();
	});

	it('renames a step everywhere and refuses clashes', () => {
		const f = renameStep(base(), 'copy', 'install');
		expect(f.steps[0]!.next).toBe('install');
		expect(f.steps[1]!.id).toBe('install');
		expect(renameStep(base(), 'copy', 'done')).toEqual(base());
	});

	it('updates fields and drops emptied ones', () => {
		const f = updateStep(base(), 'welcome', { when: "x == 'y'", layout: '' });
		expect(f.steps[0]!.when).toBe("x == 'y'");
		expect(f.steps[0]!.layout).toBeUndefined();
	});
});

describe('layoutGraph', () => {
	it('puts steps in columns by distance from the start', () => {
		const g = layoutGraph(base());
		expect(g.nodes.map((n) => [n.id, n.column])).toEqual([
			['welcome', 0],
			['copy', 1],
			['done', 2],
		]);
		expect(g.edges).toHaveLength(2);
		expect(g.width).toBeGreaterThan(g.nodes[2]!.x);
	});

	it('stacks siblings in rows, labels branches and marks back edges', () => {
		const f: Flow = {
			version: 1,
			id: 'x',
			steps: [
				{ id: 'a', type: 'branch', branches: [{ when: 'cond', next: 'b' }], next: 'c' },
				{ id: 'b', type: 'page', next: 'a' },
				{ id: 'c', type: 'finish' },
			],
		};
		const g = layoutGraph(f);
		expect(g.nodes.find((n) => n.id === 'c')!.row).toBe(1);
		expect(g.edges.find((e) => e.label === 'cond')).toBeDefined();
		expect(g.edges.find((e) => e.from === 'b')!.back).toBe(true);
	});

	it('places unreachable steps in their own column', () => {
		const f = base();
		f.steps.push({ id: 'orphan', type: 'page', next: 'done' });
		const g = layoutGraph(f);
		expect(g.nodes.find((n) => n.id === 'orphan')!.column).toBe(3);
	});
});
