// @vitest-environment happy-dom
// Tests the Tauri screen through the app: what it shows, the commands its controls send, and how it follows the project.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createChromeStore } from '../chrome/chromeStore';
import { SCREEN_ACTIONS, shortcutTable } from '../chrome/menuModel';
import { noTerminal, type Api } from '../lib/api';
import { browserSettings } from '../lib/settings';
import { tauriViewFixture } from '../lib/tauriFixture';
import type { ChangeEvent, ProjectSummary, TauriView } from '../lib/types';
import { startApp, type App } from './startApp';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const project = (root: string): ProjectSummary => ({
	root,
	id: 'p',
	name: 'Plume',
	publisher: 'Plume Labs',
	version: '1.0.0',
	default_theme: null,
	targets: [],
	components: [],
	themes: [],
	flows: [],
	layouts: [],
	scripts: [],
});

function boot(initial: TauriView) {
	let view = initial;
	let current = project('/a');
	const listeners = new Set<(e: ChangeEvent) => void>();
	const calls = {
		importTauri: vi.fn(async (_c: string | null, _l: string) => ({
			view,
			first_import: false,
			changes: [{ field: 'product.version', from: '1.4.1', to: '1.4.2' }],
			filled: [],
			kept: ['name'],
			components: [],
			warnings: ['resources has a glob that matches nothing'],
		})),
		setTauriSettings: vi.fn(async (_c: unknown, _l: string) => view),
		getTauri: vi.fn(async () => view),
		pickTauriConfig: vi.fn(async (): Promise<string | null> => '/a/src-tauri/tauri.conf.json'),
	};
	const api = {
		openProject: async (path: string) => (current = project(path)),
		projectSummary: async () => current,
		getFlow: async () => ({ flow: { id: 'x', steps: [] }, errors: [] }),
		getLayout: async () => ({ id: 'x' }),
		getPreview: async () => null,
		...noTerminal,
		checkProject: async () => [],
		history: async () => ({ entries: [], can_undo: false, can_redo: false }),
		mcpInfo: async () => null,
		mcpError: async () => null,
		onChange: async (h: (e: ChangeEvent) => void) => {
			listeners.add(h);
			return () => listeners.delete(h);
		},
		pickProject: async () => null,
		...calls,
	} as unknown as Api;
	return {
		api,
		calls,
		setView: (next: TauriView) => (view = next),
		emit: (doc: ChangeEvent['doc']) =>
			listeners.forEach((l) =>
				l({ kind: 'edit', doc, label: 'x', source: 'ui', can_undo: true, can_redo: false }),
			),
	};
}

async function mount(initial: TauriView) {
	const f = boot(initial);
	const chrome = createChromeStore(browserSettings(undefined));
	await chrome.init();
	const root = document.createElement('div');
	document.body.append(root);
	let app!: App;
	await act(async () => {
		app = await startApp(root, chrome, { header: false, api: f.api });
	});
	await act(async () => {
		await app.dispatch(SCREEN_ACTIONS.tauri);
	});
	const button = (name: string) =>
		[...root.querySelectorAll('button')].find((b) => b.textContent === name)!;
	const click = (el: Element) => act(async () => (el as HTMLElement).click());
	const radio = (name: string) =>
		[...root.querySelectorAll<HTMLInputElement>('input[type="radio"]')].find((r) =>
			r.closest('label')?.textContent?.startsWith(name),
		)!;
	return { ...f, app, chrome, root, button, click, radio };
}

afterEach(() => {
	document.body.replaceChildren();
});

