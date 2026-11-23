// The app shell: state, actions and rendering of the active screen.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createApi, type Api } from './lib/api';
import { h, mount } from './lib/dom';
import { historyRows, isRedo, isUndo } from './lib/format';
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
import { layoutView } from './views/layouts';
import { flowView } from './views/flow';
import { previewView } from './views/preview';
import { projectView } from './views/project';
import { scriptView } from './views/script';

export type Route = 'project' | 'flow' | 'layouts' | 'script' | 'preview';

export interface State {
	route: Route;
	summary: ProjectSummary | null;
	history: HistoryView;
	mcp: McpInfo | null;
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
	scriptDirty: boolean;
	report: ScriptReport | null;
	previewTarget: string;
	preview: Preview | null;
	previewStep: number;
	showHistory: boolean;
	toast: { text: string; error: boolean } | null;
	error: string | null;
}

export interface Ctx {
	api: Api;
	state: State;
	render(): void;
	go(route: Route): void;
	/** Runs an action, shows its error as a toast, and re-renders. */
	run(action: () => Promise<unknown>): Promise<void>;
	saveFlow(flow: Flow, label: string): Promise<void>;
	saveLayout(layout: Layout, label: string): Promise<void>;
	loadFlow(id: string): Promise<void>;
	loadLayout(id: string): Promise<void>;
	loadScript(path: string): Promise<void>;
	saveScript(source: string): Promise<void>;
	testScript(source?: string): Promise<void>;
	runPreview(target: string): Promise<void>;
	openProject(): Promise<void>;
}

