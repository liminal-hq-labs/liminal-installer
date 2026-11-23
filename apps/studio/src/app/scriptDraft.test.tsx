// @vitest-environment happy-dom
// Tests the Script screen's draft, dirty, save, discard, selection and test-run logic around the whole app, with a fake backend.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { EditorView } from '@codemirror/view';
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { startApp, type App } from './startApp';
import { createChromeStore, type ChromeStore } from '../chrome/chromeStore';
import { SCREEN_ACTIONS } from '../chrome/menuModel';
import { noTerminal, type Api } from '../lib/api';
import { browserSettings } from '../lib/settings';
import type { ProjectSummary, ScriptReport } from '../lib/types';

// React reports state changes outside `act` when this is set; each test settles the app inside `act`.
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const SOURCES: Record<string, string> = {
	'scripts/a.lua': '-- capabilities: env\nfunction detect(page)\nend\n',
	'scripts/b.lua': 'function validate(page)\nend\n',
};

const summary = (): ProjectSummary => ({
	root: '/demo/installer',
	id: 'demo',
	name: 'Demo',
	publisher: 'x',
	version: '1.0.0',
	default_theme: null,
	targets: [
		{
			full_id: 'dos-x86:tui-retro',
			id: 'dos-x86',
			renderer: 'tui-retro',
			theme: 't',
			outputs: [],
			components: [],
		},
		{
			full_id: 'linux-x86_64:tui',
			id: 'linux-x86_64',
			renderer: 'tui',
			theme: 't',
			outputs: [],
			components: [],
		},
	],
	components: [],
	themes: [],
	flows: [],
	layouts: [],
	scripts: Object.keys(SOURCES),
});

function fakeApi() {
	const files = { ...SOURCES };
	const writes: { path: string; source: string }[] = [];
	const tests: { path: string; source: string | undefined; target: string | undefined }[] = [];
	const report = (target?: string): ScriptReport => ({
		ok: true,
		hooks: [{ name: 'detect', when: 'A page opens', present: true }],
		capabilities: [],
		diagnostics: [],
		output: [],
		target: target ?? null,
		tests: [{ hook: 'detect', ok: true, summary: 'ok, set nothing', lines: [] }],
	});
	const api = {
		openProject: async () => summary(),
		projectSummary: async () => summary(),
		listTargets: async () => summary().targets,
		getFlow: async () => {
			throw new Error('no flow');
		},
		readScript: async (path: string) => files[path] ?? '',
		writeScript: async (path: string, source: string) => {
			writes.push({ path, source });
			files[path] = source;
		},
		testScript: async (path: string, source?: string, target?: string) => {
			tests.push({ path, source, target });
			return report(target);
		},
		checkScriptSyntax: async () => [],
		runPreview: async () => {
			throw new Error('no preview');
		},
		getPreview: async () => null,
		...noTerminal,
		checkProject: async () => [],
		history: async () => ({ entries: [], can_undo: false, can_redo: false }),
		undo: async () => ({ entries: [], can_undo: false, can_redo: false }),
		redo: async () => ({ entries: [], can_undo: false, can_redo: false }),
		mcpInfo: async () => null,
		onChange: async () => () => undefined,
		pickProject: async () => null,
	} as unknown as Api;
	return { api, writes, tests };
}

const settle = () =>
	act(async () => {
		for (let i = 0; i < 8; i++) await new Promise((r) => setTimeout(r, 0));
	});

const choose = async (select: HTMLSelectElement, value: string) => {
	await act(async () => {
		select.value = value;
		select.dispatchEvent(new Event('change', { bubbles: true }));
	});
	await settle();
};

