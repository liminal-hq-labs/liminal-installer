// The Studio's settings: a typed schema with validation, kept in a file in the desktop shell and in the browser's storage in the demo.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invokeCommand } from './api';

/** The file format version this build writes. Matches `VERSION` in `src-tauri/src/settings.rs`. */
export const SETTINGS_VERSION = 1;

export const THEMES = ['afterglow', 'afterglow-dark', 'afterglow-light', 'system'] as const;
export type ThemeId = (typeof THEMES)[number];
export const DENSITIES = ['comfortable', 'compact'] as const;
export type Density = (typeof DENSITIES)[number];
export const MOTIONS = ['system', 'on', 'off'] as const;
export type MotionSetting = (typeof MOTIONS)[number];

/** Inclusive bounds of the numeric settings. Kept equal to the ones in the Rust schema. */
export const LIMITS = {
	recentLimit: [5, 20],
	textSize: [90, 130],
	editorFontSize: [10, 24],
	tabWidth: [1, 8],
	mcpPort: [0, 65535],
} as const;

export interface Settings {
	/** The file format version. Managed by the Studio; a patch cannot change it. */
	version: number;

	// General
	menuBar: boolean;
	reopenLastProject: boolean;
	confirmDiscard: boolean;
	recentLimit: number;
	/** Newest first, at most `recentLimit` long. */
	recentProjects: string[];
	lastProject: string | null;

	// Appearance
	theme: ThemeId;
	density: Density;
	reduceMotion: MotionSetting;
	/** Interface text size, as a percentage. */
	textSize: number;

	// Editor
	editorFontSize: number;
	tabWidth: number;
	wrapLines: boolean;
	lineNumbers: boolean;
	formatOnSave: boolean;

	// Preview
	/** The full id of the target a dry run starts on; empty for the first one. */
	previewTarget: string;
	previewAutoRun: boolean;
	showSkippedSteps: boolean;

	// Agents
	mcpEnabled: boolean;
	/** 0 picks a free port. */
	mcpPort: number;

	// Projects
	projectsFolder: string;
	runnerPath: string;
	dockerPath: string;
}

export type SettingKey = keyof Settings;

export const defaultSettings: Settings = {
	version: SETTINGS_VERSION,
	menuBar: false,
	reopenLastProject: false,
	confirmDiscard: true,
	recentLimit: 10,
	recentProjects: [],
	lastProject: null,
	theme: 'afterglow',
	density: 'comfortable',
	reduceMotion: 'system',
	textSize: 100,
	editorFontSize: 13,
	tabWidth: 4,
	wrapLines: true,
	lineNumbers: true,
	formatOnSave: false,
	previewTarget: '',
	previewAutoRun: false,
	showSkippedSteps: true,
	// The desktop shell decides this one (on in development, off in a release build); the page only shows it.
	mcpEnabled: false,
	mcpPort: 0,
	projectsFolder: '',
	runnerPath: '',
	dockerPath: '',
};

const isInt = (v: unknown): v is number => typeof v === 'number' && Number.isInteger(v);
const bool = (v: unknown): v is boolean => typeof v === 'boolean';
const text = (v: unknown): v is string =>
	typeof v === 'string' && !v.includes('\0') && !v.includes('\n');
const oneOf =
	(list: readonly string[]) =>
	(v: unknown): boolean =>
		typeof v === 'string' && list.includes(v);
const inRange =
	([lo, hi]: readonly [number, number]) =>
	(v: unknown): boolean =>
		isInt(v) && v >= lo && v <= hi;

/** Whether a value is acceptable for a setting, and why not. */
const CHECKS: { [K in Exclude<SettingKey, 'version'>]: [(v: unknown) => boolean, string] } = {
	menuBar: [bool, 'must be on or off'],
	reopenLastProject: [bool, 'must be on or off'],
	confirmDiscard: [bool, 'must be on or off'],
	recentLimit: [inRange(LIMITS.recentLimit), 'the recent projects limit must be between 5 and 20'],
	recentProjects: [
		(v) => Array.isArray(v) && v.every((p) => text(p) && p !== ''),
		'a recent project has no path',
	],
	lastProject: [(v) => v === null || (text(v) && v !== ''), 'the last project is not a valid path'],
	theme: [oneOf(THEMES), 'unknown theme'],
	density: [oneOf(DENSITIES), 'unknown density'],
	reduceMotion: [oneOf(MOTIONS), 'unknown motion setting'],
	textSize: [inRange(LIMITS.textSize), 'text size must be between 90 and 130'],
	editorFontSize: [inRange(LIMITS.editorFontSize), 'font size must be between 10 and 24'],
	tabWidth: [inRange(LIMITS.tabWidth), 'tab width must be between 1 and 8'],
	wrapLines: [bool, 'must be on or off'],
	lineNumbers: [bool, 'must be on or off'],
	formatOnSave: [bool, 'must be on or off'],
	previewTarget: [text, 'not a valid target'],
	previewAutoRun: [bool, 'must be on or off'],
	showSkippedSteps: [bool, 'must be on or off'],
	mcpEnabled: [bool, 'must be on or off'],
	mcpPort: [inRange(LIMITS.mcpPort), 'the port must be between 0 and 65535'],
	projectsFolder: [text, 'not a valid path'],
	runnerPath: [text, 'not a valid path'],
	dockerPath: [text, 'not a valid path'],
};

