// Mounts the app into an element and hands back what a shell drives it with.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import type { ChromeStore } from '../chrome/chromeStore';
import { createApi, errorMessage, type Api } from '../lib/api';
import type { Scheme } from '../preview/model';
import type { TerminalView } from '../preview/TerminalPreview';
import { createStudio, type App } from '../stores/studio';
import { createStudioRouter, routerNavigator } from './router';
import { ShellOptionsContext } from './ShellOptions';
import { StudioProvider } from './StudioContext';
import text from '../ui/text.module.css';

export type { App };

export interface StartOptions {
	/** The page draws its own header: a browser tab, where there is no title bar. */
	header: boolean;
	/** The backend to use instead of the desktop shell or the demo. For tests. */
	api?: Api;
	/** Settles unsaved script edits before they would be lost: true to go on. */
	settleUnsaved?: () => Promise<boolean>;
	/** Opens the full-screen preview in a window of its own. */
	openPreviewWindow?: (
		target: string,
		step: number,
		view: TerminalView,
		scheme: Scheme,
	) => Promise<void>;
}

/**
 * Draws the app into `root` and resolves once the first project (if any) is loaded. The studio
 * takes its backend as `api`, so a test hands in a fake one; the shell drives it through the
 * returned `App`.
 */
export async function startApp(
	root: HTMLElement,
	chrome: ChromeStore,
	options: StartOptions,
): Promise<App> {
	const reactRoot = createRoot(root);
	let api: Api;
	try {
		api = options.api ?? (await createApi());
	} catch (e) {
		reactRoot.render(<p className={text.fatal}>{errorMessage(e)}</p>);
		return { dispatch: async () => false, openPath: async () => false, discard: () => undefined };
	}
	const router = createStudioRouter();
	const studio = createStudio({
		api,
		chrome,
		nav: routerNavigator(router),
		...(options.settleUnsaved ? { settleUnsaved: options.settleUnsaved } : {}),
		...(options.openPreviewWindow ? { openPreviewWindow: options.openPreviewWindow } : {}),
	});
	reactRoot.render(
		<StrictMode>
			<ShellOptionsContext.Provider value={{ header: options.header }}>
				<StudioProvider value={studio}>
					<RouterProvider router={router} />
				</StudioProvider>
			</ShellOptionsContext.Provider>
		</StrictMode>,
	);
	await router.load();
	await studio.start();
	return studio.app();
}
