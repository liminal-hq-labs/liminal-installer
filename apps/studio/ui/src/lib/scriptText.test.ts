// Tests the script editor's text helpers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { formatScript, lineCount } from './scriptText';

describe('formatScript', () => {
	it('strips trailing spaces, uses line feeds and ends with one newline', () => {
		expect(formatScript('a = 1  \r\nb = 2\t\r\n\r\n\r\n')).toBe('a = 1\nb = 2\n');
		expect(formatScript('x')).toBe('x\n');
	});

	it('keeps indentation and inner blank lines, and an empty script empty', () => {
		expect(formatScript('if a then\n\n\tb()\nend')).toBe('if a then\n\n\tb()\nend\n');
		expect(formatScript('  \n\n')).toBe('');
		expect(formatScript('')).toBe('');
	});

	it('is stable', () => {
		const once = formatScript('a  \n\n\nb  ');
		expect(formatScript(once)).toBe(once);
	});
});

describe('lineCount', () => {
	it('counts lines, not the newline that ends the last', () => {
		expect(lineCount('')).toBe(1);
		expect(lineCount('a')).toBe(1);
		expect(lineCount('a\n')).toBe(1);
		expect(lineCount('a\nb')).toBe(2);
		expect(lineCount('a\n\n')).toBe(2);
	});
});