/** The reason a patch cannot be applied, or null when it can. An unknown key or a bad value refuses the whole patch. */
export function validatePatch(patch: unknown): string | null {
	if (!patch || typeof patch !== 'object' || Array.isArray(patch))
		return 'settings must be an object';
	for (const [key, value] of Object.entries(patch)) {
		if (key === 'version') return 'the version is managed by the Studio';
		const check = (CHECKS as Record<string, [(v: unknown) => boolean, string] | undefined>)[key];
		if (!check) return `unknown setting \`${key}\``;
		if (!check[0](value)) return check[1];
	}
	return null;
}

/** Puts the recent list within its limit and drops repeats. */
export function normalizeRecent(list: readonly string[], limit: number): string[] {
	return [...new Set(list)].slice(0, limit);
}

/** The settings in `base` with `patch` over them, or the reason it was refused. The recent list is kept within its limit. */
export function mergeSettings(
	base: Settings,
	patch: Partial<Settings>,
): { ok: true; settings: Settings } | { ok: false; reason: string } {
	const reason = validatePatch(patch);
	if (reason) return { ok: false, reason };
	const next = { ...base, ...patch };
	next.recentProjects = normalizeRecent(next.recentProjects, next.recentLimit);
	return { ok: true, settings: next };
}

/** Moves `path` to the front of the recent list. */
export function pushRecent(list: readonly string[], path: string, limit: number): string[] {
	return normalizeRecent([path, ...list], limit);
}

/**
 * The settings in `value`, with the default for every field that is missing or invalid and the other fields kept.
 * An older file with no version is migrated: the first file held only `menuBar` and `theme`, which keep their meaning.
 */
export function normalizeSettings(value: unknown): Settings {
	const v = (value && typeof value === 'object' && !Array.isArray(value) ? value : {}) as Record<
		string,
		unknown
	>;
	let settings = { ...defaultSettings };
	for (const key of Object.keys(CHECKS) as Exclude<SettingKey, 'version'>[]) {
		if (key in v && CHECKS[key][0](v[key])) settings = { ...settings, [key]: v[key] };
	}
	settings.recentProjects = normalizeRecent(settings.recentProjects, settings.recentLimit);
	settings.version = isInt(v.version) ? Math.max(v.version, SETTINGS_VERSION) : SETTINGS_VERSION;
	return settings;
}

/** The keys of `value` that are not settings this build knows, so they can be written back untouched. */
export function unknownKeys(value: unknown): Record<string, unknown> {
	if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
	return Object.fromEntries(
		Object.entries(value).filter(([k]) => k !== 'version' && !(k in CHECKS)),
	);
}

/** True when two settings are equal in every field. */
export function sameSettings(a: Settings, b: Settings): boolean {
	return (Object.keys(defaultSettings) as SettingKey[]).every(
		(k) => JSON.stringify(a[k]) === JSON.stringify(b[k]),
	);
}

export const SETTINGS_EVENT = 'studio://settings-changed';

/** Where the settings live and how a window hears about a change made in another window. */
export interface SettingsPort {
	read(): Promise<Settings>;
	/** Merges `patch` into the saved settings and resolves with the result. Rejects with the reason when refused. */
	write(patch: Partial<Settings>): Promise<Settings>;
	/** Puts every setting back to its default. */
	reset(): Promise<Settings>;
	/** Calls `listener` after any window changes the settings; resolves to an unsubscribe function. */
	onChange(listener: (settings: Settings) => void): Promise<() => void>;
}

export async function tauriSettings(): Promise<SettingsPort> {
	const { listen } = await import('@tauri-apps/api/event');
	const call = async (command: string, args?: Record<string, unknown>) =>
		normalizeSettings(await invokeCommand(command, args));
	return {
		read: () => call('read_settings'),
		write: (patch) => call('write_settings', { patch }),
		reset: () => call('reset_settings'),
		onChange: (listener) =>
			listen<Settings>(SETTINGS_EVENT, (e) => listener(normalizeSettings(e.payload))),
	};
}

/** The narrow part of `Storage` that is used. */
export interface KeyValueStore {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

const KEY = 'liminal-installer-studio.settings';

/** Settings kept in browser storage. Blocked or broken storage means the defaults, and a write still holds for the session. */
export function browserSettings(storage: KeyValueStore | undefined): SettingsPort {
	const listeners = new Set<(s: Settings) => void>();
	let current: Settings | undefined;
	let extra: Record<string, unknown> = {};
	const load = (): Settings => {
		try {
			const raw = storage?.getItem(KEY);
			const parsed = raw ? JSON.parse(raw) : {};
			extra = unknownKeys(parsed);
			return normalizeSettings(parsed);
		} catch {
			return { ...defaultSettings };
		}
	};
	const keep = (next: Settings) => {
		current = next;
		try {
			storage?.setItem(KEY, JSON.stringify({ ...extra, ...next }));
		} catch {
			// Storage can be blocked or full; the choice then lasts until the page closes.
		}
		for (const l of [...listeners]) l(next);
		return next;
	};
	return {
		read: async () => (current ??= load()),
		async write(patch) {
			const merged = mergeSettings((current ??= load()), patch);
			if (!merged.ok) throw new Error(merged.reason);
			return keep(merged.settings);
		},
		async reset() {
			current ??= load();
			return keep({ ...defaultSettings });
		},
		async onChange(listener) {
			listeners.add(listener);
			return () => void listeners.delete(listener);
		},
	};
}
