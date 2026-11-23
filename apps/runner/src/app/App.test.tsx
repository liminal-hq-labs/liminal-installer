// @vitest-environment happy-dom
// Tests the window: the page is drawn by the shared renderer, buttons and keys answer it, and a screen reader hears it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';
import { copying, done, info, licence, page, welcome } from '../lib/fixtures';
import { createRunner, type Runner } from '../lib/runner';
import { createTestApi, type TestApi } from '../lib/testApi';
import { App } from './App';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root | undefined;
let host: HTMLElement | undefined;
afterEach(() => {
	act(() => root?.unmount());
	host?.remove();
});

async function mount(api: TestApi): Promise<Runner> {
	const runner = createRunner(api);
	host = document.createElement('div');
	document.body.append(host);
	root = createRoot(host);
	await act(async () => {
		root!.render(<App runner={runner} />);
		await runner.start();
	});
	return runner;
}

const button = (label: string): HTMLButtonElement =>
	[...document.querySelectorAll<HTMLButtonElement>('button')].find((b) =>
		b.textContent?.includes(label),
	)!;

const press = (key: string, init: KeyboardEventInit = {}) =>
	act(() => {
		document.body.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, ...init }));
	});

describe('the window', () => {
	it('draws the page with the project’s own text and names the window', async () => {
		const api = createTestApi([{ type: 'info', info }, page('welcome', 'page', 1, welcome)]);
		await mount(api);
		expect(document.body.textContent).toContain('Welcome to Test Product');
		expect(document.querySelector('main')?.getAttribute('aria-label')).toBe(
			'Setup for Test Product',
		);
		expect(document.querySelector('[role=status][aria-live=polite]')?.textContent).toBe('Welcome');
	});

	it('answers Next from the button, and from Enter and Esc on the page', async () => {
		const api = createTestApi([{ type: 'info', info }, page('welcome', 'page', 1, welcome)]);
		await mount(api);
		await press('Enter');
		expect(api.answers.map((a) => a.action)).toEqual(['next']);
		await act(async () => api.emit(page('welcome', 'page', 2, welcome)));
		await press('Escape');
		expect(api.answers.map((a) => a.action)).toEqual(['next', 'cancel']);
		await act(async () => api.emit(page('welcome', 'page', 3, welcome)));
		await act(async () => button('Next').click());
		expect(api.answers.map((a) => a.action)).toEqual(['next', 'cancel', 'next']);
	});

	it('switches Next off until the shell says the page allows it', async () => {
		const api = createTestApi([
			{ type: 'info', info },
			page('licence', 'page', 1, licence, { disabled: ['1.1'] }),
		]);
		api.nextUpdate = { disabled: [], vars: {} };
		const runner = await mount(api);
		expect(button('Next').disabled).toBe(true);
		await press('Enter');
		expect(api.answers).toEqual([]);
		await act(async () => runner.change('licence.accepted', 'yes'));
		expect(button('Next').disabled).toBe(false);
		await press('Enter');
		expect(api.answers).toHaveLength(1);
	});

	it('presses the default button with Ctrl+Enter even from a choice', async () => {
		const api = createTestApi([
			{ type: 'info', info },
			page('licence', 'page', 1, licence, { state: { 'licence.accepted': 'yes' } }),
		]);
		await mount(api);
		const radio = document.querySelector<HTMLElement>('[role=radio]')!;
		await act(async () => {
			radio.dispatchEvent(
				new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
			);
		});
		expect(api.answers).toEqual([]);
		await act(async () => {
			radio.dispatchEvent(
				new KeyboardEvent('keydown', {
					key: 'Enter',
					ctrlKey: true,
					bubbles: true,
					cancelable: true,
				}),
			);
		});
		expect(api.answers.map((a) => a.action)).toEqual(['next']);
	});

	it('goes back with Alt+Left', async () => {
		const api = createTestApi([{ type: 'info', info }, page('licence', 'page', 2, licence)]);
		await mount(api);
		await press('ArrowLeft', { altKey: true });
		expect(api.answers.map((a) => a.action)).toEqual(['back']);
	});

	it('leaves Esc alone during the commit, and cancels with the button', async () => {
		const api = createTestApi([{ type: 'info', info }, page('copying', 'commit', 3, copying)]);
		await mount(api);
		await press('Escape');
		expect(api.cancels).toBe(0);
		await act(async () => button('Cancel').click());
		expect(api.cancels).toBe(1);
	});

	it('announces progress by tenths while the commit runs', async () => {
		const api = createTestApi([{ type: 'info', info }, page('copying', 'commit', 3, copying)]);
		await mount(api);
		await act(async () =>
			api.emit({
				type: 'progress',
				progress: {
					phase: 'commit',
					file: 'a',
					files_done: 1,
					files_total: 2,
					total: 0.55,
					file_fraction: 0.5,
					free_fraction: 0.9,
					low_space: false,
					vars: { 'progress.percent': '55' },
				},
			}),
		);
		expect(document.querySelector('[role=status][aria-live=polite]')?.textContent).toBe(
			'50% complete',
		);
		// The renderer's own meter follows the same readings.
		expect(document.querySelector('[role=meter]')?.getAttribute('aria-valuenow')).toBe('55');
	});

	it('draws its own progress screen when the run has no flow, and its own end screen', async () => {
		const api = createTestApi([{ type: 'info', info: { ...info, mode: 'repair' } }]);
		await mount(api);
		expect(document.body.textContent).toContain('Repairing Test Product');
		await act(async () => api.emit({ type: 'done' }));
		expect(document.body.textContent).toContain('Test Product has been repaired');
		await act(async () => button('Close').click());
		expect(api.exits).toBe(1);
	});

	it('says why a run failed, after the engine undid it', async () => {
		const api = createTestApi([{ type: 'info', info }, page('done', 'finish', 5, done)]);
		await mount(api);
		await act(async () => api.emit({ type: 'failed', message: 'no space left' }));
		expect(document.body.textContent).toContain('Setup could not finish');
		expect(document.body.textContent).toContain('no space left');
	});
});
