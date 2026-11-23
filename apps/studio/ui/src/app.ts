// The app shell: state, actions and rendering of the active screen.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createApi, type Api } from './lib/api';
import { h, mount } from './lib/dom';
import { historyRows } from './lib/format';
import type {
	Flow,
	FlowView,
	HistoryView,
	Layout,
	McpInfo,
	Preview,
	Problem,
	ProjectSummary,
	ScriptReport,
} from './lib/types';
import { SCREEN_ACTIONS, type ActionId, type Screen } from './chrome/menuModel';
import type { ChromeStore } from './chrome/chromeStore';
import { scriptActions } from './lib/scriptActions';
import type { Settings } from './lib/settings';
import { layoutView } from './views/layouts';
import { flowView } from './views/flow';
import { previewView } from './views/preview';
import { projectView } from './views/project';
import { afterScriptRender, applyEditorSettings, scriptView } from './views/script';

export type Route = Screen;

export interface TestRun {
	status: 'running' | 'done' | 'error';
	target: string;
	/** The text that was run. */
	text: string;
	/** True when it differed from the saved file. */
	unsaved: boolean;
	at?: Date;
	ms?: number;
	report?: ScriptReport;
	error?: string;
}

export interface State {
	route: Route;
	summary: ProjectSummary | null;
	history: HistoryView;
	mcp: McpInfo | null;
	/** Why the agent server is off although it is switched on. */
	mcpError: string | null;
	problems: Problem[];
	flowId: string;
	flow: FlowView | null;
	selectedStep: string | null;
	connectFrom: string | null;
	layoutId: string;
	layout: Layout | null;
	layoutPath: number[];
	previewRenderer: string;
	overrideRenderer: string | null;
	scriptPath: string;
	scriptSource: string;
	/** The text in the editor. It equals `scriptSource` until there are unsaved edits, and `scriptDirty` is whether it does not. */
	scriptDraft: string;
	scriptDirty: boolean;
	/** The target a test runs against (`os-arch:renderer`), or empty for the sample machine. */
	scriptTarget: string;
	/** The last test run on this script, which goes out of date as the draft changes. */
	testRun: TestRun | null;
	/** The hooks and capabilities the script has, from the last report. */
	report: ScriptReport | null;
	previewTarget: string;
	preview: Preview | null;
	previewStep: number;
	showHistory: boolean;
	toast: { text: string; error: boolean } | null;
	error: string | null;
}

/** "2 errors, 1 warning": the counts the status bar shows, leaving out a zero. */
export function problemSummary(errors: number, warnings: number): string {
	const part = (n: number, one: string) => (n === 0 ? [] : [`${n} ${one}${n === 1 ? '' : 's'}`]);
	return [...part(errors, 'error'), ...part(warnings, 'warning')].join(', ');
}

export interface Ctx {
	api: Api;
	state: State;
	render(): void;
	go(route: Route): void;
	/** Runs an action, shows its error as a toast, and re-renders. */
	run(action: () => Promise<unknown>): Promise<boolean>;
	saveFlow(flow: Flow, label: string): Promise<void>;
	saveLayout(layout: Layout, label: string): Promise<void>;
	loadFlow(id: string): Promise<void>;
	loadLayout(id: string): Promise<void>;
	loadScript(path: string): Promise<void>;
	/** Opens another script, asking first when the open one has unsaved edits. */
	selectScript(path: string): Promise<void>;
	/** Saves the draft. */
	saveScript(): Promise<void>;
	/** Runs the draft against the chosen target. */
	testScript(): Promise<void>;
	/** The editor's text changed. */
	setDraft(text: string): void;
	discardDraft(): void;
	formatDraft(): void;
	/** Shows a message for a moment. */
	toast(text: string, error: boolean, ms?: number): void;
	runPreview(target: string): Promise<void>;
	openProject(): Promise<void>;
	/** Opens the project in a folder, and remembers it as a recent one. */
	openPath(path: string): Promise<void>;
	/** The settings in force. */
	settings(): Settings;
	/** Tells the window chrome what the menus and title need to know. */
	syncChrome(): void;
}

/** What the shell drives the app with. */
export interface App {
	/** Runs the action a menu item or shortcut names. Resolves once it is done, true when it succeeded; an error shows as a toast. */
	dispatch(id: ActionId): Promise<boolean>;
	/** Opens the project in a folder, as the Open recent menu does. Resolves true when it opened. */
	openPath(path: string): Promise<boolean>;
	/** Throws away unsaved script edits. */
	discard(): void;
}

