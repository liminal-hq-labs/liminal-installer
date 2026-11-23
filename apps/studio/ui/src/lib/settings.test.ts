// Tests the settings schema: validation, merge, migration of the old file and the parity with the Rust schema.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
	DENSITIES,
	LIMITS,
	MOTIONS,
	SETTINGS_VERSION,
	THEMES,
	browserSettings,
	defaultSettings,
	mergeSettings,
	normalizeSettings,
	pushRecent,
	sameSettings,
	unknownKeys,
	validatePatch,
	type KeyValueStore,
} from './settings';

const memory = (initial?: string): KeyValueStore & { data: Map<string, string> } => {
	const data = new Map<string, string>();
	if (initial) data.set('liminal-installer-studio.settings', initial);
	return { data, getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
};

describe('validation', () => {
	it('accepts every default and the bounds of every range', () => {
		expect(validatePatch({ version: SETTINGS_VERSION })).toBe(
			'the version is managed by the Studio',
		);
		const { version: _version, ...rest } = defaultSettings;
		expect(validatePatch(rest)).toBeNull();
		for (const [key, [lo, hi]] of Object.entries(LIMITS)) {
			expect(validatePatch({ [key]: lo }), `${key} low`).toBeNull();
			expect(validatePatch({ [key]: hi }), `${key} high`).toBeNull();
			expect(validatePatch({ [key]: lo - 1 }), `${key} below`).not.toBeNull();
			expect(validatePatch({ [key]: hi + 1 }), `${key} above`).not.toBeNull();
		}
	});

	it('refuses the wrong type, an unknown choice, an unknown key and a bad shape', () => {
		for (const patch of [
			{ menuBar: 'yes' },
			{ theme: 'neon' },
			{ density: 'tiny' },
			{ reduceMotion: 'maybe' },
			{ tabWidth: 2.5 },
			{ recentProjects: [''] },
			{ recentProjects: 'a' },
			{ runnerPath: 'a\nb' },
			{ nope: 1 },
			{ version: 2 },
			{ menuBar: true, nope: 1 },
		]) {
			expect(validatePatch(patch), JSON.stringify(patch)).not.toBeNull();
		}
		expect(validatePatch([1])).toBe('settings must be an object');
		expect(validatePatch(null)).toBe('settings must be an object');
	});

	it('says what is wrong in words', () => {
		expect(validatePatch({ textSize: 140 })).toBe('text size must be between 90 and 130');
		expect(validatePatch({ nope: 1 })).toBe('unknown setting `nope`');
	});
});

describe('merge', () => {
	it('changes only the named fields', () => {
		const merged = mergeSettings(defaultSettings, { tabWidth: 2, menuBar: true });
		expect(merged).toMatchObject({
			ok: true,
			settings: { tabWidth: 2, menuBar: true, theme: 'afterglow' },
		});
	});

	it('refuses a bad patch whole', () => {
		expect(mergeSettings(defaultSettings, { menuBar: true, tabWidth: 99 })).toMatchObject({
			ok: false,
		});
	});

	it('keeps the recent list within its limit and without repeats', () => {
		const many = Array.from({ length: 30 }, (_, i) => `/p/${i}`);
		const merged = mergeSettings(defaultSettings, { recentProjects: many });
		expect(merged.ok && merged.settings.recentProjects).toHaveLength(10);
		const lowered = mergeSettings(
			{ ...defaultSettings, recentProjects: many.slice(0, 10) },
			{ recentLimit: 5 },
		);
		expect(lowered.ok && lowered.settings.recentProjects).toHaveLength(5);
		expect(pushRecent(['/a', '/b', '/c'], '/b', 10)).toEqual(['/b', '/a', '/c']);
		expect(pushRecent(['/a', '/b'], '/c', 2)).toEqual(['/c', '/a']);
	});
});

describe('reading', () => {
	it('fills in the default for a field that is missing or invalid and keeps the rest', () => {
		expect(normalizeSettings(undefined)).toEqual(defaultSettings);
		expect(normalizeSettings([])).toEqual(defaultSettings);
		expect(
			normalizeSettings({ menuBar: true, theme: 'neon', tabWidth: 'wide', wrapLines: false }),
		).toEqual({ ...defaultSettings, menuBar: true, wrapLines: false });
	});

	it('migrates the first file, which had only the menu bar and the theme and no version', () => {
		const migrated = normalizeSettings({ menuBar: true, theme: 'afterglow-light' });
		expect(migrated).toEqual({
			...defaultSettings,
			menuBar: true,
			theme: 'afterglow-light',
			version: SETTINGS_VERSION,
		});
	});

	it('does not lower a newer version', () => {
		expect(normalizeSettings({ version: 9 }).version).toBe(9);
	});

	it('compares settings field by field', () => {
		expect(sameSettings(defaultSettings, { ...defaultSettings })).toBe(true);
		expect(sameSettings(defaultSettings, { ...defaultSettings, recentProjects: ['/a'] })).toBe(
			false,
		);
	});
});

describe('the browser port', () => {
	it('writes a merge, refuses a bad patch with the reason and tells listeners', async () => {
		const port = browserSettings(memory());
		const seen: number[] = [];
		await port.onChange((s) => seen.push(s.tabWidth));
		await expect(port.write({ tabWidth: 2 })).resolves.toMatchObject({ tabWidth: 2 });
		await expect(port.write({ tabWidth: 99 })).rejects.toThrow('tab width must be between 1 and 8');
		expect((await port.read()).tabWidth).toBe(2);
		expect(seen).toEqual([2]);
	});

	it('keeps keys it does not know when it writes', async () => {
		const storage = memory(JSON.stringify({ menuBar: true, futureThing: { a: 1 } }));
		expect(unknownKeys(JSON.parse(storage.getItem('liminal-installer-studio.settings')!))).toEqual({
			futureThing: { a: 1 },
		});
		const port = browserSettings(storage);
		await port.write({ tabWidth: 2 });
		const saved = JSON.parse(storage.data.get('liminal-installer-studio.settings')!);
		expect(saved).toMatchObject({
			futureThing: { a: 1 },
			menuBar: true,
			tabWidth: 2,
			version: SETTINGS_VERSION,
		});
	});

	it('resets to the defaults', async () => {
		const port = browserSettings(memory());
		await port.write({ tabWidth: 2 });
		await expect(port.reset()).resolves.toEqual(defaultSettings);
	});
});

describe('parity with the Rust schema', () => {
	const rust = readFileSync(new URL('../../../src-tauri/src/settings.rs', import.meta.url), 'utf8');
	const range = (name: string) => {
		const m = rust.match(
			new RegExp(`${name}: std::ops::RangeInclusive<u32> = (\\d+)\\.\\.=(\\d+)`),
		);
		return m ? [Number(m[1]), Number(m[2])] : undefined;
	};
	const list = (name: string) =>
		[
			...(
				rust.match(new RegExp(`const ${name}: \\[&str; \\d+\\] = \\[([^\\]]*)\\]`))?.[1] ?? ''
			).matchAll(/"([^"]+)"/g),
		].map((m) => m[1]);

	it('has the same version, ranges and choices', () => {
		expect(rust).toContain(`pub const VERSION: u32 = ${SETTINGS_VERSION};`);
		expect(range('RECENT_LIMIT')).toEqual([...LIMITS.recentLimit]);
		expect(range('TEXT_SIZE')).toEqual([...LIMITS.textSize]);
		expect(range('FONT_SIZE')).toEqual([...LIMITS.editorFontSize]);
		expect(range('TAB_WIDTH')).toEqual([...LIMITS.tabWidth]);
		expect(list('THEMES')).toEqual([...THEMES]);
		expect(list('DENSITIES')).toEqual([...DENSITIES]);
		expect(list('MOTIONS')).toEqual([...MOTIONS]);
	});

	it('names every setting in the Rust struct', () => {
		const fields = [
			...rust
				.slice(rust.indexOf('pub struct Settings'), rust.indexOf('impl Default for Settings'))
				.matchAll(/pub (\w+):/g),
		]
			.map((m) => m[1]!)
			.filter((f) => f !== 'extra')
			.map((f) => f.replace(/_(\w)/g, (_, c: string) => c.toUpperCase()));
		expect(fields.sort()).toEqual(Object.keys(defaultSettings).sort());
	});
});
