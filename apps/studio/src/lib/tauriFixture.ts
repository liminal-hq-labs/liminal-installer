// A Tauri view for tests and fakes: a configured, imported, up-to-date Plume Notes project.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TauriView } from './types';

/** The view, with `patch` laid over it. */
export function tauriViewFixture(patch: Partial<TauriView> = {}): TauriView {
	return {
		configured: true,
		config: 'src-tauri/tauri.conf.json',
		config_found: true,
		imported: true,
		stale: false,
		drift: [],
		pending: [],
		snapshot: null,
		windows: 'native',
		macos: 'native',
		linux: 'native',
		needs_wrap: false,
		identity: [
			{
				field: 'id',
				imported: 'app.plume.notes',
				project: 'app.plume.notes',
				overridden: false,
				differs: false,
			},
			{
				field: 'name',
				imported: 'Plume Notes',
				project: 'Plume Notes',
				overridden: false,
				differs: false,
			},
			{
				field: 'publisher',
				imported: 'Plume Labs',
				project: 'Plume Labs',
				overridden: false,
				differs: false,
			},
			{ field: 'version', imported: '1.4.2', project: '1.4.2', overridden: false, differs: false },
		],
		webview2: {
			imported: { mode: 'embedBootstrapper', silent: true, min_version: '110.0.1587.40' },
			override: null,
			effective: { mode: 'embedBootstrapper', silent: true, min_version: '110.0.1587.40' },
			modes: [
				{
					mode: 'downloadBootstrapper',
					size_cost_mb: 0,
					needs_network: true,
					carries: null,
					source: 'microsoft.com',
				},
				{
					mode: 'embedBootstrapper',
					size_cost_mb: 2,
					needs_network: true,
					carries: 'setup.exe',
					source: null,
				},
				{
					mode: 'offlineInstaller',
					size_cost_mb: 127,
					needs_network: false,
					carries: 'offline.exe',
					source: null,
				},
				{
					mode: 'fixedRuntime',
					size_cost_mb: 180,
					needs_network: false,
					carries: null,
					source: null,
				},
				{ mode: 'skip', size_cost_mb: 0, needs_network: false, carries: null, source: null },
			],
			if_missing: 'bootstrapper',
			if_present: 'found-checked',
		},
		updates: {
			configured: true,
			mode: 'both',
			feed: 'https://releases.example/feed.json',
			channel: 'stable',
			check_interval_hours: 24,
			installer_applies: true,
			app_applies: true,
			app_has_updater: true,
			endpoints: ['https://releases.example/{{target}}'],
		},
		problems: [],
		...patch,
	};
}