describe('the Tauri screen', () => {
	it('is the sixth screen, with Ctrl+6 in the shortcuts table and its name in the title', async () => {
		const { root, chrome } = await mount(tauriViewFixture());
		expect(root.querySelector('main h1')?.textContent).toBe('Tauri');
		expect(chrome.get().title).toBe('Plume · Tauri');
		expect(root.querySelector('nav a[aria-current="page"]')?.textContent).toBe('Tauri');
		const view = shortcutTable().find((g) => g.menu === 'View')!;
		expect(view.rows.find((r) => r.id === 'view.tauri')?.keys).toEqual(['Ctrl+6']);
	});

	it('shows an explanation and the import button for a project with no Tauri app', async () => {
		const { root, button, calls, click } = await mount(
			tauriViewFixture({
				configured: false,
				config: null,
				imported: false,
				identity: [],
				webview2: null,
			}),
		);
		expect(root.textContent).toContain('This project has no Tauri app yet');
		expect(root.querySelector('table')).toBeNull();
		await click(button('Import tauri.conf.json…'));
		expect(calls.pickTauriConfig).toHaveBeenCalledWith('/a');
		expect(calls.importTauri).toHaveBeenCalledWith(
			'/a/src-tauri/tauri.conf.json',
			'Import tauri.conf.json',
		);
	});

	it('imports a picked file, re-imports the project’s own config, and summarises the result', async () => {
		const { root, button, calls, click } = await mount(tauriViewFixture());
		expect(root.textContent).toContain('src-tauri/tauri.conf.json');
		expect(root.textContent).toContain('Found');
		await click(button('Re-import'));
		expect(calls.importTauri).toHaveBeenLastCalledWith(null, 'Import tauri.conf.json again');
		const summary = root.querySelector('[data-testid="import-summary"]')!.textContent!;
		expect(summary).toContain('1 value changed');
		expect(summary).toContain('Kept your values for: name');
		expect(summary).toContain('1 warning');
		expect(summary).toContain('resources has a glob');
	});

	it('does not import when the picker is cancelled', async () => {
		const { calls, button, click } = await mount(tauriViewFixture());
		calls.pickTauriConfig.mockResolvedValueOnce(null);
		await click(button('Import tauri.conf.json…'));
		expect(calls.importTauri).not.toHaveBeenCalled();
	});

	it('shows drift with the pending values and a Re-import button, or Up to date', async () => {
		const stale = tauriViewFixture({
			stale: true,
			drift: [{ file: 'src-tauri/tauri.conf.json', kind: 'changed' }],
			pending: [{ field: 'product.version', from: '1.4.1', to: '1.4.2' }],
		});
		const { root } = await mount(stale);
		const card = root.querySelector('[aria-labelledby="tauri-drift-h"]')!;
		expect(card.textContent).toContain('Out of date');
		expect(card.textContent).toContain('changed');
		expect([...card.querySelectorAll('tbody td')].map((c) => c.textContent)).toEqual([
			'1.4.1',
			'1.4.2',
		]);
		expect([...card.querySelectorAll('button')].map((b) => b.textContent)).toEqual(['Re-import']);
		document.body.replaceChildren();
		const fresh = await mount(tauriViewFixture());
		expect(fresh.root.querySelector('[aria-labelledby="tauri-drift-h"]')!.textContent).toContain(
			'Up to date',
		);
	});

	it('badges the product fields and keeps a project value through overrides', async () => {
		const view = tauriViewFixture();
		view.identity[1] = { ...view.identity[1]!, project: 'Notes', overridden: true, differs: true };
		view.identity[3] = { ...view.identity[3]!, project: '1.4.1', differs: true };
		const { root, calls, click } = await mount(view);
		const rows = [...root.querySelectorAll('[aria-labelledby="tauri-identity-h"] tbody tr')];
		const badge = (i: number) => rows[i]!.querySelector('span span')?.textContent;
		expect([badge(0), badge(1), badge(3)]).toEqual(['Imported', 'Overridden', 'Differs']);
		const keep = (field: string) =>
			root.querySelector<HTMLInputElement>(
				`input[aria-label="Keep the project’s value for ${field}"]`,
			)!;
		expect(keep('Name').checked).toBe(true);
		await click(keep('Version'));
		expect(calls.setTauriSettings).toHaveBeenCalledWith(
			{ overrides: ['name', 'version'] },
			'Change whether the project keeps its version',
		);
		await click(keep('Name'));
		expect(calls.setTauriSettings).toHaveBeenLastCalledWith({ overrides: [] }, expect.any(String));
	});

	it('chooses native or wrap and says why wrap matters when the app needs it', async () => {
		const { root, radio, calls, click } = await mount(tauriViewFixture({ needs_wrap: true }));
		expect(radio('Native').checked).toBe(true);
		expect(root.textContent).toContain('NSIS hooks or a custom template');
		expect(root.textContent).toContain('macOS and Linux are always native.');
		await click(radio('Wrap'));
		expect(calls.setTauriSettings).toHaveBeenCalledWith({ windows: 'wrap' }, expect.any(String));
	});

	it('lists the five WebView2 modes with their cost and sends a mode choice', async () => {
		const { root, radio, calls, click } = await mount(tauriViewFixture());
		const modes = [
			...root.querySelectorAll('[aria-labelledby="tauri-webview2-h"] input[type="radio"]'),
		];
		expect(modes).toHaveLength(5);
		expect(radio('Offline installer').closest('label')!.textContent).toContain('+127 MB');
		expect(radio('Offline installer').closest('label')!.textContent).toContain('Works offline');
		expect(radio('Embed bootstrapper').checked).toBe(true);
		await click(radio('Offline installer'));
		expect(calls.setTauriSettings).toHaveBeenCalledWith(
			{ webview2: { mode: 'offlineInstaller' } },
			expect.any(String),
		);
	});

	it('words both WebView2 cases and clears an override with Use imported value', async () => {
		const view = tauriViewFixture();
		view.webview2 = {
			...view.webview2!,
			override: { mode: 'skip' },
			if_missing: 'skip',
			if_present: 'skip',
		};
		const { root, button, calls, click } = await mount(view);
		const card = root.querySelector('[aria-labelledby="tauri-webview2-h"]')!.textContent!;
		expect(card).toContain('On a machine without WebView2: nothing is installed');
		expect(card).toContain('With WebView2 installed: nothing is done.');
		expect(card).toContain('Overridden');
		await click(button('Use imported value'));
		expect(calls.setTauriSettings).toHaveBeenCalledWith({ webview2: {} }, expect.any(String));
	});

	it('disables Use imported value while nothing is overridden, and commits a minimum version on blur', async () => {
		const { root, button, calls } = await mount(tauriViewFixture());
		expect((button('Use imported value') as HTMLButtonElement).disabled).toBe(true);
		const input = root.querySelector<HTMLInputElement>('input[type="text"]')!;
		expect(input.closest('label')!.textContent).toContain('Minimum version');
		await act(async () => {
			const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
			set.call(input, '120.0.1');
			input.dispatchEvent(new Event('input', { bubbles: true }));
		});
		expect(calls.setTauriSettings).not.toHaveBeenCalled();
		await act(async () => {
			input.dispatchEvent(new FocusEvent('focusout', { bubbles: true }));
		});
		expect(calls.setTauriSettings).toHaveBeenCalledWith(
			{ webview2: { min_version: '120.0.1' } },
			expect.any(String),
		);
	});

	it('explains the two-updater conflict and sends the whole updates table on a mode change', async () => {
		const view = tauriViewFixture();
		view.updates = { ...view.updates, mode: 'liminal', app_applies: false };
		const { root, calls } = await mount(view);
		const note = root.querySelector('[data-testid="update-note"]')!;
		expect(note.textContent).toContain('two updaters');
		expect(note.getAttribute('role')).toBe('alert');
		expect(root.textContent).toContain('https://releases.example/{{target}}');
		const select = root.querySelector<HTMLSelectElement>('select')!;
		await act(async () => {
			select.value = 'both';
			select.dispatchEvent(new Event('change', { bubbles: true }));
		});
		expect(calls.setTauriSettings).toHaveBeenCalledWith(
			{
				updates: {
					mode: 'both',
					feed: 'https://releases.example/feed.json',
					channel: 'stable',
					check_interval_hours: 24,
				},
			},
			expect.any(String),
		);
	});

	it('lists problems with errors first', async () => {
		const { root } = await mount(
			tauriViewFixture({
				problems: [
					{ severity: 'warning', scope: 'updates', message: 'two updaters' },
					{ severity: 'error', scope: 'tauri', message: 'config missing' },
				],
			}),
		);
		const items = [...root.querySelectorAll('[aria-labelledby="tauri-problems-h"] li')];
		expect(items.map((i) => i.textContent)).toEqual([
			'Errortauri config missing',
			'Warningupdates two updaters',
		]);
	});

	it('reloads when a Tauri change event arrives, and shows a failed load', async () => {
		const { root, setView, emit, calls } = await mount(tauriViewFixture());
		const before = calls.getTauri.mock.calls.length;
		setView(tauriViewFixture({ windows: 'wrap' }));
		await act(async () => emit({ type: 'tauri' }));
		expect(calls.getTauri.mock.calls.length).toBeGreaterThan(before);
		expect(
			root.querySelector<HTMLInputElement>('input[type="radio"]:checked')!.closest('label')!
				.textContent,
		).toContain('Wrap');
		calls.getTauri.mockRejectedValueOnce({ kind: 'io', message: 'tauri.conf.json: denied' });
		await act(async () => emit({ type: 'tauri' }));
		expect(root.querySelector('[role="alert"]')?.textContent).toBe('tauri.conf.json: denied');
	});

	it('forgets the import result when another project opens', async () => {
		const { root, app, button, click } = await mount(tauriViewFixture());
		await click(button('Re-import'));
		expect(root.querySelector('[data-testid="import-summary"]')).not.toBeNull();
		await act(async () => void (await app.openPath('/b')));
		expect(root.querySelector('[data-testid="import-summary"]')).toBeNull();
	});
});
