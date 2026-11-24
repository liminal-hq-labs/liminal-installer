// Helpers for the Media screen's tests: the app mounted over a fake backend, and ways to find and press things.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act } from 'react';
import { vi } from 'vitest';
import { createChromeStore } from '../chrome/chromeStore';
import type { DialogResult, DialogSpec } from '../chrome/dialogWindow';
import { noMedia, noTerminal, type Api } from '../lib/api';
import { fakeMedia, type FakeMedia } from '../lib/fakeMedia';
import { browserSettings } from '../lib/settings';
import { tauriViewFixture } from '../lib/tauriFixture';
import type { MediaInfo, ProjectSummary } from '../lib/types';
import { startApp, type App } from './startApp';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const project: ProjectSummary = {
	root: '/a',
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
};

export interface MountOptions {
	/** What the confirmation window answers; a button id, or null for closed. Without it nothing asks. */
	answer?: string | null;
	/** Leave the media API out, as the desktop shell does until the planner exists. */
	unavailable?: boolean;
}

/** Mounts the app over a fake media backend that starts with `initial`, on the Media screen. */
export async function mountMedia(initial: MediaInfo[] = [], options: MountOptions = {}) {
	const media: FakeMedia = fakeMedia(initial);
	const asked: DialogSpec[] = [];
	const confirm = vi.fn(async (spec: DialogSpec): Promise<DialogResult> => {
		asked.push(spec);
		return options.answer === undefined ? 'remove' : options.answer;
	});
	const api = {
		openProject: async () => project,
		projectSummary: async () => project,
		getFlow: async () => ({ flow: { id: 'x', steps: [] }, errors: [] }),
		getLayout: async () => ({ id: 'x' }),
		getPreview: async () => null,
		...noTerminal,
		checkProject: async () => [],
		mcpInfo: async () => null,
		mcpError: async () => null,
		pickProject: async () => null,
		getTauri: async () => tauriViewFixture(),
		...(options.unavailable
			? { ...noMedia, history: media.history, onChange: media.onChange }
			: media),
	} as unknown as Api;
	const chrome = createChromeStore(browserSettings(undefined));
	await chrome.init();
	const root = document.createElement('div');
	document.body.append(root);
	let app!: App;
	await act(async () => {
		app = await startApp(root, chrome, {
			header: false,
			api,
			...(options.answer === undefined && !('answer' in options) ? {} : { confirm }),
		});
	});
	await act(async () => {
		await app.dispatch('view.media');
	});
	const click = (el: Element | null | undefined) => act(async () => (el as HTMLElement).click());
	const button = (name: string) =>
		[...root.querySelectorAll('button')].find((b) => b.textContent === name);
	const rows = () => [...root.querySelectorAll<HTMLElement>('[role="option"]')];
	const key = (el: Element | null, k: string, init: KeyboardEventInit = {}) =>
		act(async () => {
			el?.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, ...init }));
		});
	const settle = () => act(async () => undefined);
	const run = (action: () => Promise<unknown>) => act(async () => void (await action()));
	return { app, chrome, root, media, confirm, asked, click, button, rows, key, settle, act: run };
}

export type Mounted = Awaited<ReturnType<typeof mountMedia>>;
