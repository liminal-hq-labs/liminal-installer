// The Studio backend as the UI sees it: Tauri commands, or an in-browser demo in development.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	ChangeEvent,
	Flow,
	FlowView,
	HistoryView,
	Layout,
	McpInfo,
	Preview,
	Problem,
	ProjectSummary,
	ScriptReport,
	TargetInfo,
} from './types';

export interface Api {
	openProject(path: string): Promise<ProjectSummary>;
	projectSummary(): Promise<ProjectSummary>;
	listTargets(): Promise<TargetInfo[]>;
	getFlow(id: string): Promise<FlowView>;
	setFlow(id: string, flow: Flow, label: string): Promise<FlowView>;
	getLayout(id: string, renderer?: string): Promise<Layout>;
	setLayout(id: string, layout: Layout, label: string): Promise<Layout>;
	readScript(path: string): Promise<string>;
	writeScript(path: string, source: string, label: string): Promise<void>;
	testScript(path: string, source?: string): Promise<ScriptReport>;
	runPreview(target: string): Promise<Preview>;
	getPreview(): Promise<Preview | null>;
	checkProject(): Promise<Problem[]>;
	history(): Promise<HistoryView>;
	undo(): Promise<HistoryView>;
	redo(): Promise<HistoryView>;
	mcpInfo(): Promise<McpInfo | null>;
	/** Why the agent server is not running although it is switched on, or null. */
	mcpError(): Promise<string | null>;
	/** Calls `handler` after every change from any source; resolves to an unsubscribe function. */
	onChange(handler: (e: ChangeEvent) => void): Promise<() => void>;
	/** Asks the person for a project folder; null if cancelled or unsupported. */
	pickProject(startIn?: string): Promise<string | null>;
}

export const CHANGE_EVENT = 'studio://changed';

async function tauriApi(): Promise<Api> {
	const { invoke } = await import('@tauri-apps/api/core');
	const { listen } = await import('@tauri-apps/api/event');
	return {
		openProject: (path) => invoke('open_project', { path }),
		projectSummary: () => invoke('project_summary'),
		listTargets: () => invoke('list_targets'),
		getFlow: (id) => invoke('get_flow', { id }),
		setFlow: (id, flow, label) => invoke('set_flow', { id, flow, label }),
		getLayout: (id, renderer) => invoke('get_layout', { id, renderer: renderer ?? null }),
		setLayout: (id, layout, label) => invoke('set_layout', { id, layout, label }),
		readScript: (path) => invoke('read_script', { path }),
		writeScript: (path, source, label) => invoke('write_script', { path, source, label }),
		testScript: (path, source) => invoke('test_script', { path, source: source ?? null }),
		runPreview: (target) => invoke('run_preview', { target }),
		getPreview: () => invoke('get_preview'),
		checkProject: () => invoke('check_project'),
		history: () => invoke('history'),
		undo: () => invoke('undo'),
		redo: () => invoke('redo'),
		mcpInfo: () => invoke('mcp_info'),
		mcpError: () => invoke('mcp_error'),
		onChange: async (handler) => listen<ChangeEvent>(CHANGE_EVENT, (e) => handler(e.payload)),
		pickProject: async (startIn) => {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const picked = await open({ directory: true, title: 'Open installer project', ...(startIn ? { defaultPath: startIn } : {}) });
			return typeof picked === 'string' ? picked : null;
		},
	};
}

export async function createApi(): Promise<Api> {
	if ('__TAURI_INTERNALS__' in window) return tauriApi();
	if (import.meta.env.DEV) {
		const { demoApi } = await import('./demoApi');
		return demoApi();
	}
	throw new Error('The Studio needs its desktop shell.');
}
