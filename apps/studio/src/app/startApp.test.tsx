// @vitest-environment happy-dom
// Tests the app shell against a fake backend: opening another project, error toasts, awaited actions and the status bar.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { problemSummary } from '../lib/format';
import { startApp, type App } from './startApp';
import { createChromeStore } from '../chrome/chromeStore';
import { StudioError, noTerminal, type Api } from '../lib/api';
import { browserSettings } from '../lib/settings';
import { tauriViewFixture } from '../lib/tauriFixture';
import type {
	ChangeEvent,
	Flow,
	HistoryEntry,
	HistoryView,
	Problem,
	ProjectSummary,
} from '../lib/types';

const flowOf = (id: string): Flow => ({
	version: 1,
	id,
	start: 'a',
	steps: [{ id: 'a', type: 'finish' }],
});

function summary(
	root: string,
	flows: string[],
	layouts: string[],
	scripts: string[] = [],
): ProjectSummary {
	return {
		root,
		id: `test.${root}`,
		name: root,
		publisher: 'Test',
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
		flows: flows.map((id) => ({ id, steps: 1, errors: 0 })),
		layouts,
		scripts,
	};
}

const projects: Record<string, ProjectSummary> = {
	'/a': summary('/a', ['install', 'extra'], ['welcome', 'other'], ['scripts/a.lua']),
	'/b': summary('/b', ['setup'], ['intro']),
};

// React reports state changes outside `act` when this is set; the helpers below settle inside it.
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

interface Fake {
	api: Api;
	calls: string[];
	problems: Problem[];
	entries: HistoryEntry[];
	emit(e: ChangeEvent): void;
}

function fakeApi(overrides: Partial<Api> = {}): Fake {
	let current: ProjectSummary | null = null;
	const calls: string[] = [];
	const listeners = new Set<(e: ChangeEvent) => void>();
	const fake: Fake = {
		calls,
		problems: [],
		entries: [],
		emit: (e) => listeners.forEach((l) => l(e)),
		api: undefined as unknown as Api,
	};
	const view = (): HistoryView => ({ entries: fake.entries, can_undo: true, can_redo: false });
	const need = () => {
		if (!current) throw new Error('no project is open');
		return current;
	};
	fake.api = {
		openProject: async (path) => {
			current = projects[path] ?? null;
			if (!current) throw new Error(`no project at ${path}`);
			return current;
		},
		projectSummary: async () => need(),
		listTargets: async () => need().targets,
		getFlow: async (id) => {
			calls.push(`getFlow:${id}`);
			return { flow: flowOf(id), errors: [] };
		},
		setFlow: async (_id, flow) => ({ flow, errors: [] }),
		getLayout: async (id) => {
			calls.push(`getLayout:${id}`);
			return { version: 1, id, root: { kind: 'window' } };
		},
		setLayout: async (_id, layout) => layout,
		readScript: async () => '',
		writeScript: async () => undefined,
		testScript: async () => ({
			ok: true,
			hooks: [],
			capabilities: [],
			diagnostics: [],
			output: [],
			target: null,
			tests: [],
		}),
		checkScriptSyntax: async () => [],
		runPreview: async () => {
			throw new Error('not used');
		},
		getPreview: async () => null,
		...noTerminal,
		checkProject: async () => fake.problems,
		history: async () => view(),
		undo: async () => view(),
		redo: async () => view(),
		mcpInfo: async () => null,
		mcpError: async () => null,
		onChange: async (handler) => {
			listeners.add(handler);
			return () => listeners.delete(handler);
		},
		pickProject: async () => null,
		getTauri: async () => tauriViewFixture(),
		importTauri: async () => {
			throw new Error('not used');
		},
		setTauriSettings: async () => tauriViewFixture(),
		pickTauriConfig: async () => null,
		...overrides,
	};
	return fake;
}

const memoryStorage = () => {
	const data = new Map<string, string>();
	return {
		getItem: (k: string) => data.get(k) ?? null,
		setItem: (k: string, v: string) => void data.set(k, v),
		removeItem: (k: string) => void data.delete(k),
	};
};

async function boot(fake: Fake) {
	const chrome = createChromeStore(browserSettings(memoryStorage()));
	await chrome.init();
	const root = document.createElement('div');
	document.body.append(root);
	let started!: App;
	await act(async () => {
		started = await startApp(root, chrome, { header: true, api: fake.api });
	});
	// Each call settles React inside `act`, so the page is up to date when it resolves.
	const settled = async (run: () => Promise<boolean>): Promise<boolean> => {
		let result = false;
		await act(async () => {
			result = await run();
		});
		return result;
	};
	const app: App = {
		dispatch: (id) => settled(() => started.dispatch(id)),
		openPath: (path) => settled(() => started.openPath(path)),
		discard: () => void act(async () => started.discard()),
	};
	return { app, root, chrome };
}

afterEach(() => {
	document.body.replaceChildren();
});

describe('opening another project', () => {
	it("drops the old selections and falls back to the new project's first flow and layout", async () => {
		const fake = fakeApi();
		const { app, root } = await boot(fake);
		await app.openPath('/a');
		expect(fake.calls).toContain('getFlow:install');
		expect(fake.calls).toContain('getLayout:welcome');

		// Choose other documents in A, as a person would from the Overview.
		await app.dispatch('view.flow');
		fake.calls.length = 0;
		await app.openPath('/b');

		expect(fake.calls).toContain('getFlow:setup');
		expect(fake.calls).toContain('getLayout:intro');
		expect(fake.calls.filter((c) => c === 'getFlow:install' || c === 'getLayout:welcome')).toEqual(
			[],
		);
		expect(root.querySelector('[data-testid="status-path"]')?.textContent).toBe('/b');
	});

	it('shows a project that fails to open as a toast and keeps the one that was open', async () => {
		const fake = fakeApi();
		const { app, root } = await boot(fake);
		await app.openPath('/a');
		expect(await app.openPath('/missing')).toBe(false);
		expect(root.querySelector('[data-testid="toast"][data-error="true"]')?.textContent).toContain(
			'no project at /missing',
		);
		expect(root.querySelector('[data-testid="status-path"]')?.textContent).toBe('/a');
	});
});

