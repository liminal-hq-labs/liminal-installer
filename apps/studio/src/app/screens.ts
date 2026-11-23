// Which path in the router's history each screen has, and a hook for the screen on show.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useRouterState } from '@tanstack/react-router';
import type { Screen } from '../chrome/menuModel';

/** Where each screen lives in the router's history. The window has no address bar, so these are internal. */
export const SCREEN_PATHS: Record<Screen, string> = {
	project: '/',
	flow: '/flow',
	layouts: '/layouts',
	script: '/scripts',
	preview: '/dry-run',
	tauri: '/tauri',
};

export const screenAt = (pathname: string): Screen =>
	(Object.entries(SCREEN_PATHS).find(([, path]) => path === pathname)?.[0] as Screen) ?? 'project';

/** The screen on show, for a component that draws differently by screen, such as the sidebar. */
export function useScreen(): Screen {
	return screenAt(useRouterState({ select: (s) => s.location.pathname }));
}