const NAV: { route: Route; label: string; icon: string }[] = [
	{ route: 'project', label: 'Overview', icon: 'M2 4.5h4l1.5 1.5H14v6.5H2z' },
	{ route: 'flow', label: 'Flow', icon: 'M3 3h4v3H3zM9 10h4v3H9zM7 4.5h2.5V10' },
	{ route: 'layouts', label: 'Layouts', icon: 'M2 3h12v10H2zM2 6h12M6 6v7' },
	{ route: 'script', label: 'Scripts', icon: 'M5 4L2 8l3 4M11 4l3 4-3 4M9 3l-2 10' },
	{
		route: 'preview',
		label: 'Dry-run preview',
		icon: 'M8 3C4 3 2 8 2 8s2 5 6 5 6-5 6-5-2-5-6-5zM8 6a2 2 0 100 4 2 2 0 000-4z',
	},
];

const icon = (d: string) => {
	const ns = 'http://www.w3.org/2000/svg';
	const s = document.createElementNS(ns, 'svg');
	s.setAttribute('viewBox', '0 0 16 16');
	s.setAttribute('width', '16');
	s.setAttribute('height', '16');
	s.setAttribute('fill', 'none');
	s.setAttribute('stroke', 'currentColor');
	s.setAttribute('stroke-width', '1.5');
	s.setAttribute('stroke-linejoin', 'round');
	s.setAttribute('aria-hidden', 'true');
	const p = document.createElementNS(ns, 'path');
	p.setAttribute('d', d);
	s.append(p);
	return s;
};

