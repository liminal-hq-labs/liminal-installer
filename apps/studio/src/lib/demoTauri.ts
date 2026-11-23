// A stand-in for the Tauri import in the browser demo: a believable Plume Notes project that is one import behind.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	InstallMode,
	Problem,
	TauriFieldChange,
	TauriImportResult,
	TauriIdentityRow,
	TauriModeInfo,
	TauriSettingsChange,
	TauriView,
	Updates,
	Webview2Mode,
	Webview2Override,
	Webview2Setting,
} from './types';

const CONFIG = 'src-tauri/tauri.conf.json';

/** What the demo's `tauri.conf.json` says now, which the project has not caught up with. */
const SOURCE = {
	id: 'app.plume.notes',
	name: 'Plume Notes',
	publisher: 'Plume Labs',
	version: '1.4.2',
};

const IMPORTED_WEBVIEW2: Webview2Setting = {
	mode: 'embedBootstrapper',
	silent: true,
	min_version: '110.0.1587.40',
};

const MODES: TauriModeInfo[] = [
	{
		mode: 'downloadBootstrapper',
		size_cost_mb: 0,
		needs_network: true,
		carries: null,
		source: 'go.microsoft.com/fwlink/p/?LinkId=2124703',
	},
	{
		mode: 'embedBootstrapper',
		size_cost_mb: 2,
		needs_network: true,
		carries: 'MicrosoftEdgeWebview2Setup.exe',
		source: 'go.microsoft.com/fwlink/p/?LinkId=2124703',
	},
	{
		mode: 'offlineInstaller',
		size_cost_mb: 127,
		needs_network: false,
		carries: 'MicrosoftEdgeWebView2RuntimeInstallerX64.exe',
		source: 'go.microsoft.com/fwlink/p/?LinkId=2124704',
	},
	{
		mode: 'fixedRuntime',
		size_cost_mb: 180,
		needs_network: false,
		carries: 'Microsoft.WebView2.FixedVersionRuntime',
		source: null,
	},
	{ mode: 'skip', size_cost_mb: 0, needs_network: false, carries: null, source: null },
];

const MISSING: Record<Webview2Mode, string> = {
	skip: 'skip',
	fixedRuntime: 'fixed',
	downloadBootstrapper: 'download',
	embedBootstrapper: 'bootstrapper',
	offlineInstaller: 'offline',
};

interface State {
	configured: boolean;
	/** The project has caught up with the config since the last import. */
	current: boolean;
	imported: boolean;
	project: Record<string, string>;
	overrides: string[];
	windows: InstallMode;
	webview2: Webview2Override | null;
	updates: Updates;
}

const initial = (): State => ({
	configured: true,
	current: false,
	imported: true,
	project: { ...SOURCE, version: '1.4.1' },
	overrides: [],
	windows: 'native',
	webview2: null,
	updates: { mode: 'liminal', feed: 'https://releases.plume.example/feed.json', channel: 'stable' },
});

/** The browser demo's Tauri state, with the same reads and writes as the backend's commands. */
export function demoTauri() {
	let state = initial();

	const pending = (): TauriFieldChange[] =>
		state.current
			? []
			: Object.entries(SOURCE)
					.filter(([k, v]) => state.project[k] !== v && !state.overrides.includes(k))
					.map(([k, v]) => ({ field: `product.${k}`, from: state.project[k] ?? null, to: v }));

	const effective = (): Webview2Setting => {
		const o = state.webview2;
		return {
			mode: o?.mode ?? IMPORTED_WEBVIEW2.mode,
			silent: o?.silent ?? IMPORTED_WEBVIEW2.silent,
			path: o?.path ?? null,
			min_version: o?.min_version ?? IMPORTED_WEBVIEW2.min_version ?? null,
		};
	};

	const view = (): TauriView => {
		const rows: TauriIdentityRow[] = Object.entries(SOURCE).map(([field, imported]) => {
			const project = state.configured ? (state.project[field] ?? null) : null;
			return {
				field,
				imported: state.imported ? imported : null,
				project,
				overridden: state.overrides.includes(field),
				differs: state.imported && project !== imported,
			};
		});
		const stale = state.configured && !state.current;
		const eff = effective();
		const problems: Problem[] = [];
		if (state.updates.mode === 'liminal')
			problems.push({
				severity: 'warning',
				scope: 'updates',
				message: 'the app has its own updater, so the installed app would have two updaters',
			});
		if (stale)
			problems.push({
				severity: 'warning',
				scope: 'tauri',
				message: `${CONFIG} changed since the last import`,
			});
		return {
			configured: state.configured,
			config: state.configured ? CONFIG : null,
			config_found: true,
			imported: state.imported,
			stale,
			drift: stale ? [{ file: CONFIG, kind: 'changed' }] : [],
			pending: pending(),
			snapshot: null,
			windows: state.windows,
			macos: 'native',
			linux: 'native',
			needs_wrap: false,
			identity: rows,
			webview2: state.imported
				? {
						imported: IMPORTED_WEBVIEW2,
						override: state.webview2,
						effective: eff,
						modes: MODES,
						if_missing: MISSING[eff.mode],
						if_present:
							eff.mode === 'skip'
								? 'skip'
								: eff.mode === 'fixedRuntime'
									? 'fixed'
									: eff.min_version
										? 'found-checked'
										: 'found',
					}
				: null,
			updates: {
				configured: true,
				mode: state.updates.mode,
				feed: state.updates.feed ?? null,
				channel: state.updates.channel ?? null,
				check_interval_hours: state.updates.check_interval_hours ?? null,
				installer_applies: state.updates.mode === 'liminal' || state.updates.mode === 'both',
				app_applies: state.updates.mode === 'tauri' || state.updates.mode === 'both',
				app_has_updater: true,
				endpoints: ['https://releases.plume.example/{{target}}/{{current_version}}'],
			},
			problems,
		};
	};

	return {
		view,
		/** The state as text, for the undo history. */
		snapshot: (): string => JSON.stringify(state),
		restore: (text: string | undefined): void => {
			state = text ? (JSON.parse(text) as State) : initial();
		},
		importConfig: (): TauriImportResult => {
			const changes = pending();
			const first = !state.imported || !state.configured;
			const kept = state.overrides.filter(
				(k) => state.project[k] !== SOURCE[k as keyof typeof SOURCE],
			);
			for (const [k, v] of Object.entries(SOURCE)) {
				if (!state.overrides.includes(k)) state.project[k] = v;
			}
			state = { ...state, configured: true, imported: true, current: true };
			return {
				view: view(),
				first_import: first,
				changes,
				filled: first ? Object.keys(SOURCE).map((k) => `product.${k}`) : [],
				kept,
				components: first ? ['main'] : [],
				warnings: first ? ['bundle.resources was imported as a payload rule'] : [],
			};
		},
		set: (change: TauriSettingsChange): TauriView => {
			if (change.windows) state.windows = change.windows;
			if (change.overrides) state.overrides = change.overrides;
			if (change.webview2) {
				const empty = Object.values(change.webview2).every((v) => v === null || v === undefined);
				state.webview2 = empty ? null : change.webview2;
			}
			if (change.updates) state.updates = change.updates;
			return view();
		},
	};
}
