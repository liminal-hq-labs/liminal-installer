// Tests the tokens-to-CSS converter against the design system export.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
// @ts-expect-error plain JavaScript module without types
import { resolveRefs, toCss, valueIn } from './tokens-to-css.mjs';
import tokens from '../src/theme/tokens.json';

describe('tokens to CSS', () => {
	it('turns references into variable lookups so themes carry through', () => {
		expect(resolveRefs('{brand-purple}')).toBe('var(--lis-brand-purple)');
		expect(resolveRefs('1px solid {border-subtle}')).toBe('1px solid var(--lis-border-subtle)');
	});

	it('reads a token per theme, and only overrides where a theme says so', () => {
		const orange = { name: 'x', value: { afterglow: '#ffaa40', 'afterglow-light': '#b45309' } };
		expect(valueIn(orange, 'afterglow')).toBe('#ffaa40');
		expect(valueIn(orange, 'afterglow-light')).toBe('#b45309');
		expect(valueIn(orange, 'afterglow-dark')).toBeUndefined();
		expect(valueIn({ name: 'y', value: '#fff' }, 'afterglow')).toBe('#fff');
		expect(valueIn({ name: 'y', value: '#fff' }, 'afterglow-light')).toBeUndefined();
	});

	it('makes the base theme the default and the others opt-in', () => {
		const css: string = toCss(tokens);
		expect(css).toMatch(/:root,\n:root\[data-theme="afterglow"\] \{/);
		expect(css).toContain('--lis-bg-app: #050507;');
		expect(css).toContain('--lis-brand-orange: #ffaa40;');
		const light = css.slice(css.indexOf('[data-theme="afterglow-light"] {'));
		expect(light).toContain('color-scheme: light;');
		expect(light).toContain('--lis-bg-app: #fbfaf6;');
		expect(light).toContain('--lis-brand-orange: #b45309;');
		expect(light).not.toContain('--lis-font-body');
	});

	it('emits the layout, type and spacing groups', () => {
		const css: string = toCss(tokens);
		expect(css).toContain('--lis-topbar-height: 40px;');
		expect(css).toContain('--lis-statusbar-height: 28px;');
		expect(css).toContain('--lis-sidebar-width: 220px;');
		expect(css).toContain('--lis-space-2: 0.5rem;');
		expect(css).toMatch(/--lis-font-heading: 'Space Grotesk'/);
	});

	it('leaves other products’ palettes out', () => {
		const css: string = toCss(tokens);
		expect(css).not.toContain('--lis-wp-');
		expect(css).not.toContain('--lis-cadence-');
	});

	it('resolves accent through the brand colour', () => {
		expect(toCss(tokens)).toContain('--lis-accent: var(--lis-brand-orange);');
	});
});
