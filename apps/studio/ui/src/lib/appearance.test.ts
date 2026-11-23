// Tests how the appearance settings become what the page shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { applyAppearance, browserSystem, resolveMotion, resolveTheme } from './appearance';
import { defaultSettings } from './settings';

describe('appearance', () => {
	it('resolves the system theme to Afterglow dark or light and leaves a chosen one alone', () => {
		expect(resolveTheme('system', { dark: true, reducedMotion: false })).toBe('afterglow');
		expect(resolveTheme('system', { dark: false, reducedMotion: false })).toBe('afterglow-light');
		expect(resolveTheme('afterglow-light', { dark: true, reducedMotion: false })).toBe('afterglow-light');
		expect(resolveTheme('afterglow-dark', { dark: false, reducedMotion: false })).toBe('afterglow-dark');
	});

	it('follows the system for motion only when asked', () => {
		const reduced = { dark: true, reducedMotion: true };
		expect(resolveMotion('system', reduced)).toBe(true);
		expect(resolveMotion('system', { ...reduced, reducedMotion: false })).toBe(false);
		expect(resolveMotion('on', { ...reduced, reducedMotion: false })).toBe(true);
		expect(resolveMotion('off', reduced)).toBe(false);
	});

	it('sets the theme, density, motion and text scale on the root', () => {
		const style = new Map<string, string>();
		const root = { dataset: {} as DOMStringMap, style: { setProperty: (k: string, v: string) => void style.set(k, v) } };
		applyAppearance(root, { ...defaultSettings, theme: 'system', density: 'compact', textSize: 120, reduceMotion: 'on' }, { dark: false, reducedMotion: false });
		expect(root.dataset).toEqual({ theme: 'afterglow-light', density: 'compact', motion: 'reduced' });
		expect(style.get('--text-scale')).toBe('1.2');
	});

	it('reads the browser preferences', () => {
		expect(browserSystem((q) => ({ matches: q.includes('light') }))).toEqual({ dark: false, reducedMotion: false });
		expect(browserSystem((q) => ({ matches: q.includes('reduce') }))).toEqual({ dark: true, reducedMotion: true });
	});
});
