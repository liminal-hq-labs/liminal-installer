// Tests the tokens-to-CSS converter.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
// @ts-expect-error plain JavaScript module without types
import { flatten, toCss } from './tokens-to-css.mjs';
import tokens from '../src/styles/tokens.json';

describe('tokens to CSS', () => {
	it('flattens nested groups into prefixed names', () => {
		expect(flatten({ color: { bg: '#000', 'text-strong': '#fff' }, radius: { sm: '6px' } })).toEqual([
			['--lh-color-bg', '#000'],
			['--lh-color-text-strong', '#fff'],
			['--lh-radius-sm', '6px'],
		]);
	});

	it('makes dark the default and light an opt-in theme', () => {
		const css: string = toCss(tokens);
		expect(css).toMatch(/:root,\n:root\[data-theme="dark"\] \{/);
		expect(css).toContain('--lh-color-bg: #050507;');
		const light = css.slice(css.indexOf('[data-theme="light"] {'));
		expect(light).toContain('--lh-color-bg: #fbfaf6;');
		expect(light).not.toContain('--lh-font');
	});

	it('does not leak the themes group into the base variables', () => {
		const css: string = toCss(tokens);
		expect(css).not.toContain('--lh-themes');
	});
});
