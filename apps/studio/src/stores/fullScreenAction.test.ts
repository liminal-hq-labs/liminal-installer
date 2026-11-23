// Tests that View > Full-screen preview carries the step on show and how a terminal target is drawn into the window it opens.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createChromeStore } from '../chrome/chromeStore';
import type { Navigator } from './studio';
import { noTerminal, type Api } from '../lib/api';
import { browserSettings } from '../lib/settings';
import type { Preview } from '../lib/types';
import { createStudio } from './studio';

const memory = () => {
	const data = new Map<string, string>();
	return {
		getItem: (k: string) => data.get(k) ?? null,
		setItem: (k: string, v: string) => void data.set(k, v),
	};
};

function studio(openPreviewWindow?: Parameters<typeof createStudio>[0]['openPreviewWindow']) {
	const chrome = createChromeStore(browserSettings(memory()));
	const api = { ...noTerminal } as unknown as Api;
	const nav: Navigator = {
		go: async () => undefined,
		current: () => 'preview',
		subscribe: () => () => undefined,
	};
	return createStudio({ api, chrome, nav, openPreviewWindow });
}

describe('View > Full-screen preview', () => {
	it('opens the window on the target, the step and the view on show', async () => {
		const open = vi.fn(async () => undefined);
		const s = studio(open);
		s.preview.setState({
			previewTarget: 'dos-x86:tui-retro',
			previewStep: 3,
			previewView: 'terminal',
			previewScheme: 'dark',
			preview: { target: 'dos-x86:tui-retro' } as unknown as Preview,
		});
		await s.app().dispatch('view.fullScreenPreview');
		expect(open).toHaveBeenCalledWith('dos-x86:tui-retro', 3, 'terminal', 'dark');
	});

	it('opens in the rich view by default, and fills the page without a window', async () => {
		const open = vi.fn(async () => undefined);
		const s = studio(open);
		s.preview.setState({
			previewTarget: 'a:tui',
			preview: { target: 'a:tui' } as unknown as Preview,
		});
		await s.app().dispatch('view.fullScreenPreview');
		expect(open).toHaveBeenCalledWith('a:tui', 0, 'rich', 'system');
		const page = studio();
		page.preview.setState({
			previewTarget: 'a:tui',
			preview: { target: 'a:tui' } as unknown as Preview,
		});
		await page.app().dispatch('view.fullScreenPreview');
		expect(page.preview.getState().fullscreen).toBe(true);
	});
});
