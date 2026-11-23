// Tests keyboard navigation: which button Enter, Esc and Alt+Left press, and the paths that name the switched-off ones.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { licence, welcome } from './fixtures';
import { backAction, enterAction, escapeAction, pathsOf } from './keys';

describe('keyboard navigation', () => {
	it('presses the default button on Enter and Cancel on Esc', () => {
		expect(enterAction(welcome, new Set())).toBe('next');
		expect(escapeAction(welcome, new Set())).toBe('cancel');
		expect(backAction(welcome, new Set())).toBeNull();
	});

	it('finds Back, and skips a button the page switched off', () => {
		expect(backAction(licence, new Set())).toBe('back');
		// The footer is child 1 of the window; Next is its second button.
		const next = [...pathsOf(licence)].find(([n]) => n.props?.label === 'Next')?.[1];
		expect(next).toBe('1.1');
		expect(enterAction(licence, new Set([next!]))).toBeNull();
	});

	it('numbers nodes as the shell does', () => {
		const paths = [...pathsOf(welcome).values()];
		expect(paths).toEqual(['', '0', '0.0', '1', '1.0', '1.1']);
	});
});
