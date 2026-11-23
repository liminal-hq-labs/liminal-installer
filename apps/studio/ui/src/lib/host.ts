// What Preferences needs from the desktop shell beyond the settings: file locations, the agent token, pickers and confirmations.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createDialogs, tauriDialogBackend, type DialogResult, type DialogSpec } from '../chrome/dialogWindow';
import { inTauri } from '../chrome/windowControls';

export interface AppInfo {
	version: string;
	settingsFile: string;
	logDir: string;
}

export interface AgentInfo {
	url: string;
	port: number;
}

export interface Host {
	appInfo(): Promise<AppInfo>;
	/** Shows the settings file or the log folder in the file manager. */
	reveal(what: 'settings' | 'logs'): Promise<void>;
	agent(): Promise<AgentInfo | null>;
	/** The token, fetched only when a person asks to see or copy it. */
	agentToken(): Promise<string | null>;
	regenerateToken(): Promise<string>;
	/** Asks for a folder or a file. Null when cancelled. */
	browse(kind: 'folder' | 'file', title: string, start?: string): Promise<string | null>;
	copy(text: string): Promise<void>;
	/** Asks a question in a window of its own, and resolves with the id of the button chosen, or null. */
	confirm(spec: DialogSpec): Promise<DialogResult>;
}

const copyText = async (text: string) => {
	await navigator.clipboard.writeText(text);
};

export async function tauriHost(): Promise<Host> {
	const { invoke } = await import('@tauri-apps/api/core');
	const dialogs = createDialogs(await tauriDialogBackend());
	const call = async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
		try {
			return await invoke<T>(command, args);
		} catch (e) {
			throw new Error(e instanceof Error ? e.message : String(e));
		}
	};
	return {
		appInfo: () => call('app_info'),
		reveal: (what) => call('reveal', { what }),
		agent: () => call('mcp_info'),
		agentToken: () => call('mcp_token'),
		regenerateToken: () => call('mcp_regenerate_token'),
		async browse(kind, title, start) {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const picked = await open({ directory: kind === 'folder', multiple: false, title, ...(start ? { defaultPath: start } : {}) });
			return typeof picked === 'string' ? picked : null;
		},
		copy: copyText,
		confirm: (spec) => dialogs.open(spec).result,
	};
}

/** The browser demo has no files, no agent server and no windows: it answers plainly and asks with `window.confirm`. */
export function browserHost(): Host {
	return {
		appInfo: async () => ({ version: 'browser demo', settingsFile: 'Browser storage', logDir: 'Not available in a browser' }),
		reveal: async () => {
			throw new Error('A browser has no file manager to show it in.');
		},
		agent: async () => null,
		agentToken: async () => null,
		regenerateToken: async () => {
			throw new Error('The agent server runs only in the desktop app.');
		},
		browse: async () => null,
		copy: copyText,
		confirm: async (spec) => {
			const ok = window.confirm([spec.message ?? spec.title, spec.detail].filter(Boolean).join('\n\n'));
			const buttons = spec.buttons ?? [];
			return ok ? (buttons[0]?.id ?? 'ok') : null;
		},
	};
}

export async function createHost(): Promise<Host> {
	return inTauri() ? tauriHost() : browserHost();
}
