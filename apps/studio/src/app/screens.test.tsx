// @vitest-environment happy-dom
// Tests the five screens through the router: each draws from the stores, and its controls save through the backend.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { createChromeStore } from '../chrome/chromeStore';
import { SCREEN_ACTIONS } from '../chrome/menuModel';
import { noTerminal, type Api } from '../lib/api';
import { browserSettings } from '../lib/settings';
import type { Flow, Layout, Preview, ProjectSummary } from '../lib/types';
import { startApp, type App } from './startApp';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const flow: Flow = {
	version: 1,
	id: 'install',
	start: 'welcome',
	steps: [
		{ id: 'welcome', type: 'page', layout: 'welcome', next: 'copy' },
		{ id: 'copy', type: 'commit', next: 'done' },
		{ id: 'done', type: 'finish' },
	],
};
const layout: Layout = {
	version: 1,
	id: 'welcome',
	root: {
		kind: 'window',
		props: { title: 'Welcome' },
		children: [{ kind: 'text', props: { text: 'Hello' } }],
	},
};
const summary: ProjectSummary = {
	root: '/p',
	id: 'p',
	name: 'P',
	publisher: 'x',
	version: '1.0.0',
	default_theme: null,
	targets: [
		{
			full_id: 'linux-x86_64:tui',
			id: 'linux-x86_64',
			renderer: 'tui',
			theme: null,
			outputs: [],
			components: [],
		},
	],
	components: [],
	themes: [],
	flows: [{ id: 'install', steps: 3, errors: 0 }],
	layouts: ['welcome'],
	scripts: [],
};
const preview: Preview = {
	target: 'linux-x86_64:tui',
	renderer: 'tui',
	theme: null,
	plan: { ops: [{ op: 'mkdir', path: '/opt/p' }] },
	problems: [],
	steps: [
		{ id: 'welcome', kind: 'page', skipped: false, layout_tree: layout.root, state: {} },
		{ id: 'copy', kind: 'commit', skipped: false, state: {} },
	],
	vars: {},
} as unknown as Preview;

function fake() {
	const saved: { flow: string[]; layout: string[] } = { flow: [], layout: [] };
	const api = {
		openProject: async () => summary,
		projectSummary: async () => summary,
		listTargets: async () => summary.targets,
		getFlow: async () => ({ flow, errors: [] }),
		setFlow: async (_id: string, next: Flow, label: string) => {
			saved.flow.push(label);
			return { flow: next, errors: [] };
		},
		getLayout: async () => layout,
		setLayout: async (_id: string, next: Layout, label: string) => {
			saved.layout.push(label);
			return next;
		},
		readScript: async () => '',
		writeScript: async () => undefined,
		testScript: async () => ({}),
		checkScriptSyntax: async () => [],
		runPreview: async () => preview,
		getPreview: async () => null,
		...noTerminal,
		checkProject: async () => [],
		history: async () => ({ entries: [], can_undo: false, can_redo: false }),
		undo: async () => ({ entries: [], can_undo: false, can_redo: false }),
		redo: async () => ({ entries: [], can_undo: false, can_redo: false }),
		mcpInfo: async () => null,
		mcpError: async () => null,
		onChange: async () => () => undefined,
		pickProject: async () => null,
	} as unknown as Api;
	return { api, saved };
}

async function boot() {
	const f = fake();
	const chrome = createChromeStore(browserSettings(undefined));
	await chrome.init();
	const root = document.createElement('div');
	document.body.append(root);
	let app!: App;
	await act(async () => {
		app = await startApp(root, chrome, { header: false, api: f.api });
	});
	const go = (screen: keyof typeof SCREEN_ACTIONS) =>
		act(async () => {
			await app.dispatch(SCREEN_ACTIONS[screen]);
		});
	return { ...f, app, chrome, root, go };
}

afterEach(() => {
	document.body.replaceChildren();
});

describe('the router', () => {
	it('shows each screen for its menu action, marks it in the sidebar and names it in the title', async () => {
		const { root, go, chrome } = await boot();
		const heading = () => root.querySelector('main h1')?.textContent;
		expect(heading()).toBe('P');
		for (const [screen, title, menu] of [
			['flow', 'Flow', ' · Flow'],
			['layouts', 'Layouts', ' · Layouts'],
			['script', 'Script', ' · Scripts'],
			['preview', 'Dry run', ' · Dry-run preview'],
			['project', 'P', ''],
		] as const) {
			await go(screen);
			expect(heading()).toBe(title);
			expect(chrome.get().menu.screen).toBe(screen);
			expect(chrome.get().title).toBe(`P${menu}`);
		}
		expect(root.querySelector('nav a[aria-current="page"]')?.textContent).toBe('Overview');
	});

	it('asks for a project while none is open', async () => {
		const f = fake();
		f.api.projectSummary = async () => {
			throw new Error('no project is open');
		};
		const chrome = createChromeStore(browserSettings(undefined));
		await chrome.init();
		const root = document.createElement('div');
		document.body.append(root);
		await act(async () => {
			await startApp(root, chrome, { header: false, api: f.api });
		});
		expect(root.querySelector('main h1')?.textContent).toBe('Open a project');
	});
});

describe('the Flow screen', () => {
	it('draws the graph, edits the selected step and saves with a history label', async () => {
		const { root, go, saved } = await boot();
		await go('flow');
		const node = root.querySelector<SVGGElement>('g[aria-label="page step welcome"]')!;
		expect(node).not.toBeNull();
		await act(async () => node.dispatchEvent(new MouseEvent('click', { bubbles: true })));
		const when = root.querySelector<HTMLInputElement>('aside label input')!;
		expect(when).not.toBeNull();
		await act(async () => {
			[...root.querySelectorAll('button')].find((b) => b.textContent === '+ action')!.click();
		});
		expect(saved.flow.at(-1)).toMatch(/^Add action step /);
	});
});

describe('the Layouts screen', () => {
	it('lists the nodes, previews the layout and saves a new child', async () => {
		const { root, go, saved } = await boot();
		await go('layouts');
		expect(root.querySelector('[role="tree"]')?.textContent).toContain('window · Welcome');
		expect(root.querySelector('[data-look]')?.getAttribute('data-look')).toBe('afterglow');
		await act(async () => {
			[...root.querySelectorAll('button')].find((b) => b.textContent === 'Add child')!.click();
		});
		expect(saved.layout.at(-1)).toBe('Add text to welcome');
	});
});

describe('the Dry-run screen', () => {
	it('runs a preview for the target, steps through its screens and lists the plan', async () => {
		const { root, go } = await boot();
		await go('preview');
		expect(root.textContent).toContain('Run a preview to see the plan');
		await act(async () => {
			[...root.querySelectorAll('button')].find((b) => b.textContent === 'Run preview')!.click();
		});
		expect(root.querySelector('[aria-label="Steps"]')?.textContent).toContain('welcome');
		expect(root.querySelector('[data-look="dos"]')).not.toBeNull();
		expect(root.textContent).toContain('Plan · 1 operations');
		await act(async () => {
			[...root.querySelectorAll('button')].find((b) => b.textContent === 'Next →')!.click();
		});
		expect(root.querySelector('strong')?.textContent).toBe('copy');
		expect(root.textContent).toContain('This step has no screen.');
	});
});
