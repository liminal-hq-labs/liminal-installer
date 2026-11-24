// Pure helpers for the Media screen: sizes, the label, serial and folder rules, and the list's pills.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { formatLocale } from '../i18n/active';
import type {
	DiskEntry,
	DocKey,
	MediaInfo,
	MediaKind,
	MediaPlan,
	MediaSeverity,
	MediaWarning,
	MediaWarningCode,
	PackEntryPlan,
	VolumePlan,
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

// ---- the capacity meter

/** The bytes one block cell of a floppy's meter stands for. */
export const CELL_BYTES = 16 * 1024;
/** The cells of a disc's meter, which has too many bytes to draw one cell for each 16 KiB. */
const DISC_CELLS = 80;

/** How many cells a medium's meter has. */
export const meterCellCount = (kind: MediaKind, capacity: number): number =>
	kind === 'cdrom' ? DISC_CELLS : Math.ceil(capacity / CELL_BYTES);

/** What fills a disk: its own files, the common pack, the reserve that must stay free, and the rest. */
export interface MeterParts {
	own: number;
	common: number;
	reserve: number;
	free: number;
	/** The bytes beyond the disk, when it is over full. */
	over: number;
}

/** Splits one disk's bytes into the parts the meter draws. The reserve is the first of the free bytes that are left. */
export function meterParts(volume: VolumePlan, reserve: number): MeterParts {
	const free = Math.max(0, volume.free);
	const held = Math.min(Math.max(0, reserve), free);
	return {
		common: Math.min(volume.common, volume.used),
		own: Math.max(0, Math.min(volume.used, volume.capacity) - volume.common),
		reserve: held,
		free: free - held,
		over: Math.max(0, -volume.free),
	};
}

/**
 * Turns bytes into whole cells by largest remainder, so the cells always add up to `count`. A part
 * that has any bytes gets at least one cell, so a small reserve is still drawn.
 */
export function meterCells(parts: MeterParts, count: number): Omit<MeterParts, 'over'> {
	const keys = ['own', 'common', 'reserve', 'free'] as const;
	const total = keys.reduce((n, k) => n + parts[k], 0);
	if (total === 0 || count <= 0) return { own: 0, common: 0, reserve: 0, free: count };
	const exact = keys.map((k) => (parts[k] / total) * count);
	const cells = exact.map((x) => Math.floor(x));
	let left = count - cells.reduce((n, c) => n + c, 0);
	const byRemainder = exact
		.map((x, i) => [x - Math.floor(x), i] as const)
		.sort((a, b) => b[0] - a[0]);
	for (const [, i] of byRemainder) {
		if (left <= 0) break;
		cells[i]! += 1;
		left -= 1;
	}
	keys.forEach((k, i) => {
		if (parts[k] > 0 && cells[i] === 0) {
			const donor = cells.indexOf(Math.max(...cells));
			cells[donor]! -= 1;
			cells[i] = 1;
		}
	});
	return { own: cells[0]!, common: cells[1]!, reserve: cells[2]!, free: cells[3]! };
}

// ---- the disk map

/** One row of the disk map's tree. */
export interface TreeRow {
	/** Unique in the plan: `1/WIN31/SETUP.LPK`. */
	key: string;
	level: number;
	/** `volume` is the root of a disk; `entry` a file or folder; `pack` a file inside a pack. */
	kind: 'volume' | 'entry' | 'pack';
	volume: VolumePlan;
	entry?: DiskEntry;
	packEntry?: PackEntryPlan;
	parent: string | null;
	expandable: boolean;
	/** The position among siblings, counting from 1, and how many there are. */
	pos: number;
	size: number;
}

/** The key of a disk's root. */
export const volumeKey = (v: VolumePlan): string => String(v.number);

const sizeOf = (e: DiskEntry): number => e.on_disk + e.children.reduce((n, c) => n + sizeOf(c), 0);

function childRows(
	volume: VolumePlan,
	parent: TreeRow,
	entries: DiskEntry[],
	expanded: ReadonlySet<string>,
): TreeRow[] {
	return entries.flatMap((entry, i): TreeRow[] => {
		const key = `${parent.key}/${entry.name}`;
		const row: TreeRow = {
			key,
			level: parent.level + 1,
			kind: 'entry',
			volume,
			entry,
			parent: parent.key,
			expandable: entry.children.length > 0 || entry.entries.length > 0,
			pos: i + 1,
			size: sizeOf(entry),
		};
		if (!row.expandable || !expanded.has(key)) return [row];
		const packs: TreeRow[] = entry.entries.map((p, j) => ({
			key: `${key}/${p.stored}`,
			level: row.level + 1,
			kind: 'pack',
			volume,
			entry,
			packEntry: p,
			parent: key,
			expandable: false,
			pos: j + 1,
			size: p.packed,
		}));
		return [row, ...childRows(volume, row, entry.children, expanded), ...packs];
	});
}

/** The rows of a disk's tree that are showing: its root, and what is under each branch that is open. */
export function visibleRows(volume: VolumePlan, expanded: ReadonlySet<string>): TreeRow[] {
	const root: TreeRow = {
		key: volumeKey(volume),
		level: 1,
		kind: 'volume',
		volume,
		parent: null,
		expandable: true,
		pos: 1,
		size: volume.used,
	};
	return expanded.has(root.key)
		? [root, ...childRows(volume, root, volume.root, expanded)]
		: [root];
}

/** How many siblings a row has. */
export function siblings(rows: TreeRow[], row: TreeRow): number {
	return row.parent === null ? 1 : rows.filter((r) => r.parent === row.parent).length;
}

// ---- the sizes tables

export type SortDir = 'ascending' | 'descending';

/** Sorts a copy of `rows` by `value`; text compares as text, numbers as numbers, and ties keep their order. */
export function sortedBy<T>(
	rows: readonly T[],
	value: (row: T) => string | number,
	dir: SortDir,
): T[] {
	const sign = dir === 'ascending' ? 1 : -1;
	return rows
		.map((row, i) => ({ row, i, v: value(row) }))
		.sort((a, b) => {
			const order =
				typeof a.v === 'number' && typeof b.v === 'number'
					? a.v - b.v
					: String(a.v).localeCompare(String(b.v), undefined, { numeric: true });
			return order * sign || a.i - b.i;
		})
		.map((x) => x.row);
}
