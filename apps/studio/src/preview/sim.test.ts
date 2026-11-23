// Tests the numbers behind an interactive preview: space, drives, stepping and the copy animation.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { PREVIEW_WINDOW, fitWindow, parsePreviewQuery, previewWindowQuery } from './fullScreen';
import { STAGE } from './model';
import { copyFrame, derive, driveOf, formatSize, nextFolder, stepFrom } from './sim';

const components = [
	{ id: 'game', files: 2 },
	{ id: 'music', files: 1 },
];

describe('derive', () => {
	it('adds up the chosen components and updates the space and the plan summary', () => {
		const all = derive(components, ['game', 'music'], 'C:\\FIZZ');
		expect(all.vars['install.size']).toBe('48 MB');
		expect(all.vars['plan.summary']).toBe('2 components · 3 files · 48 MB');
		const one = derive(components, ['game'], 'C:\\FIZZ');
		expect(one.vars['install.size']).toBe('32 MB');
		expect(one.files).toBe(2);
		expect(one.freeFraction).toBeGreaterThan(all.freeFraction);
	});

	it('follows the folder: a nearly full drive is low and a floppy does not fit', () => {
		const roomy = derive(components, ['game', 'music'], 'C:\\FIZZ');
		expect(roomy.fits).toBe(true);
		expect(roomy.freeFraction).toBeGreaterThan(0.05);
		const tight = derive(components, ['game', 'music'], 'D:\\GAMES\\FIZZ');
		expect(tight.fits).toBe(true);
		expect(tight.freeFraction).toBeLessThan(0.05);
		const floppy = derive(components, ['game'], 'A:\\FIZZ');
		expect(floppy.fits).toBe(false);
		expect(floppy.freeFraction).toBe(0);
	});

	it('reads a Unix path as a drive too', () => {
		expect(driveOf('/home/you/Sample').capacity).toBeGreaterThan(driveOf('/mnt/usb/x').capacity);
		expect(nextFolder('/home/you/Sample')).toBe('/mnt/usb/Sample');
		expect(nextFolder('A:\\FIZZ')).toBe('C:\\FIZZ');
	});
});

describe('formatSize', () => {
	it('writes KB, MB and GB', () => {
		expect(formatSize(0.5)).toBe('512 KB');
		expect(formatSize(48)).toBe('48 MB');
		expect(formatSize(12700)).toBe('12.4 GB');
	});
});

describe('stepFrom', () => {
	const steps = [{ skipped: false }, { skipped: true }, { skipped: false }, { skipped: true }];
	it('skips steps that do not apply and stops at the ends', () => {
		expect(stepFrom(steps, 0, 1)).toBe(2);
		expect(stepFrom(steps, 2, -1)).toBe(0);
		expect(stepFrom(steps, 0, -1)).toBe(0);
		expect(stepFrom(steps, 2, 1)).toBe(2);
	});
});

describe('copyFrame', () => {
	it('fills each file in turn while the total climbs, then pauses and starts again', () => {
		const start = copyFrame(0, 4);
		expect(start).toMatchObject({ file: 0, total: 0, fileIndex: 0 });
		const mid = copyFrame(1.6 * 2 + 0.8, 4);
		expect(mid.fileIndex).toBe(2);
		expect(mid.file).toBeCloseTo(0.5);
		expect(mid.total).toBeCloseTo(0.625);
		const end = copyFrame(1.6 * 4 + 0.2, 4);
		expect(end).toMatchObject({ file: 1, total: 1, fileIndex: 3 });
		expect(copyFrame(1.6 * 5 + 0.1, 4).total).toBeCloseTo(0.1 / 6.4);
	});

	it('moves the billboard on every few seconds', () => {
		expect(copyFrame(0, 3).slide).toBe(0);
		expect(copyFrame(3.6, 3).slide).toBe(1);
	});
});

describe('the full-screen window', () => {
	it('opens with the target and step and reads them back', () => {
		const q = previewWindowQuery('win16-x86:win16', 3);
		expect(parsePreviewQuery(`?window=preview&${q}`)).toEqual({
			target: 'win16-x86:win16',
			step: 3,
		});
		expect(parsePreviewQuery('?window=preview')).toBeNull();
		expect(parsePreviewQuery('?target=x&step=-2')?.step).toBe(0);
	});

	it('is sized to the look, scaled down to fit the screen with the aspect ratio kept', () => {
		const roomy = fitWindow('win95', { width: 3840, height: 2160 });
		expect(roomy.scale).toBe(1);
		expect(roomy.width).toBe(STAGE.win95.w);
		const small = fitWindow('win95', { width: 1366, height: 768 });
		expect(small.scale).toBeLessThan(1);
		expect(small.height).toBeLessThanOrEqual(768);
		expect(small.width / (small.height - 92)).toBeCloseTo(STAGE.win95.w / STAGE.win95.h, 1);
	});

	it('has a spec in the shell and a place in both capability files', () => {
		const tauri = new URL('../../src-tauri/', import.meta.url);
		const windows = readFileSync(new URL('src/windows.rs', tauri), 'utf8');
		expect(windows).toContain(`"${PREVIEW_WINDOW}" => Some(Spec`);
		for (const file of ['default.json', 'dev-automation.json']) {
			const cap = JSON.parse(readFileSync(new URL(`capabilities/${file}`, tauri), 'utf8'));
			expect(cap.windows).toContain(PREVIEW_WINDOW);
		}
		const caps = readFileSync(new URL('capabilities/default.json', tauri), 'utf8');
		expect(caps).toContain('core:window:allow-set-fullscreen');
	});
});
