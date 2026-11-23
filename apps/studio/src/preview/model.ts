// What the preview renderer knows: which look a target gets, sample values and the values a node reads.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { LayoutNode } from '../lib/types';

/** The four looks the Studio draws. The renderer and theme pick one. */
export type Look = 'dos' | 'win95' | 'afterglow' | 'neutral';

/** Stage sizes, in CSS pixels, that each look is composed at. A host scales the stage to fit. */
export const STAGE: Record<Look, { w: number; h: number }> = {
	dos: { w: 960, h: 600 },
	win95: { w: 1024, h: 768 },
	afterglow: { w: 1024, h: 720 },
	neutral: { w: 1024, h: 720 },
};

/**
 * Picks the look for a target. Text-mode and Windows renderers have one look each. The canvas and
 * web renderers draw Neutral for the Neutral theme (or a layout that says `style: "neutral"`) and
 * Afterglow otherwise.
 */
export function lookFor(renderer: string, theme?: string | null, rootStyle?: string): Look {
	if (renderer === 'tui-retro' || renderer === 'tui') return 'dos';
	if (renderer === 'win32-classic' || renderer === 'win16') return 'win95';
	return theme === 'neutral' || rootStyle === 'neutral' ? 'neutral' : 'afterglow';
}

/** Stand-in values for the `{placeholders}` a running engine would fill. */
export const SAMPLE: Record<string, string> = {
	'product.name': 'Sample Product',
	'product.version': '1.0.0',
	'product.publisher': 'Liminal HQ',
	'target.id': 'linux-x86_64',
	'install.path': '/home/you/Sample',
	'install.size': '48 MB',
	'system.free_space': '12.4 GB',
	'system.free_space_after': '12.3 GB',
	'progress.file': 'data/level-03.lvl',
	'progress.percent': '55',
	'plan.summary': '3 components · 41 files · 48 MB',
	'sound.detected': 'Sound Blaster',
	'feed.latest': '1.1.0',
	'receipt.installed': '1.0.0',
	'feed.notes': 'Fixes and new levels.',
	'validate.problems': '',
};

/** Lists a layout may name as a string, such as `options = "sound.cards"`, for when no run supplied them. */
const SAMPLE_LISTS: Record<string, Option[]> = {
	'sound.cards': [
		{ id: 'none', label: 'No sound' },
		{ id: 'speaker', label: 'PC speaker' },
		{ id: 'sbpro', label: 'Sound Blaster / Pro / 16' },
	],
};

/** What the preview lets a person do. In `edit` a click selects a node; in `run` the controls work. */
export interface Interact {
	mode: 'edit' | 'run';
	/** The selected node's `data-node-path` key, outlined in the preview. */
	selected?: string | null;
	onSelect?: (key: string) => void;
	onChange?: (key: string, value: unknown) => void;
	onAction?: (action: string) => void;
}

/** Readings that override the page's meters and billboard while a preview plays. */
export interface Live {
	file?: number;
	total?: number;
	freeSpace?: number;
	slide?: number;
}

/** What a preview knows about the run it draws. */
export interface RenderOptions {
	renderer: string;
	theme?: string | null;
	/** Sample strings for `{placeholders}`, over {@link SAMPLE}. */
	vars?: Record<string, string>;
	/** The page state: bound values, meter readings and lists a script filled in. */
	state?: Record<string, unknown>;
	/** The project's components, for `items = "components"` when the tree was not expanded. */
	components?: { id: string; name: string; required?: boolean }[];
	interact?: Interact;
	live?: Live;
	/** `contain` fills the parent and fits the whole stage in it; the default fits its width. */
	fit?: 'width' | 'contain';
	/** Where a node came from, as its `data-node-path` key, so a click can select it. */
	pathOf?: (node: LayoutNode) => string | undefined;
}

