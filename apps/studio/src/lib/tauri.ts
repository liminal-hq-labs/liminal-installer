// Pure helpers for the Tauri screen: badges, overrides, size costs and the plain-language notes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, type MessageId } from '../i18n/messages';
import type {
	Problem,
	TauriIdentityRow,
	TauriSettingsChange,
	TauriUpdates,
	TauriWebview2,
	UpdateMode,
	Webview2Mode,
	Webview2Override,
} from './types';

/** The `[product]` fields the import fills, in the order the table shows them. */
export const IDENTITY_FIELDS = ['id', 'name', 'publisher', 'version'] as const;

/** The five ways WebView2 arrives, in the order the radio group lists them. */
export const WEBVIEW2_MODES: readonly Webview2Mode[] = [
	'downloadBootstrapper',
	'embedBootstrapper',
	'offlineInstaller',
	'fixedRuntime',
	'skip',
];

export const UPDATE_MODES: readonly UpdateMode[] = ['none', 'liminal', 'tauri', 'both'];

export type FieldBadge = 'imported' | 'overridden' | 'differs';

/** What to say about one product field: kept by the project, changed by the next import, or just imported. */
export function fieldBadges(row: TauriIdentityRow): FieldBadge[] {
	if (row.overridden) return ['overridden'];
	if (row.differs) return ['differs'];
	return row.imported === null ? [] : ['imported'];
}

/** The fields that keep the project's value after `field` is turned on or off. */
export function nextOverrides(
	rows: readonly TauriIdentityRow[],
	field: string,
	keep: boolean,
): string[] {
	const kept = new Set(rows.filter((r) => r.overridden).map((r) => r.field));
	if (keep) kept.add(field);
	else kept.delete(field);
	return [...kept];
}

/** "+127 MB", or a plain "no extra size" for a mode that adds nothing. */
export function formatSizeCost(mb: number): string {
	return mb > 0 ? t('tauri.sizeCost', { mb }) : t('tauri.sizeNone');
}

export const MODE_LABEL: Record<Webview2Mode, MessageId> = {
	downloadBootstrapper: 'tauri.mode.downloadBootstrapper',
	embedBootstrapper: 'tauri.mode.embedBootstrapper',
	offlineInstaller: 'tauri.mode.offlineInstaller',
	fixedRuntime: 'tauri.mode.fixedRuntime',
	skip: 'tauri.mode.skip',
};

/** True when the project holds any WebView2 value of its own. */
export function webview2Overridden(w: TauriWebview2): boolean {
	const o = w.override;
	return !!o && Object.values(o).some((v) => v !== null && v !== undefined);
}

/** The project's WebView2 override with `patch` applied; an empty value takes that field out. */
export function webview2Patch(
	w: TauriWebview2,
	patch: { [K in keyof Webview2Override]?: Webview2Override[K] | '' },
): Webview2Override {
	const next: Record<string, unknown> = { ...(w.override ?? {}) };
	for (const [key, value] of Object.entries(patch)) {
		if (value === '' || value === null || value === undefined) delete next[key];
		else next[key] = value;
	}
	for (const key of Object.keys(next)) if (next[key] === null) delete next[key];
	return next as Webview2Override;
}

const MISSING: Record<string, MessageId> = {
	skip: 'tauri.missing.skip',
	fixed: 'tauri.missing.fixed',
	download: 'tauri.missing.download',
	bootstrapper: 'tauri.missing.bootstrapper',
	offline: 'tauri.missing.offline',
};

const PRESENT: Record<string, MessageId> = {
	skip: 'tauri.present.skip',
	fixed: 'tauri.present.fixed',
	found: 'tauri.present.found',
	'found-checked': 'tauri.present.foundChecked',
};

/** The sentence for what the step does with no WebView2 on the machine. */
export function ifMissingText(key: string): string {
	const id = MISSING[key];
	return id ? t(id) : t('tauri.stepUnknown', { key });
}

/** The sentence for what the step does when WebView2 is already installed. */
export function ifPresentText(key: string, minVersion: string | null | undefined): string {
	const id = PRESENT[key];
	return id ? t(id, { version: minVersion ?? '' }) : t('tauri.stepUnknown', { key });
}

/** The plain-language rule for who updates the installed app, given whether the app has an updater of its own. */
export function updateNote(mode: UpdateMode, appHasUpdater: boolean): MessageId {
	switch (mode) {
		case 'none':
			return 'tauri.updates.noteNone';
		case 'liminal':
			return appHasUpdater ? 'tauri.updates.noteTwoUpdaters' : 'tauri.updates.noteLiminal';
		case 'tauri':
			return appHasUpdater ? 'tauri.updates.noteTauri' : 'tauri.updates.noteNoUpdater';
		case 'both':
			return 'tauri.updates.noteBoth';
	}
}

/** True when the installer and the app would each apply updates, which `both` resolves by taking the newer version. */
export function hasTwoUpdaters(u: TauriUpdates): boolean {
	return u.installer_applies && u.app_has_updater && u.mode !== 'both';
}

/** The `[updates]` table after changing some of its fields; empty text takes a field out. */
export function updatesPatch(
	u: TauriUpdates,
	patch: { mode?: UpdateMode; feed?: string; channel?: string; interval?: string },
): TauriSettingsChange {
	const mode = patch.mode ?? u.mode;
	const feed = (patch.feed ?? u.feed ?? '').trim();
	const channel = (patch.channel ?? u.channel ?? '').trim();
	const intervalText = (patch.interval ?? String(u.check_interval_hours ?? '')).trim();
	const hours = intervalText === '' ? null : Number.parseInt(intervalText, 10);
	return {
		updates: {
			mode,
			...(feed ? { feed } : {}),
			...(channel ? { channel } : {}),
			...(hours !== null && Number.isFinite(hours) && hours >= 0
				? { check_interval_hours: hours }
				: {}),
		},
	};
}

/** True when `text` is empty or a whole number of hours. */
export const isIntervalText = (text: string): boolean => /^\s*\d*\s*$/.test(text);

/** Errors first, then warnings, each group in the order the engine reported them. */
export function sortProblems(problems: readonly Problem[]): Problem[] {
	return [...problems].sort(
		(a, b) => Number(b.severity === 'error') - Number(a.severity === 'error'),
	);
}

/** The history label for a settings change, from the parts it touches. */
export function settingsLabel(change: TauriSettingsChange): string {
	if (change.windows) return t('history.tauriWindows', { mode: change.windows });
	if (change.overrides) return t('history.tauriOverrides');
	if (change.webview2) return t('history.tauriWebview2');
	if (change.updates) return t('history.tauriUpdates');
	return t('history.tauriSettings');
}

/** The last part of a path, for labels. */
export const baseName = (path: string): string => path.split(/[\\/]/).filter(Boolean).pop() ?? path;
