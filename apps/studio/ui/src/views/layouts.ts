// The Layouts screen: a node tree editor, per-renderer overrides and a live preview.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Ctx } from '../app';
import { h } from '../lib/dom';
import {
	NODE_KINDS,
	RENDERERS,
	addChild,
	describeNode,
	effectiveNode,
	flattenTree,
	getNode,
	hasOverride,
	moveNode,
	parsePropValue,
	propToText,
	removeNode,
	setBind,
	setKind,
	setProp,
	toggleOverride,
} from '../lib/layoutEdit';
import { fitStages, lookFor, renderLayoutHtml } from '../lib/previewRender';
import type { Layout } from '../lib/types';

const same = (a: number[], b: number[]) => a.length === b.length && a.every((v, i) => v === b[i]);

function tree(ctx: Ctx, layout: Layout): HTMLElement {
	const s = ctx.state;
	const save = (next: Layout, label: string) => void ctx.run(() => ctx.saveLayout(next, label));
	const node = getNode(layout.root, s.layoutPath);
	let addKind = 'text';
	return h(
		'div',
		{},
		h(
			'ul',
			{ class: 'tree', role: 'tree', 'aria-label': 'Layout nodes' },
			flattenTree(layout.root).map(({ path, depth, node: n }) =>
				h(
					'li',
					{
						role: 'treeitem',
						'aria-selected': same(path, s.layoutPath),
						class: same(path, s.layoutPath) ? 'selected' : '',
						style: `padding-left:${8 + depth * 16}px`,
						tabindex: 0,
						on: {
							click: () => {
								s.layoutPath = path;
								ctx.render();
							},
							keydown: (e) => {
								if ((e as KeyboardEvent).key === 'Enter') {
									s.layoutPath = path;
									ctx.render();
								}
							},
						},
					},
					h('span', { class: 'kind' }, describeNode(n)),
					Object.keys(n.overrides ?? {}).map((r) =>
						h('span', { class: 'chip', title: `Has an override for ${r}` }, r),
					),
				),
			),
		),
		h(
			'div',
			{ class: 'toolbar' },
			h(
				'select',
				{
					'aria-label': 'Node kind to add',
					on: {
						change: (e) => {
							addKind = (e.target as HTMLSelectElement).value;
						},
					},
				},
				NODE_KINDS.map((k) => h('option', { value: k, selected: k === 'text' }, k)),
			),
			h(
				'button',
				{
					class: 'btn',
					disabled: !node,
					on: {
						click: () => {
							const r = addChild(layout, s.layoutPath, addKind);
							s.layoutPath = r.path;
							save(r.layout, `Add ${addKind} to ${layout.id}`);
						},
					},
				},
				'Add child',
			),
			h(
				'button',
				{
					class: 'btn ghost',
					disabled: s.layoutPath.length === 0,
					'aria-label': 'Move up',
					on: {
						click: () => {
							const r = moveNode(layout, s.layoutPath, -1);
							s.layoutPath = r.path;
							save(r.layout, `Move node in ${layout.id}`);
						},
					},
				},
				'↑',
			),
			h(
				'button',
				{
					class: 'btn ghost',
					disabled: s.layoutPath.length === 0,
					'aria-label': 'Move down',
					on: {
						click: () => {
							const r = moveNode(layout, s.layoutPath, 1);
							s.layoutPath = r.path;
							save(r.layout, `Move node in ${layout.id}`);
						},
					},
				},
				'↓',
			),
			h(
				'button',
				{
					class: 'btn danger',
					disabled: s.layoutPath.length === 0,
					on: {
						click: () => {
							const p = s.layoutPath;
							s.layoutPath = p.slice(0, -1);
							save(removeNode(layout, p), `Remove node from ${layout.id}`);
						},
					},
				},
				'Remove',
			),
		),
	);
}