/** Replaces `{name}` with its sample value, leaving unknown names visible. */
export function fill(text: string, vars: Record<string, string> = SAMPLE): string {
	return text.replace(/\{([A-Za-z0-9_.-]+)\}/g, (m, key: string) => vars[key] ?? m);
}

export interface Option {
	id: string;
	label: string;
	description?: string;
	checked?: boolean;
	required?: boolean;
	badges?: { text: string; tone?: string }[];
}

export interface Ctx {
	look: Look;
	renderer: string;
	vars: Record<string, string>;
	state: Record<string, unknown>;
	components: NonNullable<RenderOptions['components']>;
	interact?: Interact;
	live?: Live;
	pathOf?: (node: LayoutNode) => string | undefined;
}

export type Props = Record<string, unknown>;
export const props = (n: LayoutNode): Props => n.props ?? {};
export const str = (n: LayoutNode, key: string): string => {
	const v = props(n)[key];
	return typeof v === 'string' ? v : '';
};
export const num = (n: LayoutNode, key: string, dflt: number): number => {
	const v = props(n)[key];
	return typeof v === 'number' ? v : dflt;
};

// ---------------------------------------------------------------------------------------------
// Values

export function lookup(cx: Ctx, bind: string | undefined): unknown {
	if (!bind) return undefined;
	if (bind in cx.state) return cx.state[bind];
	return cx.vars[bind];
}

export function options(cx: Ctx, node: LayoutNode): Option[] {
	const p = props(node);
	const raw = p.items ?? p.options;
	if (typeof raw === 'string') {
		if (raw === 'components' && cx.components.length)
			return cx.components.map((c) => ({
				id: c.id,
				label: c.name,
				required: c.required,
				checked: true,
			}));
		const fromState = cx.state[raw];
		if (Array.isArray(fromState)) return fromState.map(toOption);
		return SAMPLE_LISTS[raw] ?? [];
	}
	return Array.isArray(raw) ? raw.map(toOption) : [];
}

function toOption(o: unknown, i: number): Option {
	if (typeof o === 'object' && o !== null) {
		const r = o as Record<string, unknown>;
		const id = String(r.id ?? i);
		return {
			id,
			label: String(r.label ?? r.name ?? id),
			description: typeof r.description === 'string' ? r.description : undefined,
			checked: r.checked === true,
			required: r.required === true,
			badges: Array.isArray(r.badges) ? (r.badges as Option['badges']) : undefined,
		};
	}
	return { id: String(o), label: String(o) };
}

const METER_SAMPLE: Record<string, number> = { file: 0.7, total: 0.55, free_space: 0.94 };

const LIVE_KEY = { file: 'file', total: 'total', free_space: 'freeSpace' } as const;

/** The meter's source: `file`, `total` or `free_space`. */
export const meterSource = (node: LayoutNode): keyof typeof LIVE_KEY => {
	const s = str(node, 'source') || str(node, 'meter') || 'total';
	return s in LIVE_KEY ? (s as keyof typeof LIVE_KEY) : 'total';
};

export function fraction(cx: Ctx, node: LayoutNode): number {
	const live = cx.live?.[LIVE_KEY[meterSource(node)]];
	if (typeof live === 'number') return Math.max(0, Math.min(1, live));
	const v = lookup(cx, node.bind);
	if (typeof v === 'number') return Math.max(0, Math.min(1, v > 1 ? v / 100 : v));
	return METER_SAMPLE[str(node, 'source') || str(node, 'meter') || 'total'] ?? 0.5;
}

export const isLow = (node: LayoutNode, pct: number): boolean =>
	str(node, 'source') === 'free_space' && pct / 100 < num(node, 'low_below', 0.05);

/** Whether a tree has a meter or a billboard, which a Play button can animate. */
export function hasMovingParts(node: LayoutNode): boolean {
	return (
		node.kind === 'meter' || node.kind === 'billboard' || (node.children ?? []).some(hasMovingParts)
	);
}
