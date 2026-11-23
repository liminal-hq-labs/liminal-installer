// Tests the run's state: pages in, answers out, cancel, the end of a run and failures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { copying, done, info, licence, page, welcome } from './fixtures';
import { createRunner } from './runner';
import { createTestApi } from './testApi';

const wait = () => new Promise((r) => setTimeout(r, 0));

describe('the run', () => {
	it('starts with the events the shell already sent and titles the window', async () => {
		const api = createTestApi([{ type: 'info', info }, page('welcome', 'page', 1, welcome)]);
		const run = createRunner(api);
		await run.start();
		const s = run.store.getState();
		expect(s.phase).toBe('running');
		expect(s.page?.step).toBe('welcome');
		expect(s.vars['product.name']).toBe('Test Product');
		expect(api.titles).toEqual(['Test Product Setup']);
	});

	it('answers Next with the values on the page, and ignores a second press', async () => {
		const api = createTestApi([{ type: 'info', info }, page('licence', 'page', 1, licence)]);
		const run = createRunner(api);
		await run.start();
		run.change('licence.accepted', 'yes');
		run.act('next');
		run.act('next');
		await wait();
		expect(api.answers).toEqual([{ action: 'next', state: { 'licence.accepted': 'yes' } }]);
		// The next page makes a new answer possible.
		api.emit(page('copying', 'page', 2, welcome));
		run.act('back');
		expect(api.answers).toHaveLength(2);
		expect(api.answers[1]?.action).toBe('back');
	});

	it('takes the switched-off nodes and values the shell answers a change with', async () => {
		const api = createTestApi([{ type: 'info', info }, page('licence', 'page', 1, licence)]);
		api.nextUpdate = { disabled: [], vars: { 'install.size': '12 MB' } };
		const run = createRunner(api);
		await run.start();
		run.store.setState({ disabled: ['1.1.1'] });
		run.change('licence.accepted', 'yes');
		await wait();
		expect(api.updates).toEqual([{ 'licence.accepted': 'yes' }]);
		expect(run.store.getState().disabled).toEqual([]);
		expect(run.store.getState().vars['install.size']).toBe('12 MB');
	});

	it('mirrors a typed folder into the placeholder at once', async () => {
		const api = createTestApi([{ type: 'info', info }, page('folder', 'page', 1, welcome)]);
		const run = createRunner(api);
		await run.start();
		run.change('install.path', '/opt/app');
		expect(run.store.getState().vars['install.path']).toBe('/opt/app');
	});

	it('browses for a folder with the system chooser', async () => {
		const api = createTestApi([{ type: 'info', info }, page('folder', 'page', 1, welcome)]);
		api.folder = '/mnt/games/App';
		const run = createRunner(api);
		await run.start();
		run.act('browse');
		await wait();
		expect(run.store.getState().values['install.path']).toBe('/mnt/games/App');
	});

	it('follows the commit with progress and cancels it through the shell', async () => {
		const api = createTestApi([{ type: 'info', info }, page('copying', 'commit', 3, copying)]);
		const run = createRunner(api);
		await run.start();
		api.emit({
			type: 'progress',
			progress: {
				phase: 'commit',
				file: 'bin/app',
				files_done: 1,
				files_total: 4,
				total: 0.25,
				file_fraction: 0.25,
				free_fraction: 0.9,
				low_space: false,
				vars: { 'progress.file': 'bin/app', 'progress.percent': '25' },
			},
		});
		expect(run.store.getState().vars['progress.percent']).toBe('25');
		run.act('next');
		expect(api.answers).toEqual([]);
		run.act('cancel');
		expect(api.cancels).toBe(1);
		expect(api.answers).toEqual([]);
	});

	it('closes by itself after the flow’s own finish page', async () => {
		const api = createTestApi([{ type: 'info', info }, page('done', 'finish', 5, done)]);
		const run = createRunner(api);
		await run.start();
		api.emit({ type: 'done' });
		expect(api.exits).toBe(1);
	});

	it('shows an end screen when the run had no finish page, until Close', async () => {
		const api = createTestApi([{ type: 'info', info: { ...info, mode: 'repair' } }]);
		const run = createRunner(api);
		await run.start();
		api.emit({ type: 'done' });
		expect(api.exits).toBe(0);
		expect(run.store.getState().phase).toBe('done');
		run.act('close');
		expect(api.exits).toBe(1);
	});

	it('leaves when cancelled, and keeps the message when failed', async () => {
		const api = createTestApi([{ type: 'info', info }]);
		const run = createRunner(api);
		await run.start();
		api.emit({ type: 'failed', message: 'disk full' });
		expect(run.store.getState()).toMatchObject({ phase: 'failed', message: 'disk full' });
		expect(api.exits).toBe(0);
		api.emit({ type: 'cancelled' });
		expect(api.exits).toBe(1);
	});
});