describe('a project that cannot be opened', () => {
	const failing = (error: Error) =>
		fakeApi({
			openProject: async () => {
				throw error;
			},
		});

	it('is dropped from the recent list when it is missing, and shows only the message', async () => {
		const { app, root, chrome } = await boot(
			failing(new StudioError('not_found', 'no project at /gone')),
		);
		await chrome.recordProject('/gone');
		expect(chrome.get().settings.recentProjects).toContain('/gone');
		expect(await app.openPath('/gone')).toBe(false);
		expect(root.querySelector('[data-testid="toast"][data-error="true"]')?.textContent).toBe(
			'no project at /gone',
		);
		expect(chrome.get().settings.recentProjects).not.toContain('/gone');
	});

	it('stays in the recent list when it is there but invalid', async () => {
		const { app, root, chrome } = await boot(
			failing(new StudioError('invalid_input', 'invalid installer.toml: bad')),
		);
		await chrome.recordProject('/broken');
		expect(await app.openPath('/broken')).toBe(false);
		expect(root.querySelector('[data-testid="toast"][data-error="true"]')?.textContent).toBe(
			'invalid installer.toml: bad',
		);
		expect(chrome.get().settings.recentProjects).toContain('/broken');
	});
});

describe('running actions', () => {
	it('toasts an error from an action and reports failure', async () => {
		const fake = fakeApi({
			undo: async () => {
				throw new Error('nothing to undo');
			},
		});
		const { app, root } = await boot(fake);
		await app.openPath('/a');
		expect(await app.dispatch('edit.undo')).toBe(false);
		const toast = root.querySelector('[data-testid="toast"][data-error="true"]');
		expect(toast?.textContent).toBe('nothing to undo');
		expect(toast?.getAttribute('role')).toBe('status');
	});

	it('resolves dispatch only when the action has finished', async () => {
		let release!: () => void;
		const gate = new Promise<void>((resolve) => (release = resolve));
		const fake = fakeApi();
		const view = await fake.api.history();
		fake.api.undo = async () => {
			await gate;
			return view;
		};
		const { app } = await boot(fake);
		await app.openPath('/a');
		const done = vi.fn();
		const pending = app.dispatch('edit.undo').then((ok) => done(ok));
		await Promise.resolve();
		await Promise.resolve();
		expect(done).not.toHaveBeenCalled();
		release();
		await pending;
		expect(done).toHaveBeenCalledWith(true);
	});

	it('reports false for an action the app does not run', async () => {
		const { app } = await boot(fakeApi());
		expect(await app.dispatch('file.quit')).toBe(false);
	});
});

describe('the status bar', () => {
	const problem = (severity: Problem['severity'], n: number): Problem => ({
		severity,
		scope: 'flow:install',
		message: `finding ${n}`,
	});
	const entry = (index: number, applied: boolean): HistoryEntry => ({
		index,
		label: `edit ${index}`,
		source: 'ui',
		doc: { type: 'flow', id: 'install' },
		applied,
	});

	it('says there are no problems only when there are no findings at all', async () => {
		const fake = fakeApi();
		const { app, root } = await boot(fake);
		await app.openPath('/a');
		expect(root.querySelector('[data-testid="status-end"]')?.textContent).toContain('No problems');
	});

	it('counts errors and warnings, and a click opens the checks on the Overview', async () => {
		const fake = fakeApi();
		fake.problems = [problem('warning', 1), problem('warning', 2), problem('error', 3)];
		const { app, root } = await boot(fake);
		await app.openPath('/a');
		await app.dispatch('view.flow');
		const button = root.querySelector<HTMLButtonElement>('#status-problems')!;
		expect(button.textContent).toBe('1 error, 2 warnings');
		expect(root.querySelector('[data-testid="status-end"]')?.textContent).not.toContain(
			'No problems',
		);
		await act(async () => button.click());
		expect(root.querySelector('#checks')).not.toBeNull();
		expect(root.querySelector('#checks')?.textContent).toContain('finding 3');
	});

	it('shows warnings alone as a count, not as no problems', async () => {
		const fake = fakeApi();
		fake.problems = [problem('warning', 1)];
		const { app, root } = await boot(fake);
		await app.openPath('/a');
		expect(root.querySelector('#status-problems')?.textContent).toBe('1 warning');
	});

	it('leaves undone entries out of the History count', async () => {
		const fake = fakeApi();
		fake.entries = [entry(0, true), entry(1, true), entry(2, false)];
		const { app, root } = await boot(fake);
		await app.openPath('/a');
		expect(root.querySelector('[data-testid="status-end"]')?.textContent).toContain('History (2)');
	});

	it('words the counts', () => {
		expect(problemSummary(1, 0)).toBe('1 error');
		expect(problemSummary(0, 3)).toBe('3 warnings');
		expect(problemSummary(2, 1)).toBe('2 errors, 1 warning');
	});
});

describe('the agent card', () => {
	it('says why the server did not start', async () => {
		const fake = fakeApi({ mcpError: async () => 'port 4000 is in use' });
		const { app, root } = await boot(fake);
		await app.openPath('/a');
		const card = root.querySelector('[data-testid="agent-card"]');
		expect(card?.textContent).toContain('did not start');
		expect(card?.textContent).toContain('port 4000 is in use');
	});
});
