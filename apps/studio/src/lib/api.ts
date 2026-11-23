// The Studio backend as the UI sees it: Tauri commands, or an in-browser demo in development.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import type {
	ChangeEvent,
	ErrorKind,
	Flow,
	FlowView,
	HistoryView,
	Layout,
	McpInfo,
	Preview,
	Problem,
	ProjectSummary,
	ScriptDiagnostic,
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
	/** Runs the script's hooks against a sample; `target` (`os-arch:renderer`) builds the sample from that target. */
	testScript(path: string, source?: string, target?: string): Promise<ScriptReport>;
	/** Checks Lua source without running it: syntax errors and warnings, with lines and columns. */
	checkScriptSyntax(source: string): Promise<ScriptDiagnostic[]>;
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

const ERROR_KINDS: readonly ErrorKind[] = [
	'not_found',
	'invalid_input',
	'io',
	'project',
	'script',
	'backend',
	'agent',
	'internal',
];

/**
 * What a failed command rejects with: the `kind` to branch on and a `message` to show. The shell
 * sends it as `{ kind, message }` (`ErrorPayload`); anything else that fails on the way, such as
 * the page not being in the shell, becomes `internal`.
 */
export class StudioError extends Error {
	readonly kind: ErrorKind;

	constructor(kind: ErrorKind, message: string) {
		super(message);
		this.name = 'StudioError';
		this.kind = kind;
	}
}

/** Turns whatever a command rejected with into a `StudioError`. */
export function toStudioError(raw: unknown): StudioError {
	if (raw instanceof StudioError) return raw;
	if (raw && typeof raw === 'object' && !(raw instanceof Error)) {
		const { kind, message } = raw as { kind?: unknown; message?: unknown };
		if (typeof message === 'string') {
			const known = ERROR_KINDS.find((k) => k === kind);
			return new StudioError(known ?? 'internal', message);
		}
	}
	if (raw instanceof Error) return new StudioError('internal', raw.message);
	return new StudioError('internal', typeof raw === 'string' ? raw : String(raw));
}

/** True when `e` is a `StudioError`, of `kind` when one is named. */
export function isStudioError(e: unknown, kind?: ErrorKind): e is StudioError {
	return e instanceof StudioError && (kind === undefined || e.kind === kind);
}

/** The text to show for any failure. */
export const errorMessage = (e: unknown): string => toStudioError(e).message;

/** Calls a Tauri command; a rejection arrives as a `StudioError`. */
export async function invokeCommand<T>(
	command: string,
	args?: Record<string, unknown>,
): Promise<T> {
	const { invoke } = await import('@tauri-apps/api/core');
	try {
		return await invoke<T>(command, args);
	} catch (e) {
		throw toStudioError(e);
	}
}

async function tauriApi(): Promise<Api> {
	const invoke = invokeCommand;
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
		testScript: (path, source, target) =>
			invoke('test_script', { path, source: source ?? null, target: target ?? null }),
		checkScriptSyntax: (source) => invoke('check_script_syntax', { source }),
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
			const picked = await open({
				directory: true,
				title: t('api.openProjectTitle'),
				...(startIn ? { defaultPath: startIn } : {}),
			});
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
	throw new Error(t('api.needsShell'));
}
