// The window controls the chrome drives: the Tauri window in the desktop shell, a stand-in in a browser.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createTauriWindowControls } from '@liminal-hq/studio-chrome/TitleBar/tauriWindowControls';
import type { WindowControls } from '@liminal-hq/studio-chrome/TitleBar/windowControls';

export const inTauri = (): boolean =>
	typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** A window the page cannot control, for the browser demo: the buttons do nothing but the chrome still draws. */
export function createBrowserWindowControls(): WindowControls {
	const idle = () => Object.assign(() => undefined, {});
	return {
		minimize: () => undefined,
		toggleMaximize: () => undefined,
		close: () => void window.close(),
		setAlwaysOnTop: () => undefined,
		isMaximized: () => false,
		onMaximizedChange: idle,
	};
}

/** Created once: the chrome subscribes per identity. */
export function createWindowControls(): WindowControls {
	return inTauri() ? createTauriWindowControls() : createBrowserWindowControls();
}
