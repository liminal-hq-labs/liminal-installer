// Renders a layout tree to HTML in the tui-retro or canvas style, for the live preview.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { resolveForRenderer } from './layoutEdit';
import type { Layout, LayoutNode } from './types';

export type PreviewStyle = 'retro' | 'canvas';

/** Text-mode renderers use the retro style; everything else is drawn as canvas. */
export function styleFor(renderer: string): PreviewStyle {
	return renderer === 'tui-retro' || renderer === 'tui' ? 'retro' : 'canvas';
}

/** Stand-in values for the `{placeholders}` a running engine would fill. */
export const SAMPLE: Record<string, string> = {
	'product.name': 'Sample Product',
	'product.version': '1.0.0',
	'target.id': 'linux-x86_64',
	'install.path': '/home/you/Sample',
	'install.size': '48 MB',
	'system.free_space': '12.4 GB',
	'system.free_space_after': '12.3 GB',
	'progress.file': 'data/level-03.lvl',
	'progress.percent': '62',
	'plan.summary': '3 components · 41 files · 48 MB',
	'sound.detected': 'Sound Blaster 16',
	'feed.latest': '1.1.0',
	'receipt.installed': '1.0.0',
	'feed.notes': 'Fixes and new levels.',
	'validate.problems': '',
};

export function escapeHtml(s: string): string {
	return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

/** Replaces `{name}` with its sample value, leaving unknown names visible. */
export function fill(text: string): string {
	return text.replace(/\{([a-z_.]+)\}/g, (m, key: string) => SAMPLE[key] ?? m);
}

const str = (node: LayoutNode, key: string): string => {
	const v = node.props?.[key];
	return typeof v === 'string' ? v : '';
};

interface Option {
	id: string;
	label: string;
}

function options(node: LayoutNode): Option[] {
	const raw = node.props?.options ?? node.props?.items;
	if (Array.isArray(raw)) {
		return raw.map((o, i) =>
			typeof o === 'object' && o !== null
				? { id: String((o as Option).id ?? i), label: String((o as Option).label ?? (o as Option).id ?? '') }
				: { id: String(i), label: String(o) },
		);
	}
	return [
		{ id: 'one', label: 'First choice' },
		{ id: 'two', label: 'Second choice' },
	];
}

function bar(style: PreviewStyle, pct: number, warn: boolean): string {
	if (style === 'retro') {
		const cells = 20;
		const on = Math.round((cells * pct) / 100);
		return `<span class="pv-bar-text${warn ? ' pv-warn' : ''}">${'█'.repeat(on)}${'░'.repeat(cells - on)} ${pct}%</span>`;
	}
	return `<span class="pv-bar"><span class="pv-bar-fill${warn ? ' pv-warn' : ''}" style="width:${pct}%"></span></span>`;
}

const METER_SAMPLE: Record<string, number> = { file: 70, total: 62, free_space: 94 };

function renderNode(node: LayoutNode, style: PreviewStyle, renderer: string): string {
	const kids = (node.children ?? []).map((c) => renderNode(c, style, renderer)).join('');
	const label = escapeHtml(fill(str(node, 'label')));
	const retro = style === 'retro';
	switch (node.kind) {
		case 'backdrop':
			return `<div class="pv-backdrop" data-kind="backdrop">${
				str(node, 'title') ? `<div class="pv-backdrop-title">${escapeHtml(fill(str(node, 'title')))}</div>` : ''
			}${kids}</div>`;
		case 'window':
		case 'dialog': {
			const title = str(node, 'title');
			return `<div class="pv-${node.kind}" data-kind="${node.kind}">${
				title ? `<div class="pv-titlebar">${escapeHtml(fill(title))}</div>` : ''
			}<div class="pv-body">${kids}</div></div>`;
		}
		case 'stack': {
			const row = str(node, 'direction') === 'row';
			return `<div class="pv-stack ${row ? 'pv-row' : 'pv-col'}" data-kind="stack">${kids}</div>`;
		}
		case 'grid':
			return `<div class="pv-grid" data-kind="grid">${kids}</div>`;
		case 'text': {
			const role = str(node, 'role') || 'body';
			const src = str(node, 'source');
			const content = str(node, 'text') ? fill(str(node, 'text')) : src ? `(${src})` : '';
			return `<div class="pv-text pv-${escapeHtml(role)}" data-kind="text">${escapeHtml(content)}</div>`;
		}
		case 'image':
			return `<div class="pv-image" data-kind="image" role="img" aria-label="${label}">${retro ? '[ image ]' : label || 'image'}</div>`;
		case 'billboard':
			return `<div class="pv-billboard" data-kind="billboard" aria-label="${label}">${retro ? '▒▒ billboard ▒▒' : 'Billboard'}</div>`;
		case 'meter': {
			const source = str(node, 'source') || 'total';
			const pct = METER_SAMPLE[source] ?? 50;
			return `<div class="pv-meter" data-kind="meter" data-source="${escapeHtml(source)}"><span class="pv-meter-label">${label}</span>${bar(style, pct, false)}</div>`;
		}
		case 'progress':
			return `<div class="pv-meter" data-kind="progress"><span class="pv-meter-label">${label}</span>${bar(style, 40, false)}</div>`;
		case 'button': {
			const def = node.props?.default === true ? ' pv-default' : '';
			return `<button class="pv-button${def}" data-kind="button" type="button" tabindex="-1">${retro ? `[ ${label} ]` : label}</button>`;
		}
		case 'checklist':
			return `<div class="pv-choices" data-kind="checklist" role="group" aria-label="${label}"><div class="pv-caption">${label}</div>${options(node)
				.map((o, i) => `<div>${retro ? (i === 0 ? '[x]' : '[ ]') : `<span class="pv-box${i === 0 ? ' on' : ''}"></span>`} ${escapeHtml(o.label)}</div>`)
				.join('')}</div>`;
		case 'radio':
			return `<div class="pv-choices" data-kind="radio" role="radiogroup" aria-label="${label}"><div class="pv-caption">${label}</div>${options(node)
				.map((o, i) => `<div>${retro ? (i === 0 ? '(•)' : '( )') : `<span class="pv-dot${i === 0 ? ' on' : ''}"></span>`} ${escapeHtml(o.label)}</div>`)
				.join('')}</div>`;
		case 'field':
			return `<div class="pv-field" data-kind="field"><span class="pv-caption">${label}</span><span class="pv-input">${escapeHtml(str(node, 'default'))}</span></div>`;
		case 'path':
			return `<div class="pv-field" data-kind="path"><span class="pv-caption">${label}</span><span class="pv-input">${escapeHtml(SAMPLE['install.path'] ?? '')}</span></div>`;
		case 'custom':
			return renderer === 'canvas' || renderer === 'web'
				? `<div class="pv-custom" data-kind="custom">custom content</div>`
				: '';
		default:
			return `<div class="pv-unknown" data-kind="${escapeHtml(node.kind)}">${escapeHtml(node.kind)}${kids}</div>`;
	}
}

/** The layout as HTML for `renderer`, wrapped in an element carrying its style class. */
export function renderLayoutHtml(layout: Layout, renderer: string): string {
	const style = styleFor(renderer);
	const tree = resolveForRenderer(layout.root, renderer);
	return `<div class="pv pv-${style}" data-renderer="${escapeHtml(renderer)}">${renderNode(tree, style, renderer)}</div>`;
}
