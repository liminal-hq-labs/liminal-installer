// A settings port and host for tests: a shared "file", with a change reaching every listener and optional refusals.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Host } from '../lib/host';
import { defaultSettings, mergeSettings, type Settings, type SettingsPort } from '../lib/settings';

export interface TestPort extends SettingsPort {
	current: Settings;
	writes: Partial<Settings>[];
	/** Refuses the next write with this reason. */
	refuseNext(reason: string): void;
	/** A change made by another window. */
	external(patch: Partial<Settings>): void;
}

export function testPort(initial: Partial<Settings> = {}): TestPort {
	const listeners = new Set<(s: Settings) => void>();
	let refusal: string | null = null;
	const tell = () => listeners.forEach((l) => l(port.current));
	const port: TestPort = {
		current: { ...defaultSettings, ...initial },
		writes: [],
		read: async () => port.current,
		async write(patch) {
			port.writes.push(patch);
			if (refusal) {
				const reason = refusal;
				refusal = null;
				throw new Error(reason);
			}
			const merged = mergeSettings(port.current, patch);
			if (!merged.ok) throw new Error(merged.reason);
			port.current = merged.settings;
			tell();
			return port.current;
		},
		async reset() {
			port.current = { ...defaultSettings };
			tell();
			return port.current;
		},
		onChange: async (l) => {
			listeners.add(l);
			return () => void listeners.delete(l);
		},
		refuseNext: (reason) => void (refusal = reason),
		external(patch) {
			port.current = { ...port.current, ...patch };
			tell();
		},
	};
	return port;
}

export function testHost(overrides: Partial<Host> = {}): Host {
	return {
		appInfo: async () => ({
			version: '9.9.9',
			settingsFile: '/cfg/settings.json',
			logDir: '/cfg/logs',
		}),
		reveal: async () => undefined,
		agent: async () => ({ url: 'http://127.0.0.1:5000/mcp', port: 5000 }),
		agentError: async () => null,
		newProject: async (folder) => folder,
		agentToken: async () => 'secret-token',
		regenerateToken: async () => 'fresh-token',
		browse: async () => null,
		copy: async () => undefined,
		confirm: async () => null,
		...overrides,
	};
}
