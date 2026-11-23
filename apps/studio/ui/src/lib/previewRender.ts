// Draws a layout tree as themed HTML: DOS text mode, Windows 95 classic, Afterglow and Neutral.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { resolveForRenderer } from './layoutEdit';
import type { Layout, LayoutNode } from './types';

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
}

export function escapeHtml(s: string): string {
	return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

/** Replaces `{name}` with its sample value, leaving unknown names visible. */
export function fill(text: string, vars: Record<string, string> = SAMPLE): string {
	return text.replace(/\{([A-Za-z0-9_.-]+)\}/g, (m, key: string) => vars[key] ?? m);
}

interface Option {
	id: string;
	label: string;
	description?: string;
	checked?: boolean;
	required?: boolean;
	badges?: { text: string; tone?: string }[];
}

interface Ctx {
	look: Look;
	renderer: string;
	vars: Record<string, string>;
	state: Record<string, unknown>;
	components: NonNullable<RenderOptions['components']>;
}

type Props = Record<string, unknown>;
const props = (n: LayoutNode): Props => n.props ?? {};
const str = (n: LayoutNode, key: string): string => {
	const v = props(n)[key];
	return typeof v === 'string' ? v : '';
};
const num = (n: LayoutNode, key: string, dflt: number): number => {
	const v = props(n)[key];
	return typeof v === 'number' ? v : dflt;
};

const text = (cx: Ctx, s: string): string => escapeHtml(fill(s, cx.vars));

// ---------------------------------------------------------------------------------------------
// Pixel and vector art

const colour = (c: unknown): string => (typeof c === 'string' && /^(#[0-9a-fA-F]{3,8}|[a-zA-Z]{3,20})$/.test(c) ? c : 'none');
const n2 = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? v : 0);

/** Draws an `art` property as inline SVG: `rects` of `[x, y, w, h, colour]` and free `shapes`. */
export function artSvg(art: unknown, label = ''): string {
	if (typeof art !== 'object' || art === null) return '';
	const a = art as Record<string, unknown>;
	const vb = Array.isArray(a.viewBox) ? a.viewBox : [a.width, a.height];
	const w = n2(vb[0]) || 16;
	const h = n2(vb[1]) || 16;
	const scale = n2(a.scale) || 1;
	const parts: string[] = [];
	for (const r of Array.isArray(a.rects) ? a.rects : []) {
		if (!Array.isArray(r)) continue;
		parts.push(`<rect x="${n2(r[0])}" y="${n2(r[1])}" width="${n2(r[2])}" height="${n2(r[3])}" fill="${colour(r[4])}"/>`);
	}
	for (const sh of Array.isArray(a.shapes) ? a.shapes : []) {
		if (typeof sh !== 'object' || sh === null) continue;
		const s = sh as Record<string, unknown>;
		const paint = `fill="${colour(s.fill)}" stroke="${colour(s.stroke)}" stroke-width="${n2(s.width) || 1}"${s.cap ? ` stroke-linecap="${escapeHtml(String(s.cap))}"` : ''}${s.join ? ` stroke-linejoin="${escapeHtml(String(s.join))}"` : ''}`;
		if (Array.isArray(s.rect)) parts.push(`<rect x="${n2(s.rect[0])}" y="${n2(s.rect[1])}" width="${n2(s.rect[2])}" height="${n2(s.rect[3])}" rx="${n2(s.rx)}" ${paint}/>`);
		else if (typeof s.path === 'string') parts.push(`<path d="${escapeHtml(s.path)}" ${paint}/>`);
		else if (Array.isArray(s.circle)) parts.push(`<circle cx="${n2(s.circle[0])}" cy="${n2(s.circle[1])}" r="${n2(s.circle[2])}" ${paint}/>`);
	}
	const crisp = Array.isArray(a.rects) && a.rects.length > 0 ? ' shape-rendering="crispEdges"' : '';
	const aria = label ? `role="img" aria-label="${escapeHtml(label)}"` : 'aria-hidden="true"';
	return `<svg class="pv-art" width="${w * scale}" height="${h * scale}" viewBox="0 0 ${w} ${h}"${crisp} ${aria}>${parts.join('')}</svg>`;
}

// ---------------------------------------------------------------------------------------------
// Values

function lookup(cx: Ctx, bind: string | undefined): unknown {
	if (!bind) return undefined;
	if (bind in cx.state) return cx.state[bind];
	return cx.vars[bind];
}

function options(cx: Ctx, node: LayoutNode): Option[] {
	const p = props(node);
	const raw = p.items ?? p.options;
	if (typeof raw === 'string') {
		if (raw === 'components' && cx.components.length) return cx.components.map((c) => ({ id: c.id, label: c.name, required: c.required, checked: true }));
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

function fraction(cx: Ctx, node: LayoutNode): number {
	const v = lookup(cx, node.bind);
	if (typeof v === 'number') return Math.max(0, Math.min(1, v > 1 ? v / 100 : v));
	return METER_SAMPLE[str(node, 'source') || str(node, 'meter') || 'total'] ?? 0.5;
}

const isLow = (node: LayoutNode, pct: number): boolean => str(node, 'source') === 'free_space' && pct / 100 < num(node, 'low_below', 0.05);

/** A label with its hot key marked: underlined in Windows, coloured in DOS. */
function hotLabel(cx: Ctx, label: string, hot: string): string {
	const s = fill(label, cx.vars);
	const i = hot ? s.toLowerCase().indexOf(hot.toLowerCase()) : -1;
	if (i < 0) return escapeHtml(s);
	const mark = cx.look === 'win95' ? 'u' : 'span class="pv-hot"';
	const close = cx.look === 'win95' ? 'u' : 'span';
	return `${escapeHtml(s.slice(0, i))}<${mark}>${escapeHtml(s[i]!)}</${close}>${escapeHtml(s.slice(i + 1))}`;
}

// ---------------------------------------------------------------------------------------------
// Nodes

function kids(cx: Ctx, node: LayoutNode, skip: (n: LayoutNode) => boolean = () => false): string {
	return (node.children ?? []).filter((c) => !skip(c)).map((c) => renderNode(cx, c)).join('');
}

const regionOf = (n: LayoutNode): string => str(n, 'region');

function bar(cx: Ctx, node: LayoutNode, pct: number): string {
	const warn = isLow(node, pct);
	if (cx.look === 'dos') {
		const cells = 24;
		const on = Math.round((cells * pct) / 100);
		return `<span class="pv-bar-text${warn ? ' pv-warn' : ''}"><span class="pv-on">${'█'.repeat(on)}</span><span class="pv-off">${'░'.repeat(cells - on)}</span> ${pct}%${warn ? ' LOW' : ''}</span>`;
	}
	if (cx.look === 'win95') {
		const blocks = 24;
		const on = Math.round((blocks * pct) / 100);
		return `<span class="pv-bar pv-sunken">${'<i class="pv-blk pv-lit"></i>'.repeat(on)}${'<i class="pv-blk"></i>'.repeat(blocks - on)}</span><span class="pv-pct">${pct}%</span>`;
	}
	return `<span class="pv-bar"><span class="pv-bar-fill${warn ? ' pv-warn' : ''}" style="width:${pct}%"></span></span>`;
}

function renderNode(cx: Ctx, node: LayoutNode): string {
	const p = props(node);
	const label = text(cx, str(node, 'label'));
	switch (node.kind) {
		case 'backdrop':
		case 'window':
		case 'dialog':
			// Containers nested inside content draw as plain groups; the frame is drawn from the root.
			return `<div class="pv-group" data-kind="${node.kind}">${kids(cx, node)}</div>`;
		case 'stack': {
			const row = str(node, 'direction') === 'row' || str(node, 'direction') === 'horizontal';
			const group = str(node, 'group');
			const align = str(node, 'align');
			return `<div class="pv-stack ${row ? 'pv-row' : 'pv-col'}${group ? ` pv-g-${escapeHtml(group)}` : ''}${align ? ` pv-align-${escapeHtml(align)}` : ''}" data-kind="stack">${kids(cx, node)}</div>`;
		}
		case 'grid':
			return `<div class="pv-grid" data-kind="grid">${kids(cx, node)}</div>`;
		case 'text':
			return renderText(cx, node);
		case 'image': {
			const art = artSvg(p.art, str(node, 'alt') || str(node, 'label'));
			const panel = typeof p.panel === 'string';
			const style = panel ? ` style="background:${colour(p.panel)}${typeof p.ground === 'string' ? `;--ground:${colour(p.ground)}` : ''}"` : '';
			const alt = text(cx, str(node, 'alt') || str(node, 'label') || 'image');
			return `<div class="pv-image${panel ? ' pv-panel' : ''}" data-kind="image"${style}>${art || `<span class="pv-noart" role="img" aria-label="${alt}">${alt}</span>`}</div>`;
		}
		case 'billboard':
			return renderBillboard(cx, node, label);
		case 'meter':
		case 'progress':
			return renderMeter(cx, node, label);
		case 'button': {
			const def = p.default === true ? ' pv-default' : '';
			const name = hotLabel(cx, str(node, 'label'), str(node, 'hotkey'));
			const inner = cx.look === 'dos' ? `&lt; ${name} &gt;` : cx.look === 'win95' && /^(Next|Back)$/.test(str(node, 'label')) ? (str(node, 'label') === 'Next' ? `${name} &gt;` : `&lt; ${name}`) : name;
			return `<button class="pv-button${def}" data-kind="button" data-action="${escapeHtml(str(node, 'action'))}" type="button" tabindex="-1">${inner}</button>`;
		}
		case 'checklist':
		case 'radio':
			return renderChoices(cx, node, label);
		case 'field':
		case 'path': {
			const v = lookup(cx, node.bind);
			const value = typeof v === 'string' || typeof v === 'number' ? String(v) : typeof p.default === 'string' ? p.default : '';
			const browse = node.kind === 'path' && str(node, 'browse') ? `<button class="pv-button" type="button" tabindex="-1">${hotLabel(cx, str(node, 'browse'), str(node, 'hotkey'))}</button>` : '';
			const cls = node.kind === 'path' ? 'pv-path' : 'pv-field';
			return `<div class="${cls}" data-kind="${node.kind}"><span class="pv-caption">${label}</span><span class="pv-input">${escapeHtml(fill(value, cx.vars))}</span>${browse}</div>`;
		}
		case 'custom':
			return cx.renderer === 'canvas' || cx.renderer === 'web' ? `<div class="pv-custom" data-kind="custom">custom content</div>` : '';
		default:
			return `<div class="pv-unknown" data-kind="${escapeHtml(node.kind)}">${escapeHtml(node.kind)}${kids(cx, node)}</div>`;
	}
}

function renderText(cx: Ctx, node: LayoutNode): string {
	const role = str(node, 'role') || 'body';
	const cls = `pv-text pv-${escapeHtml(role)}`;
	if (role === 'step-item' || role === 'task') {
		const state = str(node, 'state') || 'todo';
		const mark = state === 'done' ? '✓' : state === 'current' ? '●' : '○';
		return `<div class="${cls} pv-state-${escapeHtml(state)}" data-kind="text"${state === 'current' ? ' aria-current="step"' : ''}><span class="pv-mark" aria-hidden="true">${mark}</span>${text(cx, str(node, 'text'))}</div>`;
	}
	let content = str(node, 'text') ? fill(str(node, 'text'), cx.vars) : '';
	if (!content && role === 'document' && str(node, 'source')) content = `(${str(node, 'source')})`;
	if (str(node, 'key')) return `<div class="${cls}" data-kind="text"><span class="pv-keycap">${escapeHtml(str(node, 'key'))}</span> ${escapeHtml(content)}</div>`;
	if (role === 'heading' && node.props?.icon) return `<div class="${cls} pv-has-icon" data-kind="text">${artSvg(node.props.icon)}<span>${escapeHtml(content)}</span></div>`;
	if (role === 'error' && !content) return '';
	if (role === 'document' || role === 'summary') return `<pre class="${cls}" data-kind="text"${role === 'document' ? ' tabindex="-1"' : ''}>${escapeHtml(content)}</pre>`;
	return `<div class="${cls}" data-kind="text">${escapeHtml(content)}</div>`;
}

function renderBillboard(cx: Ctx, node: LayoutNode, label: string): string {
	const p = props(node);
	type Cap = { title: string; text: string; art?: unknown };
	let caps: Cap[] = [];
	if (Array.isArray(p.captions)) caps = (p.captions as Record<string, unknown>[]).map((c) => ({ title: String(c.title ?? ''), text: String(c.text ?? ''), art: c.art }));
	else if (Array.isArray(p.items)) caps = (p.items as unknown[]).map((c) => ({ title: '', text: String(c) }));
	else if (str(node, 'text')) caps = [{ title: '', text: str(node, 'text') }];
	const at = lookup(cx, node.bind);
	const i = caps.length ? (typeof at === 'number' ? at : 0) % caps.length : 0;
	const cap = caps[i];
	const art = cap?.art ?? p.art;
	const dots = caps.map((_, d) => `<i class="${d === i ? 'pv-on' : ''}"></i>`).join('');
	return `<div class="pv-billboard" data-kind="billboard" role="group" aria-label="${label}">${art ? `<div class="pv-bb-art">${artSvg(art)}</div>` : ''}<div class="pv-bb-copy"><div class="pv-bb-title">${text(cx, cap?.title ?? '')}</div><div class="pv-bb-text">${text(cx, cap?.text ?? label)}</div></div><div class="pv-bb-dots" aria-hidden="true">${dots}</div></div>`;
}

function renderMeter(cx: Ctx, node: LayoutNode, label: string): string {
	const p = props(node);
	if (node.kind === 'progress' && str(node, 'role') === 'steps') {
		const max = num(node, 'max', 8);
		const value = num(node, 'value', 1);
		const blocks = Array.from({ length: max }, (_, i) => `<i class="${i < value ? 'pv-on' : ''}"></i>`).join('');
		return `<div class="pv-steps" data-kind="progress" role="progressbar" aria-valuemin="0" aria-valuemax="${max}" aria-valuenow="${value}" aria-label="${label}"><div class="pv-steps-text">${text(cx, str(node, 'text') || `Step ${value} of ${max}`)}</div><div class="pv-steps-bar">${blocks}</div></div>`;
	}
	const pct = Math.round(fraction(cx, node) * 100);
	const source = str(node, 'source') || str(node, 'meter') || 'total';
	const caption = typeof p.caption === 'string' ? `<span class="pv-meter-caption">${text(cx, p.caption)}</span>` : '';
	return `<div class="pv-meter" data-kind="${node.kind}" data-source="${escapeHtml(source)}" role="meter" aria-label="${label}" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${pct}"><span class="pv-meter-label">${label}</span>${bar(cx, node, pct)}${caption}</div>`;
}

function renderChoices(cx: Ctx, node: LayoutNode, label: string): string {
	const check = node.kind === 'checklist';
	const items = options(cx, node);
	const v = lookup(cx, node.bind);
	const on = (o: Option, i: number): boolean => {
		if (check) return Array.isArray(v) ? v.includes(o.id) || o.required === true : o.checked === true || o.required === true;
		const dflt = typeof props(node).default === 'string' ? (props(node).default as string) : items[0]?.id;
		return (typeof v === 'string' ? v : dflt) === o.id;
		void i;
	};
	const cards = props(node).cards === true;
	const rows = items
		.map((o, i) => {
			const sel = on(o, i);
			const glyph = cx.look === 'dos' ? (check ? (sel ? '[x]' : '[ ]') : sel ? '(•)' : '( )') : `<span class="pv-${check ? 'box' : 'dot'}${sel ? ' on' : ''}" aria-hidden="true"></span>`;
			const desc = o.description ? `<span class="pv-desc">${text(cx, o.description)}</span>` : '';
			const badges = o.badges?.length ? `<span class="pv-badges">${o.badges.map((b) => `<span class="pv-badge">${text(cx, b.text)}</span>`).join('')}</span>` : '';
			const req = o.required && cards ? ' <span class="pv-req">· required</span>' : '';
			return `<div class="pv-choice${sel ? ' pv-sel' : ''}" role="${check ? 'checkbox' : 'radio'}" aria-checked="${sel}"><span class="pv-glyph">${glyph}</span><span class="pv-choice-body"><span class="pv-choice-label">${text(cx, o.label)}</span>${req}${desc}${badges}</span></div>`;
		})
		.join('');
	const sunken = props(node).sunken === true ? ' pv-sunken' : '';
	return `<div class="pv-choices${cards ? ' pv-cards' : ''}" data-kind="${node.kind}" role="${check ? 'group' : 'radiogroup'}" aria-label="${label}"><div class="pv-caption">${label}</div><div class="pv-choice-list${sunken}">${rows}</div></div>`;
}

// ---------------------------------------------------------------------------------------------
// Frames: the screen around the content

interface Parts {
	backdrop: LayoutNode | null;
	frame: LayoutNode;
	side: LayoutNode[];
	footer: LayoutNode[];
	hints: LayoutNode[];
	status: LayoutNode[];
	body: LayoutNode[];
}

function split(root: LayoutNode): Parts {
	const backdrop = root.kind === 'backdrop' ? root : null;
	const inner = backdrop ? (backdrop.children ?? []) : [root];
	const frame = inner.find((n) => n.kind === 'dialog' || n.kind === 'window') ?? { kind: 'window', children: inner.filter((n) => n.kind !== 'meter' && !regionOf(n)) };
	const all = [...(backdrop?.children ?? []), ...(frame.children ?? [])];
	const status = (backdrop?.children ?? []).filter((n) => n.kind === 'meter' || regionOf(n) === 'status');
	return {
		backdrop,
		frame,
		side: all.filter((n) => regionOf(n) === 'side'),
		footer: all.filter((n) => regionOf(n) === 'footer'),
		hints: all.filter((n) => regionOf(n) === 'hints'),
		status,
		body: (frame.children ?? []).filter((n) => !regionOf(n)),
	};
}

const join = (cx: Ctx, nodes: LayoutNode[]): string => nodes.map((n) => renderNode(cx, n)).join('');

function dosFrame(cx: Ctx, pt: Parts): string {
	const bd = pt.backdrop ? props(pt.backdrop) : {};
	const fillRow = '░'.repeat(120);
	const rows = Array.from({ length: 23 }, () => fillRow).join('\n');
	const fp = props(pt.frame);
	const title = text(cx, String(fp.title ?? bd.title ?? ''));
	const buttons = pt.footer.length ? `<div class="pv-dos-footer">${join(cx, pt.footer)}</div>` : '';
	const bodyHtml = pt.body.map((n) => renderNode(cx, n)).join('') + (pt.status.length ? `<div class="pv-stack pv-col pv-g-meters">${join(cx, pt.status)}</div>` : '');
	return `<div class="pv-stage"><div class="pv-dos-bar"><span>${text(cx, String(bd.title ?? fp.title ?? ''))}</span><span>${text(cx, String(bd.credit ?? 'Liminal Installer'))}</span></div><div class="pv-dos-fill" aria-hidden="true">${rows}</div>${pt.side.length ? `<div class="pv-dos-side">${join(cx, pt.side)}</div>` : ''}<div class="pv-dos-shadow"></div><div class="pv-dos-dialog" role="dialog" aria-label="${title}"><div class="pv-dos-dtitle">[ ${title} ]</div><div class="pv-dos-content">${bodyHtml}</div>${buttons}</div><div class="pv-dos-hints">${join(cx, pt.hints)}</div></div>`;
}

const FILE_ICON = '<svg aria-hidden="true" width="34" height="28" viewBox="0 0 34 28" shape-rendering="crispEdges"><path d="M1.5 5.5 h11 l3 3 h17 v18 h-31 z" fill="#ffff80" stroke="#808000"/><rect x="2" y="10" width="30" height="1" fill="#ffffff"/></svg><svg aria-hidden="true" width="14" height="14" viewBox="0 0 14 14" shape-rendering="crispEdges"><path d="M7 1 L13 7 L10 7 L10 13 L4 13 L4 7 L1 7 Z" fill="#c0c0c0" stroke="#404040"/></svg><svg aria-hidden="true" width="34" height="34" viewBox="0 0 34 34" shape-rendering="crispEdges"><path d="M9.5 3.5 h11 l5 5 v22 h-16 z" fill="#ffffff" stroke="#404040"/><path d="M20.5 3.5 v5 h5" fill="#dfdfdf" stroke="#404040"/><rect x="4.5" y="18.5" width="10" height="10" fill="#c0c0c0" stroke="#404040"/><rect x="7" y="21" width="5" height="3" fill="#ffffff"/></svg>';
const DRIVE_ICON = '<svg aria-hidden="true" width="34" height="14" viewBox="0 0 34 14" shape-rendering="crispEdges"><rect x="0.5" y="2.5" width="33" height="10" fill="#dfdfdf" stroke="#404040"/><rect x="1" y="3" width="32" height="2" fill="#ffffff"/><rect x="4" y="8" width="18" height="1" fill="#808080"/><rect x="26" y="7" width="4" height="2" fill="#00c000"/></svg>';
const ARROW = '<svg aria-hidden="true" width="14" height="14" viewBox="0 0 14 14" shape-rendering="crispEdges"><path d="M7 1 L13 7 L10 7 L10 13 L4 13 L4 7 L1 7 Z" fill="#c0c0c0" stroke="#404040"/></svg>';
const TOTAL_ICON = '<svg aria-hidden="true" width="34" height="34" viewBox="0 0 34 34"><circle cx="20" cy="15" r="12" fill="#e0e0e0" stroke="#404040"/><circle cx="20" cy="15" r="9" fill="none" stroke="#8fd3ff" stroke-width="2"/><circle cx="20" cy="15" r="6" fill="none" stroke="#ffd36b" stroke-width="2"/><circle cx="20" cy="15" r="3" fill="#ffffff" stroke="#404040"/><rect x="2.5" y="20.5" width="11" height="11" fill="#202020" stroke="#000"/><rect x="5" y="21" width="6" height="4" fill="#c0c0c0"/><rect x="4" y="27" width="8" height="4" fill="#ffffff"/></svg>';
const lowIcon = (low: boolean): string => `<svg aria-hidden="true" width="34" height="34" viewBox="0 0 34 34"><path d="M10.5 1.5 h13 l9 9 v13 l-9 9 h-13 l-9 -9 v-13 z" fill="${low ? '#ff4040' : '#d4d4d4'}" stroke="${low ? '#800000' : '#a0a0a0'}"/><text x="17" y="21" text-anchor="middle" font-family="Tahoma, Verdana, sans-serif" font-size="10" font-weight="700" fill="#ffffff">Low</text></svg>`;

/** The classic Windows setup status group: file, total and free-space gauges as lit-block columns. */
function winStatus(cx: Ctx, meters: LayoutNode[]): string {
	const cols = meters.map((m) => {
		const pct = Math.round(fraction(cx, m) * 100);
		const source = str(m, 'source') || 'total';
		const low = isLow(m, pct);
		const lit = Math.round((pct / 100) * 14);
		const colourOf = source === 'free_space' ? (low ? '#c00000' : '#800080') : '#000080';
		const blocks = Array.from({ length: 14 }, (_, b) => `<i style="background:${b < lit ? colourOf : 'transparent'}"></i>`).join('');
		const icons = source === 'file' ? FILE_ICON : source === 'total' ? `${DRIVE_ICON}${ARROW}${TOTAL_ICON}` : `${DRIVE_ICON}<span style="height:14px"></span>${lowIcon(low)}`;
		return `<div class="pv-vmeter" data-kind="meter" data-source="${escapeHtml(source)}" role="meter" aria-label="${text(cx, str(m, 'label'))}" aria-valuenow="${pct}" aria-valuemin="0" aria-valuemax="100"><span class="pv-vgauge pv-sunken">${blocks}</span><span class="pv-vicons">${icons}</span></div>`;
	});
	return cols.length ? `<div class="pv-status" role="group" aria-label="Setup status">${cols.join('')}</div>` : '';
}

function winFrame(cx: Ctx, pt: Parts): string {
	const bd = pt.backdrop ? props(pt.backdrop) : {};
	const fp = props(pt.frame);
	const title = text(cx, String(fp.title ?? ''));
	const side = pt.side.length ? `<div class="pv-w-side" aria-hidden="true">${join(cx, pt.side)}</div>` : '';
	return `<div class="pv-stage"><div class="pv-w-mark">${text(cx, String(bd.title ?? ''))}</div><div class="pv-w-sub">${text(cx, String(bd.subtitle ?? ''))}</div><div class="pv-w-credit">${text(cx, String(bd.credit ?? ''))}</div><div class="pv-w-dialog" role="dialog" aria-label="${title}"><div class="pv-w-title"><span>${title}</span><button class="pv-w-close" type="button" tabindex="-1" aria-label="Close"><svg aria-hidden="true" width="8" height="7" viewBox="0 0 8 7" shape-rendering="crispEdges"><path d="M0 0h2v1h1v1h2V1h1V0h2v1H7v1H6v1H5v1h1v1h1v1h1v1H6V6H5V5H3v1H2v1H0V6h1V5h1V4h1V3H2V2H1V1H0z" fill="#000"/></svg></button></div><div class="pv-w-main">${side}<div class="pv-w-content">${join(cx, pt.body)}</div></div><div class="pv-w-sep"></div><div class="pv-w-footer">${join(cx, pt.footer.flatMap((f) => f.children ?? []))}</div></div>${winStatus(cx, pt.status)}<div class="pv-w-task"><span class="pv-w-start"><svg aria-hidden="true" width="16" height="14" viewBox="0 0 16 14" shape-rendering="crispEdges"><rect x="1" y="1" width="6" height="5" fill="#f00"/><rect x="8" y="1" width="6" height="5" fill="#0a0"/><rect x="1" y="7" width="6" height="5" fill="#00f"/><rect x="8" y="7" width="6" height="5" fill="#ff0"/></svg>Start</span><span class="pv-w-win">${text(cx, String(bd.taskbar ?? 'Setup'))}</span><span class="pv-w-spacer"></span><span class="pv-w-clock">4:15 PM</span></div></div>`;
}

function modernFrame(cx: Ctx, pt: Parts): string {
	const fp = props(pt.frame);
	const icon = fp.icon ? artSvg(fp.icon) : '';
	const sideHtml = pt.side.length ? `<div class="pv-m-side">${join(cx, pt.side)}</div>` : '';
	const footerKids = pt.footer.flatMap((f) => f.children ?? []);
	const notes = footerKids.filter((k) => k.kind === 'text');
	const buttons = footerKids.filter((k) => k.kind !== 'text');
	const caption = typeof fp.caption === 'string' ? `<div class="pv-m-caption">${text(cx, fp.caption)}</div>` : '';
	return `<div class="pv-stage"><div class="pv-m-glow" aria-hidden="true"></div><div class="pv-m-window" role="dialog" aria-label="${text(cx, String(fp.label ?? fp.title ?? ''))}"><div class="pv-m-title"><span class="pv-m-name">${icon}<span>${text(cx, String(fp.title ?? ''))}</span></span><span class="pv-m-close" aria-hidden="true"><svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/></svg></span></div><div class="pv-m-main">${sideHtml}<div class="pv-m-content">${join(cx, pt.body)}</div></div><div class="pv-m-footer"><span class="pv-m-note">${join(cx, notes)}</span><span class="pv-m-buttons">${join(cx, buttons)}</span></div></div>${caption}</div>`;
}

/** Draws a prepared tree (overrides already applied) in the look the options pick. */
export function renderTreeHtml(root: LayoutNode, o: RenderOptions): string {
	const look = lookFor(o.renderer, o.theme, str(root, 'style'));
	const cx: Ctx = { look, renderer: o.renderer, vars: { ...SAMPLE, ...o.vars }, state: o.state ?? {}, components: o.components ?? [] };
	const pt = split(root);
	const frame = look === 'dos' ? dosFrame(cx, pt) : look === 'win95' ? winFrame(cx, pt) : modernFrame(cx, pt);
	const { w, h } = STAGE[look];
	return `<div class="pv-fit" data-w="${w}" data-h="${h}"><div class="pv pv-${look}${o.renderer === 'tui' ? ' pv-tui' : ''}" data-renderer="${escapeHtml(o.renderer)}" data-look="${look}">${frame}</div></div>`;
}

/** The layout as HTML for `renderer`, with its `@renderer` overrides applied first. */
export function renderLayoutHtml(layout: Layout, renderer: string, o: Partial<RenderOptions> = {}): string {
	return renderTreeHtml(resolveForRenderer(layout.root, renderer), { ...o, renderer });
}

/** Scales every stage in `host` to the host's width. Call after inserting preview HTML. */
export function fitStages(host: HTMLElement): void {
	const fit = () => {
		for (const el of host.querySelectorAll<HTMLElement>('.pv-fit')) {
			const w = Number(el.dataset.w) || 1;
			const h = Number(el.dataset.h) || 1;
			const scale = Math.min(1, el.clientWidth / w) || 1;
			const pv = el.firstElementChild as HTMLElement | null;
			if (pv) pv.style.transform = `scale(${scale})`;
			el.style.height = `${h * scale}px`;
		}
	};
	fit();
	if (typeof ResizeObserver !== 'undefined') {
		const ro = new ResizeObserver(fit);
		ro.observe(host);
		(host as HTMLElement & { _pvRo?: ResizeObserver })._pvRo?.disconnect();
		(host as HTMLElement & { _pvRo?: ResizeObserver })._pvRo = ro;
	}
}
