// Tests the Tauri screen's pure helpers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	baseName,
	fieldBadges,
	formatSizeCost,
	hasTwoUpdaters,
	ifMissingText,
	ifPresentText,
	isIntervalText,
	nextOverrides,
	settingsLabel,
	sortProblems,
	updateNote,
	updatesPatch,
	webview2Overridden,
	webview2Patch,
} from './tauri';
import { tauriViewFixture } from './tauriFixture';
import type { TauriIdentityRow } from './types';

const row = (patch: Partial<TauriIdentityRow>): TauriIdentityRow => ({
	field: 'version',
	imported: '1',
	project: '1',
	overridden: false,
	differs: false,
	...patch,
});

describe('fieldBadges', () => {
	it('says Imported for a value that matches the import', () => {
		expect(fieldBadges(row({}))).toEqual(['imported']);
	});
	it('says Differs when the next import would change the project', () => {
		expect(fieldBadges(row({ project: '0', differs: true }))).toEqual(['differs']);
	});
	it('says Overridden when the project keeps its own value, even if the two differ', () => {
		expect(fieldBadges(row({ project: '0', differs: true, overridden: true }))).toEqual([
			'overridden',
		]);
	});
	it('has nothing to say before anything is imported', () => {
		expect(fieldBadges(row({ imported: null }))).toEqual([]);
	});
});

describe('nextOverrides', () => {
	const rows = [row({ field: 'name', overridden: true }), row({ field: 'version' })];
	it('adds and removes one field and keeps the rest', () => {
		expect(nextOverrides(rows, 'version', true)).toEqual(['name', 'version']);
		expect(nextOverrides(rows, 'name', false)).toEqual([]);
		expect(nextOverrides(rows, 'name', true)).toEqual(['name']);
	});
});

describe('size cost', () => {
	it('shows the megabytes an option adds', () => {
		expect(formatSizeCost(127)).toBe('+127 MB');
		expect(formatSizeCost(0)).toBe('No extra size');
	});
});

describe('WebView2 overrides', () => {
	const w = tauriViewFixture().webview2!;
	it('is overridden only when a field is set', () => {
		expect(webview2Overridden(w)).toBe(false);
		expect(webview2Overridden({ ...w, override: {} })).toBe(false);
		expect(webview2Overridden({ ...w, override: { mode: null } })).toBe(false);
		expect(webview2Overridden({ ...w, override: { silent: false } })).toBe(true);
	});
	it('adds a field to the override and takes an emptied one out', () => {
		const withMode = webview2Patch(w, { mode: 'offlineInstaller' });
		expect(withMode).toEqual({ mode: 'offlineInstaller' });
		const both = webview2Patch({ ...w, override: withMode }, { min_version: '120.0' });
		expect(both).toEqual({ mode: 'offlineInstaller', min_version: '120.0' });
		expect(webview2Patch({ ...w, override: both }, { min_version: '' })).toEqual({
			mode: 'offlineInstaller',
		});
		expect(webview2Patch({ ...w, override: { silent: false, path: null } }, {})).toEqual({
			silent: false,
		});
	});
});

describe('the step sentences', () => {
	it('maps each key to a message and falls back for an unknown one', () => {
		for (const key of ['skip', 'fixed', 'download', 'bootstrapper', 'offline'])
			expect(ifMissingText(key)).not.toContain('see the plan');
		expect(ifPresentText('found-checked', '110.0')).toContain('version 110.0');
		expect(ifPresentText('found', null)).toBe('it is used and nothing is installed.');
		expect(ifMissingText('teleport')).toContain('teleport');
	});
});

describe('updates', () => {
	const u = tauriViewFixture().updates;
	it('names the two-updater conflict and its resolution', () => {
		expect(updateNote('liminal', true)).toBe('tauri.updates.noteTwoUpdaters');
		expect(updateNote('liminal', false)).toBe('tauri.updates.noteLiminal');
		expect(updateNote('both', true)).toBe('tauri.updates.noteBoth');
		expect(updateNote('tauri', false)).toBe('tauri.updates.noteNoUpdater');
		expect(hasTwoUpdaters({ ...u, mode: 'liminal' })).toBe(true);
		expect(hasTwoUpdaters({ ...u, mode: 'both' })).toBe(false);
		expect(hasTwoUpdaters({ ...u, mode: 'liminal', app_has_updater: false })).toBe(false);
	});
	it('rebuilds the whole [updates] table with one field changed', () => {
		expect(updatesPatch(u, { mode: 'none' })).toEqual({
			updates: {
				mode: 'none',
				feed: 'https://releases.example/feed.json',
				channel: 'stable',
				check_interval_hours: 24,
			},
		});
		expect(updatesPatch(u, { feed: ' ', interval: '' }).updates).toEqual({
			mode: 'both',
			channel: 'stable',
		});
		expect(updatesPatch(u, { interval: '0' }).updates?.check_interval_hours).toBe(0);
	});
	it('accepts an empty or whole-number interval only', () => {
		expect(['', '12', ' 3 '].every(isIntervalText)).toBe(true);
		expect(['-1', '1.5', 'x'].some(isIntervalText)).toBe(false);
	});
});

describe('sortProblems', () => {
	it('lists errors first and keeps the order within a group', () => {
		const sorted = sortProblems([
			{ severity: 'warning', scope: 'a', message: '1' },
			{ severity: 'error', scope: 'b', message: '2' },
			{ severity: 'warning', scope: 'c', message: '3' },
			{ severity: 'error', scope: 'd', message: '4' },
		]);
		expect(sorted.map((p) => p.message)).toEqual(['2', '4', '1', '3']);
	});
});

describe('labels', () => {
	it('names the part of the settings a change touches', () => {
		expect(settingsLabel({ windows: 'wrap' })).toBe('Install the Tauri app on Windows as wrap');
		expect(settingsLabel({ webview2: {} })).toBe('Change the WebView2 setting');
		expect(settingsLabel({})).toBe('Change the Tauri settings');
		expect(baseName('/a/b/tauri.conf.json')).toBe('tauri.conf.json');
		expect(baseName('C:\\x\\tauri.conf.json')).toBe('tauri.conf.json');
	});
});