describe('the script draft', () => {
	let root: HTMLElement;
	let chrome: ChromeStore;
	let app: App;
	let fake: ReturnType<typeof fakeApi>;
	let guard: ReturnType<typeof vi.fn<() => Promise<boolean>>>;

	const view = () => EditorView.findFromDOM(document.querySelector('.cm-editor') as HTMLElement)!;
	const type = (text: string) =>
		act(async () => {
			view().dispatch({ changes: { from: view().state.doc.length, insert: text } });
		});
	const text = () => view().state.doc.toString();
	const go = async (screen: keyof typeof SCREEN_ACTIONS) => {
		await act(async () => {
			await app.dispatch(SCREEN_ACTIONS[screen]);
		});
		await settle();
	};
	const dirtyLabel = () => document.getElementById('script-dirty')?.textContent;

	beforeEach(async () => {
		document.body.innerHTML = '<div id="root"></div>';
		root = document.getElementById('root')!;
		chrome = createChromeStore(browserSettings(undefined));
		await chrome.init();
		fake = fakeApi();
		guard = vi.fn<() => Promise<boolean>>();
		await act(async () => {
			app = await startApp(root, chrome, { header: false, api: fake.api, settleUnsaved: guard });
		});
		await go('script');
	});

	afterEach(() => {
		document.body.innerHTML = '';
	});

	it('shows the first script in the editor, saved', () => {
		expect(text()).toBe(SOURCES['scripts/a.lua']);
		expect(dirtyLabel()).toBe('Saved');
		expect(chrome.get().menu.scriptDirty).toBe(false);
	});

	it('marks the draft dirty as it is typed in, and clean again when the text matches the file', async () => {
		await type('-- more\n');
		expect(dirtyLabel()).toBe('Unsaved changes');
		expect(chrome.get().menu.scriptDirty).toBe(true);
		await act(async () => {
			view().dispatch({
				changes: {
					from: SOURCES['scripts/a.lua']!.length,
					to: view().state.doc.length,
					insert: '',
				},
			});
		});
		expect(dirtyLabel()).toBe('Saved');
		expect(chrome.get().menu.scriptDirty).toBe(false);
	});

	it('keeps unsaved edits across other screens and redraws', async () => {
		await type('-- keep me\n');
		await go('project');
		expect(document.querySelector('.cm-editor')).toBeNull();
		await go('script');
		expect(document.querySelector('.cm-editor')).not.toBeNull();
		expect(text()).toContain('-- keep me');
		expect(dirtyLabel()).toBe('Unsaved changes');
		await act(async () => chrome.setTitle('forces a redraw elsewhere'));
		expect(text()).toContain('-- keep me');
	});

	it('saves the draft from any screen with File > Save', async () => {
		await type('-- saved from elsewhere\n');
		await go('project');
		expect(chrome.get().menu.scriptDirty).toBe(true);
		await act(async () => {
			await app.dispatch('file.save');
		});
		await settle();
		expect(fake.writes).toEqual([
			{ path: 'scripts/a.lua', source: `${SOURCES['scripts/a.lua']}-- saved from elsewhere\n` },
		]);
		expect(chrome.get().menu.scriptDirty).toBe(false);
		await go('script');
		expect(dirtyLabel()).toBe('Saved');
		expect(text()).toContain('saved from elsewhere');
	});

	it('formats on save when that setting is on, and the editor shows what was written', async () => {
		await act(async () => chrome.updateSettings({ formatOnSave: true }));
		await type('if x then\ny()   \nend\n');
		await act(async () => {
			await app.dispatch('file.save');
		});
		expect(fake.writes[0]!.source).toBe(`${SOURCES['scripts/a.lua']}if x then\n\ty()\nend\n`);
		expect(text()).toBe(fake.writes[0]!.source);
	});

	it('discards the draft and clears the dirty flag', async () => {
		await type('-- oops\n');
		await act(async () => app.discard());
		expect(text()).toBe(SOURCES['scripts/a.lua']);
		expect(dirtyLabel()).toBe('Saved');
		expect(chrome.get().menu.scriptDirty).toBe(false);
		expect(fake.writes).toEqual([]);
	});

	it('asks before another script replaces unsaved edits, and stays when the answer is no', async () => {
		await type('-- unsaved\n');
		guard.mockResolvedValue(false);
		await choose(
			document.querySelector<HTMLSelectElement>('select[aria-label="Script"]')!,
			'scripts/b.lua',
		);
		expect(guard).toHaveBeenCalledTimes(1);
		expect(text()).toContain('-- unsaved');
		expect(document.querySelector<HTMLSelectElement>('select[aria-label="Script"]')!.value).toBe(
			'scripts/a.lua',
		);
	});

	it('opens the other script once the edits are discarded', async () => {
		await type('-- unsaved\n');
		guard.mockImplementation(async () => {
			app.discard();
			return true;
		});
		await choose(
			document.querySelector<HTMLSelectElement>('select[aria-label="Script"]')!,
			'scripts/b.lua',
		);
		expect(text()).toBe(SOURCES['scripts/b.lua']);
		expect(chrome.get().menu.scriptDirty).toBe(false);
	});

	it('does not ask when the open script is clean', async () => {
		await choose(
			document.querySelector<HTMLSelectElement>('select[aria-label="Script"]')!,
			'scripts/b.lua',
		);
		expect(guard).not.toHaveBeenCalled();
		expect(text()).toBe(SOURCES['scripts/b.lua']);
	});
});