export async function startApp(
	root: HTMLElement,
	chrome: ChromeStore,
	options: {
		header: boolean;
		/** The backend to use instead of the desktop shell or the demo. For tests. */ api?: Api;
		/** Settles unsaved script edits before they would be lost: true to go on. */ settleUnsaved?: () => Promise<boolean>;
	},
): Promise<App> {
	let api: Api;
	try {
		api = options.api ?? (await createApi());
	} catch (e) {
		mount(root, h('p', { class: 'fatal' }, String(e)));
		return { dispatch: async () => false, openPath: async () => false, discard: () => undefined };
	}
	const state: State = {
		route: 'project',
		summary: null,
		history: { entries: [], can_undo: false, can_redo: false },
		mcp: null,
		mcpError: null,
		problems: [],
		flowId: 'install',
		flow: null,
		selectedStep: null,
		connectFrom: null,
		layoutId: 'welcome',
		layout: null,
		layoutPath: [],
		previewRenderer: 'canvas',
		overrideRenderer: null,
		scriptPath: '',
		scriptSource: '',
		scriptDraft: '',
		scriptDirty: false,
		scriptTarget: '',
		testRun: null,
		report: null,
		previewTarget: '',
		preview: null,
		previewStep: 0,
		showHistory: false,
		toast: null,
		error: null,
	};
	let toastTimer: number | undefined;
	function showToast(text: string, error: boolean, ms: number): void {
		state.toast = { text, error };
		window.clearTimeout(toastTimer);
		toastTimer = window.setTimeout(() => {
			state.toast = null;
			render();
		}, ms);
	}

	const script = scriptActions({
		state,
		api,
		formatOnSave: () => chrome.get().settings.formatOnSave,
		render: () => render(),
		syncChrome: () => syncChrome(),
		toast: (text, error, ms = 3000) => showToast(text, error, ms),
		// Without a window to ask in, unsaved edits are discarded.
		guard:
			options.settleUnsaved ??
			(async () => {
				script.discardDraft();
				return true;
			}),
	});

	const ctx: Ctx = {
		api,
		state,
		render,
		syncChrome,
		toast: (text, error, ms = 3000) => showToast(text, error, ms),
		go(route) {
			state.route = route;
			state.connectFrom = null;
			void ctx.run(async () => {
				if (route === 'flow' && !state.flow && state.flowId) await ctx.loadFlow(state.flowId);
				if (route === 'layouts' && !state.layout && state.layoutId)
					await ctx.loadLayout(state.layoutId);
				if (route === 'script' && !state.scriptPath && state.summary?.scripts[0])
					await ctx.loadScript(state.summary.scripts[0]);
			});
		},
		async run(action) {
			let ok = true;
			try {
				await action();
			} catch (e) {
				ok = false;
				showToast(String(e instanceof Error ? e.message : e), true, 6000);
			}
			render();
			return ok;
		},
		async loadFlow(id) {
			state.flowId = id;
			state.flow = await api.getFlow(id);
			if (!state.flow.flow.steps.some((s) => s.id === state.selectedStep))
				state.selectedStep = null;
		},
		async saveFlow(flow, label) {
			state.flow = await api.setFlow(flow.id, flow, label);
		},
		async loadLayout(id) {
			state.layoutId = id;
			state.layout = await api.getLayout(id);
			state.layoutPath = [];
			state.overrideRenderer = null;
		},
		async saveLayout(layout, label) {
			state.layout = await api.setLayout(layout.id, layout, label);
		},
		...script,
		async runPreview(target) {
			state.previewTarget = target;
			state.preview = await api.runPreview(target);
			state.previewStep = Math.max(
				0,
				state.preview.steps.findIndex((s) => !s.skipped),
			);
		},
		async openProject() {
			const path = await api.pickProject(chrome.get().settings.projectsFolder || undefined);
			if (path) await ctx.openPath(path);
		},
		async openPath(path) {
			try {
				const previous = state.summary?.root;
				const opened = await api.openProject(path);
				// What belongs to the old project (selections, loaded documents, the preview) must not leak into the new one.
				if (opened.root !== previous) resetProjectState();
				state.summary = opened;
			} catch (e) {
				// A project that has moved or gone should not stay in the menu.
				await chrome.forgetProject(path).catch(() => undefined);
				throw e;
			}
			await refresh();
			await chrome.recordProject(state.summary.root).catch(() => undefined);
		},
		settings: () => chrome.get().settings,
	};

	/** The target a dry run starts on: the one the Preview setting names when the project has it, else the first. */
	function defaultTarget(ids: string[]): string {
		const wanted = chrome.get().settings.previewTarget;
		return ids.includes(wanted) ? wanted : (ids[0] ?? '');
	}

	/** Forgets everything tied to the project that was open: ids, selections, loaded documents, the preview and the check results. */
	function resetProjectState(): void {
		state.flowId = '';
		state.flow = null;
		state.selectedStep = null;
		state.connectFrom = null;
		state.layoutId = '';
		state.layout = null;
		state.layoutPath = [];
		state.overrideRenderer = null;
		script.clearScript();
		state.previewTarget = '';
		state.previewStep = 0;
		state.preview = null;
		state.problems = [];
		state.showHistory = false;
	}

	/** Reloads whatever the current project and screen show. A dirty script is kept. A selection the project no longer has falls back to its first. */
	async function refresh(): Promise<void> {
		state.summary = await api.projectSummary();
		const sum = state.summary;
		state.history = await api.history();
		state.problems = await api.checkProject();
		state.mcp = await api.mcpInfo();
		state.mcpError = await api.mcpError().catch(() => null);
		if (!sum.targets.some((t) => t.full_id === state.previewTarget))
			state.previewTarget = defaultTarget(sum.targets.map((t) => t.full_id));
		state.preview = await api.getPreview();
		if (state.preview) state.previewTarget = state.preview.target;
		if (!sum.flows.some((f) => f.id === state.flowId)) {
			state.flowId = (sum.flows.find((f) => f.id === 'install') ?? sum.flows[0])?.id ?? '';
			state.flow = null;
			state.selectedStep = null;
			state.connectFrom = null;
		}
		if (state.flowId) await ctx.loadFlow(state.flowId);
		if (!sum.layouts.includes(state.layoutId)) {
			state.layoutId = sum.layouts[0] ?? '';
			state.layout = null;
			state.layoutPath = [];
			state.overrideRenderer = null;
		}
		if (state.layoutId) await ctx.loadLayout(state.layoutId);
		if (state.scriptPath && !sum.scripts.includes(state.scriptPath)) {
			// The script is gone from this project. The Scripts screen opens the first one when it is next shown.
			script.clearScript();
		} else if (state.scriptPath && !state.scriptDirty) await ctx.loadScript(state.scriptPath);
	}

	await api.onChange((event) => {
		// Keep the layout selection when the change came from elsewhere.
		const keepPath = state.layoutPath;
		const keepOverride = state.overrideRenderer;
		void ctx.run(async () => {
			await refresh();
			if (event.doc?.type === 'layout' && event.doc.id === state.layoutId) {
				state.layoutPath = keepPath;
				state.overrideRenderer = keepOverride;
			}
			if (event.source === 'mcp') showToast(`Agent: ${event.label}`, false, 4000);
			if (
				chrome.get().settings.previewAutoRun &&
				(event.doc?.type === 'flow' || event.doc?.type === 'layout')
			)
				scheduleAutoRun();
		});
	});

	let autoRunTimer: number | undefined;
	/** Runs the dry run again shortly after the last flow or layout change, for the Preview setting. */
	function scheduleAutoRun(): void {
		window.clearTimeout(autoRunTimer);
		autoRunTimer = window.setTimeout(() => {
			if (!state.summary || !state.previewTarget || !chrome.get().settings.previewAutoRun) return;
			void ctx.run(() => ctx.runPreview(state.previewTarget));
		}, 250);
	}

	// Settings that change what is on screen take effect without a reload, and without losing unsaved script text.
	let seen = chrome.get().settings;
	chrome.subscribe(() => {
		const next = chrome.get().settings;
		const was = seen;
		seen = next;
		if (state.route === 'script') applyEditorSettings(ctx, next);
		if (next.mcpEnabled !== was.mcpEnabled || next.mcpPort !== was.mcpPort) {
			// Only the card changes, so text typed in the script editor stays as it is.
			void Promise.all([api.mcpInfo(), api.mcpError().catch(() => null)]).then(
				([info, why]) => {
					state.mcp = info;
					state.mcpError = why;
					root.querySelector('.agent-card')?.replaceWith(agentCard());
				},
				() => undefined,
			);
		}
		if (next.previewTarget !== was.previewTarget && state.summary) {
			const wanted = defaultTarget(state.summary.targets.map((t) => t.full_id));
			if (wanted !== state.previewTarget) state.previewTarget = wanted;
		}
		if (
			next.showSkippedSteps !== was.showSkippedSteps ||
			next.previewTarget !== was.previewTarget
		) {
			if (state.route === 'preview') render();
		}
	});

	function syncChrome(): void {
		chrome.setMenu({
			hasProject: state.summary !== null,
			canUndo: state.history.can_undo,
			canRedo: state.history.can_redo,
			screen: state.route,
			scriptDirty: state.scriptDirty && state.scriptPath !== '',
		});
		const screen =
			state.route === 'project'
				? ''
				: ` · ${NAV.find((n) => n.route === state.route)?.label ?? ''}`;
		chrome.setTitle(state.summary ? `${state.summary.name}${screen}` : '');
	}

	const actions: Partial<Record<ActionId, () => Promise<void>>> = {
		'file.open': () => ctx.openProject(),
		'file.reload': async () => {
			if (!state.summary) return;
			resetProjectState();
			state.summary = await api.openProject(state.summary.root);
			await refresh();
			showToast('Reloaded from disk', false, 2500);
		},
		'file.save': async () => {
			if (state.scriptPath && state.scriptDirty) await ctx.saveScript();
		},
		'edit.undo': async () => void (await api.undo()),
		'edit.redo': async () => void (await api.redo()),
		'project.check': async () => {
			state.problems = await api.checkProject();
			const n = state.problems.length;
			const errors = state.problems.filter((p) => p.severity === 'error').length;
			showToast(
				n === 0
					? 'Check passed: no problems'
					: `Check found ${n} problem${n === 1 ? '' : 's'}${errors ? `, ${errors} with errors` : ''}`,
				errors > 0,
				4000,
			);
		},
		'project.preview': async () => {
			await ctx.runPreview(state.previewTarget);
			ctx.go('preview');
		},
	};
	for (const [route, id] of Object.entries(SCREEN_ACTIONS) as [Route, ActionId][])
		actions[id] = async () => ctx.go(route);

	function agentCard(): HTMLElement {
		const failed = !state.mcp && state.mcpError;
		return h(
			'div',
			{ class: `agent-card${failed ? ' failed' : ''}` },
			h('span', { class: `dot ${state.mcp ? 'on' : ''}` }),
			state.mcp
				? 'Agent endpoint ready'
				: failed
					? 'Agent endpoint did not start'
					: 'Agent endpoint off',
			state.mcp ? h('code', {}, state.mcp.url) : null,
			failed ? h('span', { class: 'err small', role: 'alert' }, state.mcpError) : null,
			failed
				? h('span', { class: 'dim small' }, 'Change the port in Preferences, under Agents.')
				: null,
		);
	}

	function historyPopover(): HTMLElement {
		const rows = historyRows(state.history.entries);
		return h(
			'div',
			{ class: 'popover', role: 'dialog', 'aria-label': 'History' },
			rows.length === 0
				? h('p', { class: 'dim' }, 'No changes yet.')
				: h(
						'ol',
						{},
						rows.map((r) =>
							h(
								'li',
								{ class: r.undone ? 'undone' : '' },
								h('span', {}, r.label),
								h('span', { class: 'dim' }, ` · ${r.source} · ${r.doc}`),
							),
						),
					),
		);
	}

	/** Opens the Overview and brings its checks list into view. */
	function showChecks(): void {
		state.route = 'project';
		state.showHistory = false;
		render();
		const section = root.querySelector<HTMLElement>('#checks');
		section?.scrollIntoView?.({ block: 'start' });
		section?.focus?.({ preventScroll: true });
	}

	function render(): void {
		const s = state;
		const body = !s.summary
			? h(
					'section',
					{ class: 'empty' },
					h('h1', {}, 'Open a project'),
					h('p', {}, 'Choose a folder that holds installer.toml, or the installer folder itself.'),
					h(
						'button',
						{ class: 'btn primary', on: { click: () => void ctx.run(() => ctx.openProject()) } },
						'Open project…',
					),
				)
			: s.route === 'project'
				? projectView(ctx)
				: s.route === 'flow'
					? flowView(ctx)
					: s.route === 'layouts'
						? layoutView(ctx)
						: s.route === 'script'
							? scriptView(ctx)
							: previewView(ctx);
		const errors = s.problems.filter((p) => p.severity === 'error').length;
		const warnings = s.problems.length - errors;
		const applied = s.history.entries.filter((e) => e.applied).length;
		mount(
			root,
			h(
				'div',
				{ class: options.header ? 'shell with-header' : 'shell' },
				options.header
					? h(
							'header',
							{ class: 'topbar' },
							h('span', { class: 'brand' }, 'Liminal Installer Studio'),
							h('span', { class: 'project-name' }, s.summary?.name ?? 'No project'),
							h(
								'span',
								{ class: 'topbar-actions' },
								h(
									'button',
									{
										class: 'btn ghost',
										disabled: !s.history.can_undo,
										title: 'Undo',
										on: { click: () => actions['edit.undo'] && void ctx.run(actions['edit.undo']) },
									},
									'Undo',
								),
								h(
									'button',
									{
										class: 'btn ghost',
										disabled: !s.history.can_redo,
										title: 'Redo',
										on: { click: () => actions['edit.redo'] && void ctx.run(actions['edit.redo']) },
									},
									'Redo',
								),
								h(
									'button',
									{
										class: 'btn ghost',
										on: { click: () => void ctx.run(() => ctx.openProject()) },
									},
									'Open…',
								),
							),
						)
					: null,
				s.showHistory ? historyPopover() : null,
				h(
					'nav',
					{ class: 'sidebar', 'aria-label': 'Studio' },
					h('div', { class: 'nav-label' }, 'Project'),
					NAV.map((n) =>
						h(
							'a',
							{
								href: '#',
								class: s.route === n.route ? 'active' : '',
								'aria-current': s.route === n.route ? 'page' : undefined,
								on: {
									click: (e) => {
										e.preventDefault();
										ctx.go(n.route);
									},
								},
							},
							icon(n.icon),
							n.label,
						),
					),
					agentCard(),
				),
				h('main', { class: 'main' }, s.error ? h('p', { class: 'fatal' }, s.error) : body),
				h(
					'footer',
					{ class: 'statusbar' },
					h('span', { class: 'statusbar-path' }, s.summary?.root ?? 'No project open'),
					h(
						'span',
						{ class: 'statusbar-end' },
						s.problems.length === 0
							? h('span', {}, 'No problems')
							: h(
									'button',
									{
										class: `statusbar-btn ${errors > 0 ? 'err' : 'warn'}`,
										id: 'status-problems',
										title: 'Show the checks on the Overview',
										on: { click: () => showChecks() },
									},
									problemSummary(errors, warnings),
								),
						h(
							'button',
							{
								class: 'statusbar-btn',
								'aria-expanded': s.showHistory,
								on: {
									click: () => {
										s.showHistory = !s.showHistory;
										render();
									},
								},
							},
							`History (${applied})`,
						),
					),
				),
				s.toast
					? h(
							'div',
							{ class: `toast ${s.toast.error ? 'error' : ''}`, role: 'status' },
							s.toast.text,
						)
					: null,
			),
		);
		if (s.route === 'script') afterScriptRender(ctx);
		syncChrome();
	}

	render();
	await ctx.run(async () => {
		try {
			state.summary = await api.projectSummary();
		} catch {
			return;
		}
		await refresh();
	});
	if (state.summary) {
		// A project given on the command line counts as a recent one.
		await chrome.recordProject(state.summary.root).catch(() => undefined);
	} else {
		const last = chrome.get().settings.lastProject;
		if (chrome.get().settings.reopenLastProject && last) await ctx.run(() => ctx.openPath(last));
	}
	return {
		dispatch(id) {
			const action = actions[id];
			return action ? ctx.run(action) : Promise.resolve(false);
		},
		openPath: (path) => ctx.run(() => ctx.openPath(path)),
		discard: () => {
			ctx.discardDraft();
			render();
		},
	};
}
