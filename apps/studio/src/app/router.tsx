// The router for the main window's seven screens, over an in-memory history.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	createMemoryHistory,
	createRootRoute,
	createRoute,
	createRouter,
} from '@tanstack/react-router';
import { FlowScreen } from '../screens/FlowScreen';
import { LayoutsScreen } from '../screens/LayoutsScreen';
import { MediaScreen } from '../screens/MediaScreen';
import { PreviewScreen } from '../screens/PreviewScreen';
import { ProjectScreen } from '../screens/ProjectScreen';
import { ScriptScreen } from '../screens/ScriptScreen';
import { TauriScreen } from '../screens/TauriScreen';
import type { Navigator } from '../stores/studio';
import { Shell } from './Shell';
import { SCREEN_PATHS, screenAt } from './screens';

const rootRoute = createRootRoute({ component: Shell });

const routeTree = rootRoute.addChildren([
	createRoute({
		getParentRoute: () => rootRoute,
		path: SCREEN_PATHS.project,
		component: ProjectScreen,
	}),
	createRoute({ getParentRoute: () => rootRoute, path: SCREEN_PATHS.flow, component: FlowScreen }),
	createRoute({
		getParentRoute: () => rootRoute,
		path: SCREEN_PATHS.layouts,
		component: LayoutsScreen,
	}),
	createRoute({
		getParentRoute: () => rootRoute,
		path: SCREEN_PATHS.script,
		component: ScriptScreen,
	}),
	createRoute({
		getParentRoute: () => rootRoute,
		path: SCREEN_PATHS.preview,
		component: PreviewScreen,
	}),
	createRoute({
		getParentRoute: () => rootRoute,
		path: SCREEN_PATHS.tauri,
		component: TauriScreen,
	}),
	createRoute({
		getParentRoute: () => rootRoute,
		path: SCREEN_PATHS.media,
		component: MediaScreen,
	}),
]);

export function createStudioRouter() {
	return createRouter({
		routeTree,
		history: createMemoryHistory({ initialEntries: [SCREEN_PATHS.project] }),
	});
}

export type StudioRouter = ReturnType<typeof createStudioRouter>;

/** The studio's view of the router: go to a screen, ask which one is showing, and hear of changes. */
export function routerNavigator(router: StudioRouter): Navigator {
	return {
		go: async (screen) => {
			await router.navigate({ to: SCREEN_PATHS[screen] });
		},
		current: () => screenAt(router.state.location.pathname),
		subscribe: (listener) => router.subscribe('onResolved', listener),
	};
}