function inspector(ctx: Ctx, layout: Layout): HTMLElement {
	const s = ctx.state;
	const base = getNode(layout.root, s.layoutPath);
	if (!base) return h('p', { class: 'muted' }, 'Select a node.');
	const r = s.overrideRenderer;
	const overridden = r ? hasOverride(base, r) : false;
	const node = effectiveNode(base, overridden ? r : null);
	const editR = overridden ? r : null;
	const save = (next: Layout, label: string) => void ctx.run(() => ctx.saveLayout(next, label));
	const props = Object.entries(node.props ?? {});
	let newKey = '';
	return h(
		'div',
		{ class: 'editor' },
		h('h3', {}, 'Node'),
		h(
			'label',
			{ class: 'field' },
			h('span', { class: 'field-label' }, 'Edit for'),
			h(
				'select',
				{
					on: {
						change: (e) => {
							const v = (e.target as HTMLSelectElement).value;
							s.overrideRenderer = v || null;
							ctx.render();
						},
					},
				},
				h('option', { value: '', selected: !r }, 'All renderers (base)'),
				RENDERERS.map((x) => h('option', { value: x, selected: x === r }, x)),
			),
		),
		r
			? h(
					'label',
					{ class: 'check' },
					h('input', {
						type: 'checkbox',
						checked: overridden,
						on: {
							change: (e) =>
								save(
									toggleOverride(layout, s.layoutPath, r, (e.target as HTMLInputElement).checked),
									`${(e.target as HTMLInputElement).checked ? 'Add' : 'Remove'} ${r} override in ${layout.id}`,
								),
						},
					}),
					`Override this node for ${r}`,
				)
			: null,
		h(
			'label',
			{ class: 'field' },
			h('span', { class: 'field-label' }, 'Kind'),
			h(
				'select',
				{
					on: {
						change: (e) =>
							save(
								setKind(layout, s.layoutPath, (e.target as HTMLSelectElement).value, editR),
								`Change node kind in ${layout.id}`,
							),
					},
				},
				NODE_KINDS.map((k) => h('option', { value: k, selected: k === node.kind }, k)),
			),
		),
		h(
			'label',
			{ class: 'field' },
			h('span', { class: 'field-label' }, 'Bind'),
			h('input', {
				type: 'text',
				class: 'mono',
				value: node.bind ?? '',
				on: {
					change: (e) =>
						save(
							setBind(layout, s.layoutPath, (e.target as HTMLInputElement).value.trim(), editR),
							`Bind node in ${layout.id}`,
						),
				},
			}),
		),
		h('h3', {}, 'Properties'),
		props.map(([k, v]) =>
			h(
				'div',
				{ class: 'prop-row' },
				h('label', { class: 'prop-key mono small', for: `prop-${k}` }, k),
				h('input', {
					id: `prop-${k}`,
					type: 'text',
					class: 'mono',
					value: propToText(v),
					on: {
						change: (e) =>
							save(
								setProp(
									layout,
									s.layoutPath,
									k,
									parsePropValue((e.target as HTMLInputElement).value, v),
									editR,
								),
								`Edit ${k} in ${layout.id}`,
							),
					},
				}),
				h(
					'button',
					{
						class: 'btn ghost small',
						'aria-label': `Remove ${k}`,
						on: {
							click: () =>
								save(
									setProp(layout, s.layoutPath, k, undefined, editR),
									`Remove ${k} in ${layout.id}`,
								),
						},
					},
					'×',
				),
			),
		),
		h(
			'div',
			{ class: 'prop-row' },
			h('input', {
				type: 'text',
				class: 'mono',
				placeholder: 'new property',
				'aria-label': 'New property name',
				on: {
					input: (e) => {
						newKey = (e.target as HTMLInputElement).value.trim();
					},
				},
			}),
			h(
				'button',
				{
					class: 'btn',
					on: {
						click: () => {
							if (newKey)
								save(
									setProp(layout, s.layoutPath, newKey, 'value', editR),
									`Add ${newKey} in ${layout.id}`,
								);
						},
					},
				},
				'Add',
			),
		),
	);
}

function layoutPicker(ctx: Ctx): HTMLElement {
	const s = ctx.state;
	const ids = s.summary!.layouts;
	const groups = [...new Set(ids.map((id) => (id.includes('/') ? id.split('/')[0]! : '')))].sort(
		(a, b) => (a === '' ? -1 : b === '' ? 1 : a.localeCompare(b)),
	);
	return h(
		'label',
		{ class: 'field inline' },
		h('span', { class: 'field-label' }, 'Layout'),
		h(
			'select',
			{
				'aria-label': 'Layout',
				on: {
					change: (e) => void ctx.run(() => ctx.loadLayout((e.target as HTMLSelectElement).value)),
				},
			},
			groups.map((g) =>
				h(
					'optgroup',
					{ label: g === '' ? 'Shared (plain fallback)' : `Theme ${g}` },
					ids
						.filter((id) => (id.includes('/') ? id.split('/')[0] : '') === g)
						.map((id) =>
							h(
								'option',
								{ value: id, selected: id === s.layoutId },
								id.includes('/') ? id.split('/')[1]! : id,
							),
						),
				),
			),
		),
	);
}

export function layoutView(ctx: Ctx): HTMLElement {
	const s = ctx.state;
	const layout = s.layout;
	if (!layout) return h('p', { class: 'muted' }, 'Loading…');
	// A theme's own layout is previewed by a renderer that theme supports.
	const themeId = s.layoutId.includes('/') ? s.layoutId.split('/')[0]! : null;
	const theme = s.summary!.themes.find((t) => t.id === themeId) ?? null;
	if (theme && !theme.renderers.includes(s.previewRenderer))
		s.previewRenderer = theme.renderers[0] ?? s.previewRenderer;
	const pv = h('div', { class: 'preview-host' });
	pv.innerHTML = renderLayoutHtml(layout, s.previewRenderer, {
		theme: themeId,
		components: s.summary!.components,
	});
	fitStages(pv);
	return h(
		'div',
		{ class: 'screen' },
		h(
			'div',
			{ class: 'screen-head' },
			h(
				'div',
				{},
				h('h1', {}, 'Layouts'),
				h(
					'p',
					{ class: 'muted' },
					'A tree of nodes. A renderer uses its override where one exists and the base node otherwise.',
				),
			),
		),
		layoutPicker(ctx),
		h(
			'div',
			{ class: 'layout-grid' },
			h('section', { class: 'card', 'aria-label': 'Node tree' }, tree(ctx, layout)),
			h('section', { class: 'card', 'aria-label': 'Node properties' }, inspector(ctx, layout)),
			h(
				'section',
				{ class: 'card', 'aria-label': 'Live preview' },
				h(
					'div',
					{ class: 'toolbar' },
					h(
						'label',
						{ class: 'field inline' },
						h('span', { class: 'field-label' }, 'Preview as'),
						h(
							'select',
							{
								on: {
									change: (e) => {
										s.previewRenderer = (e.target as HTMLSelectElement).value;
										ctx.render();
									},
								},
							},
							RENDERERS.map((r) => h('option', { value: r, selected: r === s.previewRenderer }, r)),
						),
					),
					h(
						'span',
						{ class: 'chip' },
						`${lookFor(s.previewRenderer, themeId, layout.root.props?.style as string | undefined)} look`,
					),
				),
				pv,
			),
		),
	);
}