const NAV: { route: Route; label: string; icon: string }[] = [
	{ route: 'project', label: 'Project', icon: 'M2 4.5h4l1.5 1.5H14v6.5H2z' },
	{ route: 'flow', label: 'Flow', icon: 'M3 3h4v3H3zM9 10h4v3H9zM7 4.5h2.5V10' },
	{ route: 'layouts', label: 'Layouts', icon: 'M2 3h12v10H2zM2 6h12M6 6v7' },
	{ route: 'script', label: 'Script', icon: 'M5 4L2 8l3 4M11 4l3 4-3 4M9 3l-2 10' },
	{ route: 'preview', label: 'Dry run', icon: 'M8 3C4 3 2 8 2 8s2 5 6 5 6-5 6-5-2-5-6-5zM8 6a2 2 0 100 4 2 2 0 000-4z' },
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

export async function startApp(root: HTMLElement): Promise<void> {
	let api: Api;
	try {
		api = await createApi();
	} catch (e) {
		mount(root, h('p', { class: 'fatal' }, String(e)));
		return;
	}
	const state: State = {
		route: 'project',
		summary: null,
		history: { entries: [], can_undo: false, can_redo: false },
		mcp: null,
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
		scriptDirty: false,
		report: null,
		previewTarget: '',
		preview: null,
		previewStep: 0,
		showHistory: false,
		toast: null,
		error: null,
	};
	let toastTimer: number | undefined;

	const ctx: Ctx = {
		api,
		state,
		render,
		go(route) {
			state.route = route;
			state.connectFrom = null;
			void ctx.run(async () => {
				if (route === 'flow' && !state.flow) await ctx.loadFlow(state.flowId);
				if (route === 'layouts' && !state.layout) await ctx.loadLayout(state.layoutId);
				if (route === 'script' && !state.scriptPath && state.summary?.scripts[0]) await ctx.loadScript(state.summary.scripts[0]);
			});
		},
		async run(action) {
			try {
				await action();
			} catch (e) {
				state.toast = { text: String(e instanceof Error ? e.message : e), error: true };
				window.clearTimeout(toastTimer);
				toastTimer = window.setTimeout(() => {
					state.toast = null;
					render();
				}, 6000);
			}
			render();
		},
		async loadFlow(id) {
			state.flowId = id;
			state.flow = await api.getFlow(id);
			if (!state.flow.flow.steps.some((s) => s.id === state.selectedStep)) state.selectedStep = null;
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
		async loadScript(path) {
			state.scriptPath = path;
			state.scriptSource = await api.readScript(path);
			state.scriptDirty = false;
			state.report = await api.testScript(path);
		},
		async saveScript(source) {
			await api.writeScript(state.scriptPath, source, `Edit ${state.scriptPath}`);
			state.scriptSource = source;
			state.scriptDirty = false;
			state.report = await api.testScript(state.scriptPath);
		},
		async testScript(source) {
			state.report = await api.testScript(state.scriptPath, source);
		},
		async runPreview(target) {
			state.previewTarget = target;
			state.preview = await api.runPreview(target);
			state.previewStep = Math.max(0, state.preview.steps.findIndex((s) => !s.skipped));
		},
		async openProject() {
			const path = await api.pickProject();
			if (path) {
				state.summary = await api.openProject(path);
				await refresh();
			}
		},
	};

	/** Reloads whatever the current project and screen show. A dirty script is kept. */
	async function refresh(): Promise<void> {
		state.summary = await api.projectSummary();
		state.history = await api.history();
		state.problems = await api.checkProject();
		state.mcp = await api.mcpInfo();
		if (!state.previewTarget) state.previewTarget = state.summary.targets[0]?.full_id ?? '';
		state.preview = await api.getPreview();
		if (state.summary.flows.some((f) => f.id === state.flowId)) await ctx.loadFlow(state.flowId);
		if (state.summary.layouts.includes(state.layoutId)) await ctx.loadLayout(state.layoutId).then(() => undefined);
		if (state.scriptPath && !state.scriptDirty && state.summary.scripts.includes(state.scriptPath)) await ctx.loadScript(state.scriptPath);
	}

	await api.onChange((event) => {
		// Keep the layout selection when the change came from elsewhere.
		const keepPath = state.layoutPath;
		const keepOverride = state.overrideRenderer;
		void ctx
			.run(async () => {
				await refresh();
				if (event.doc?.type === 'layout' && event.doc.id === state.layoutId) {
					state.layoutPath = keepPath;
					state.overrideRenderer = keepOverride;
				}
				if (event.source === 'mcp') {
					state.toast = { text: `Agent: ${event.label}`, error: false };
					window.clearTimeout(toastTimer);
					toastTimer = window.setTimeout(() => {
						state.toast = null;
						render();
					}, 4000);
				}
			});
	});

	window.addEventListener('keydown', (e) => {
		const target = e.target as HTMLElement | null;
		if (target && (target.tagName === 'TEXTAREA' || target.tagName === 'INPUT' || target.tagName === 'SELECT')) return;
		if (isUndo(e)) {
			e.preventDefault();
			if (state.history.can_undo) void ctx.run(async () => void (await api.undo()));
		} else if (isRedo(e)) {
			e.preventDefault();
			if (state.history.can_redo) void ctx.run(async () => void (await api.redo()));
		}
	});

	function render(): void {
		const s = state;
		const body = !s.summary
			? h('section', { class: 'empty' }, h('h1', {}, 'Open a project'), h('p', {}, 'Choose a folder that holds installer.toml, or the installer folder itself.'), h('button', { class: 'btn primary', on: { click: () => void ctx.run(() => ctx.openProject()) } }, 'Open project…'))
			: s.route === 'project'
				? projectView(ctx)
				: s.route === 'flow'
					? flowView(ctx)
					: s.route === 'layouts'
						? layoutView(ctx)
						: s.route === 'script'
							? scriptView(ctx)
							: previewView(ctx);
		const undo = () => void ctx.run(async () => void (await api.undo()));
		const redo = () => void ctx.run(async () => void (await api.redo()));
		const errors = s.problems.filter((p) => p.severity === 'error').length;
		const lastApplied = [...s.history.entries].reverse().find((e) => e.applied);
		mount(
			root,
			h(
				'div',
				{ class: 'shell' },
				h(
					'header',
					{ class: 'topbar' },
					h('span', { class: 'brand' }, 'Liminal Installer Studio'),
					h('span', { class: 'project-name' }, s.summary?.name ?? 'No project'),
					h(
						'span',
						{ class: 'topbar-actions' },
						h('button', { class: 'btn ghost', disabled: !s.history.can_undo, title: lastApplied ? `Undo: ${lastApplied.label}` : 'Nothing to undo', on: { click: undo } }, 'Undo'),
						h('button', { class: 'btn ghost', disabled: !s.history.can_redo, title: 'Redo', on: { click: redo } }, 'Redo'),
						h('button', { class: 'btn ghost', 'aria-expanded': s.showHistory, on: { click: () => { s.showHistory = !s.showHistory; render(); } } }, `History (${s.history.entries.length})`),
						h('button', { class: 'btn ghost', on: { click: () => void ctx.run(() => ctx.openProject()) } }, 'Open…'),
					),
				),
				s.showHistory
					? h('div', { class: 'popover', role: 'dialog', 'aria-label': 'History' },
							historyRows(s.history.entries).length === 0
								? h('p', { class: 'dim' }, 'No changes yet.')
								: h('ol', {}, historyRows(s.history.entries).map((r) => h('li', { class: r.undone ? 'undone' : '' }, h('span', {}, r.label), h('span', { class: 'dim' }, ` · ${r.source} · ${r.doc}`)))))
					: null,
				h(
					'nav',
					{ class: 'sidebar', 'aria-label': 'Studio' },
					NAV.map((n) =>
						h('a', { href: '#', class: s.route === n.route ? 'active' : '', 'aria-current': s.route === n.route ? 'page' : undefined, on: { click: (e) => { e.preventDefault(); ctx.go(n.route); } } }, icon(n.icon), n.label),
					),
					h('div', { class: 'agent-card' }, h('span', { class: `dot ${s.mcp ? 'on' : ''}` }), s.mcp ? 'Agent endpoint ready' : 'Agent endpoint off', s.mcp ? h('code', {}, s.mcp.url) : null),
				),
				h('main', { class: 'main' }, s.error ? h('p', { class: 'fatal' }, s.error) : body),
				h('footer', { class: 'statusbar' }, h('span', {}, s.summary?.root ?? ''), h('span', {}, errors === 0 ? 'No problems' : `${errors} problem${errors === 1 ? '' : 's'}`)),
				s.toast ? h('div', { class: `toast ${s.toast.error ? 'error' : ''}`, role: 'status' }, s.toast.text) : null,
			),
		);
		if (s.route === 'script') document.getElementById('script-editor')?.focus({ preventScroll: true });
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
}
