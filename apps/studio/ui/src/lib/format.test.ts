// Tests the formatting helpers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { formatOp, historyRows, problemsFor, screenshotSrc, targetStatus } from './format';
import type { Problem } from './types';

describe('format', () => {
	it('builds image sources for both encodings', () => {
		expect(screenshotSrc({ mime: 'image/png', encoding: 'base64', data: 'AAAA' })).toBe('data:image/png;base64,AAAA');
		expect(screenshotSrc({ mime: 'image/svg+xml', encoding: 'utf8', data: '<svg a="1"/>' })).toContain('%3Csvg');
	});

	it('describes plan operations', () => {
		expect(formatOp({ op: 'copy', from: 'a', to: 'b' })).toEqual({ name: 'copy', detail: 'a → b' });
		expect(formatOp({ op: 'shortcut', name: 'Fizz', target: 'x', place: 'start_menu' }).detail).toBe('Fizz → x (start_menu)');
		expect(formatOp({ op: 'receipt.write', path: 'r' })).toEqual({ name: 'receipt.write', detail: 'r' });
		expect(formatOp({ op: 'registry.set', key: 'K', name: 'N' }).detail).toBe('key=K name=N');
	});

	it('filters problems by scope prefix', () => {
		const ps: Problem[] = [
			{ severity: 'error', scope: 'flow:install', message: 'a' },
			{ severity: 'warning', scope: 'target:dos-x86:tui-retro', message: 'b' },
		];
		expect(problemsFor(ps, 'flow:install')).toHaveLength(1);
		expect(targetStatus(ps, 'dos-x86:tui-retro')).toBe('warning');
		expect(targetStatus(ps, 'other')).toBe('ok');
	});

	it('lists history newest first and marks undone entries', () => {
		const rows = historyRows([
			{ index: 0, label: 'one', source: 'ui', doc: { type: 'flow', id: 'install' }, applied: true },
			{ index: 1, label: 'two', source: 'mcp', doc: { type: 'script', id: 's' }, applied: false },
		]);
		expect(rows.map((r) => [r.label, r.source, r.undone])).toEqual([
			['two', 'Agent', true],
			['one', 'You', false],
		]);
	});
});