describe('the test run', () => {
	let root: HTMLElement;
	let chrome: ChromeStore;
	let app: App;
	let fake: ReturnType<typeof fakeApi>;
	const view = () => EditorView.findFromDOM(document.querySelector('.cm-editor') as HTMLElement)!;
	const status = () => document.getElementById('script-test-status')?.textContent ?? '';
	const press = () =>
		act(async () => {
			[...document.querySelectorAll('button')].find((b) => b.textContent === 'Test run')!.click();
		});
	const settle2 = settle;

	beforeEach(async () => {
		document.body.innerHTML = '<div id="root"></div>';
		root = document.getElementById('root')!;
		chrome = createChromeStore(browserSettings(undefined));
		await chrome.init();
		fake = fakeApi();
		await act(async () => {
			app = await startApp(root, chrome, { header: false, api: fake.api });
		});
		await act(async () => {
			await app.dispatch(SCREEN_ACTIONS.script);
		});
		await settle2();
	});

	it('shows Running… and then when it ran, how long it took and whether the text was saved', async () => {
		let release!: () => void;
		const original = fake.api.testScript;
		fake.api.testScript = (...args) =>
			new Promise((resolve) => (release = () => resolve(original(...args))));
		await press();
		expect(status()).toBe('Running…');
		await act(async () => release());
		await settle2();
		expect(status()).toMatch(/^Tested at \d\d:\d\d:\d\d · \d+ ms \(saved file\)$/);
		expect(document.getElementById('script-console')!.textContent).toContain('✓');
		expect(document.getElementById('script-console')!.textContent).toContain('detect(page)');
		expect(document.querySelector('[data-testid="toast"]')?.textContent).toContain('Test passed');
	});

	it('runs the unsaved draft against the chosen target and says so', async () => {
		await act(async () => {
			view().dispatch({ changes: { from: view().state.doc.length, insert: '-- draft\n' } });
		});
		await choose(
			document.querySelector<HTMLSelectElement>('select[aria-label="Test target"]')!,
			'linux-x86_64:tui',
		);
		await press();
		await settle2();
		const run = fake.tests.at(-1)!;
		expect(run.target).toBe('linux-x86_64:tui');
		expect(run.source!.endsWith('-- draft\n')).toBe(true);
		expect(status()).toMatch(/\(unsaved text\)$/);
		expect(document.getElementById('script-console')!.textContent).toContain(
			'test run on linux-x86_64:tui',
		);
	});

	it('marks the report out of date as soon as the draft changes', async () => {
		await press();
		await settle2();
		expect(status()).not.toContain('out of date');
		expect(document.getElementById('script-stale')!.hidden).toBe(true);
		await act(async () => {
			view().dispatch({ changes: { from: 0, insert: 'x' } });
		});
		expect(status()).toContain('out of date');
		expect(document.getElementById('script-stale')!.hidden).toBe(false);
		expect(document.getElementById('script-console')!.className).toMatch(/stale/);
	});

	it('shows a failure in the result area, not only in a toast', async () => {
		fake.api.testScript = async (_p, _s, target) => ({
			ok: false,
			hooks: [],
			capabilities: [],
			diagnostics: [
				{ severity: 'error', scope: 's', message: 'on_commit: capability `env` is denied' },
			],
			output: [],
			target: target ?? null,
			tests: [
				{
					hook: 'on_commit',
					ok: false,
					summary: 'on_commit: capability `env` is denied',
					lines: [],
				},
			],
		});
		await press();
		await settle2();
		const text = document.getElementById('script-console')!.textContent!;
		expect(text).toContain('✗');
		expect(text).toContain('capability `env` is denied');
		expect(text).toContain('failed');
		expect(document.querySelector('[data-testid="toast"]')?.textContent).toContain('Test failed');
	});

	it('shows an error the backend raises in the console too', async () => {
		fake.api.testScript = async () => {
			throw new Error('unknown target `nope`');
		};
		await press();
		await settle2();
		expect(document.getElementById('script-console')!.textContent).toContain(
			'unknown target `nope`',
		);
		expect(status()).toMatch(/^Could not run at/);
		expect(document.querySelector('[data-testid="toast"][data-error="true"]')).not.toBeNull();
	});
});
