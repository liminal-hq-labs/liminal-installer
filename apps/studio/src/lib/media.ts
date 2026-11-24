// Pure helpers for the Media screen: sizes, the label, serial and folder rules, and the list's pills.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { formatLocale } from '../i18n/active';
import type {
	DocKey,
	MediaInfo,
	MediaKind,
	MediaPlan,
	MediaSeverity,
	MediaWarning,
	MediaWarningCode,
} from './types';

/** The most characters a FAT volume label has. */
export const MAX_FAT_LABEL = 11;
/** The most characters a Joliet volume label has. */
export const MAX_JOLIET_LABEL = 16;
/** The most characters an ISO 9660 volume id has. */
export const MAX_ISO_ID = 32;

/**
 * The punctuation a FAT label and an 8.3 name may hold beside capital letters and digits. It is the rule
 * of `li-media`'s `check_label`, which is the floppy writer's: this file mirrors it so the screen can
 * answer as a person types, and `li-media` stays the one that decides.
 */
const FAT_PUNCTUATION = "_-!#$%&'()@^{}~";

/** A whole number of bytes with the locale's digit grouping: `1,286,656`. */
export function formatBytes(bytes: number): string {
	return new Intl.NumberFormat(formatLocale()).format(bytes);
}

/** What is wrong with a label, for the screen to say in words. `null` when it is fine. */
export type LabelProblem =
	{ kind: 'tooLong'; length: number; max: number } | { kind: 'badChar'; char: string };

/** Checks a volume label against the rule of its medium: 11 characters on a floppy, 16 on a CD. */
export function checkLabel(label: string, kind: MediaKind): LabelProblem | null {
	const max = kind === 'cdrom' ? MAX_JOLIET_LABEL : MAX_FAT_LABEL;
	const chars = [...label];
	if (chars.length > max) return { kind: 'tooLong', length: chars.length, max };
	const bad = chars.find(
		(c) => !(/[A-Z0-9 ]/.test(c) || (FAT_PUNCTUATION.includes(c) && c.length === 1)),
	);
	return bad === undefined ? null : { kind: 'badChar', char: bad };
}

/** The most a label of this kind may hold. */
export const labelMax = (kind: MediaKind): number =>
	kind === 'cdrom' ? MAX_JOLIET_LABEL : MAX_FAT_LABEL;

/** A serial that is `auto`, or eight hexadecimal digits (with an optional dash after the fourth). */
export function checkSerial(text: string): 'auto' | number | null {
	if (text.toLowerCase() === 'auto') return 'auto';
	const m = /^([0-9a-fA-F]{4})-?([0-9a-fA-F]{4})$/.exec(text);
	return m ? parseInt(`${m[1]}${m[2]}`, 16) : null;
}

/** A serial as it is shown: `XXXX-XXXX`. */
export function formatSerial(serial: number): string {
	const hex = (serial >>> 0).toString(16).toUpperCase().padStart(8, '0');
	return `${hex.slice(0, 4)}-${hex.slice(4)}`;
}

/** What is wrong with a target's folder. `null` when it is fine. */
export type DirProblem = { kind: 'notOneFolder' } | { kind: 'not83' };

/** A folder is empty (the root) or one 8.3 name of capital letters, digits and the FAT punctuation. */
export function checkDir(dir: string): DirProblem | null {
	if (dir === '') return null;
	if (/[\\/]/.test(dir)) return { kind: 'notOneFolder' };
	const [stem = '', ext = '', ...rest] = dir.split('.');
	const ok = (s: string, max: number) =>
		s.length <= max && [...s].every((c) => /[A-Z0-9]/.test(c) || FAT_PUNCTUATION.includes(c));
	return stem === '' || rest.length > 0 || !ok(stem, 8) || !ok(ext, 3) ? { kind: 'not83' } : null;
}

/** The ISO 9660 volume id of a label: capital letters, digits and `_`. */
export function isoVolumeId(label: string): string {
	return [...label.toUpperCase()]
		.map((c) => (/[A-Z0-9]/.test(c) ? c : '_'))
		.slice(0, MAX_ISO_ID)
		.join('');
}

/** The Joliet volume label of a label. */
export const jolietLabel = (label: string): string =>
	[...label].slice(0, MAX_JOLIET_LABEL).join('');

/** How a medium stands, for its pill: fits, fits with a warning, or does not fit. */
export type Fit = 'ok' | 'warning' | 'error';

const FIT_CODES: ReadonlySet<MediaWarningCode> = new Set(['does_not_fit']);
const TIGHT_CODES: ReadonlySet<MediaWarningCode> = new Set(['below_reserve']);

export function fitOf(plan: MediaPlan): Fit {
	if (plan.warnings.some((w) => w.severity === 'error' || FIT_CODES.has(w.code))) return 'error';
	return plan.warnings.some((w) => TIGHT_CODES.has(w.code)) ? 'warning' : 'ok';
}

/** The most serious severity among warnings, or null with none. */
export function worst(warnings: MediaWarning[]): MediaSeverity | null {
	if (warnings.some((w) => w.severity === 'error')) return 'error';
	return warnings.length > 0 ? 'warning' : null;
}

/** The label of a new medium: the slug in upper case, cut to what the kind allows. */
export function defaultLabel(slug: string, kind: MediaKind): string {
	return slug
		.toUpperCase()
		.replace(/[^A-Z0-9]/g, '')
		.slice(0, labelMax(kind));
}

/** An id for a new medium of this kind that no existing one has: `floppy`, `floppy-2`, `floppy-3`. */
export function nextMediaId(kind: MediaKind, existing: readonly MediaInfo[]): string {
	const base = kind === 'cdrom' ? 'cd' : 'floppy';
	const taken = new Set(existing.map((m) => m.id));
	if (!taken.has(base)) return base;
	for (let n = 2; ; n++) if (!taken.has(`${base}-${n}`)) return `${base}-${n}`;
}

/**
 * The document a media edit belongs to in the history. The generated `DocKey` has no media variant until
 * `li-studio` stores media (D84), so the demo and the fakes name it with this stand-in.
 */
export const MEDIA_DOC = { type: 'media' } as unknown as DocKey;
