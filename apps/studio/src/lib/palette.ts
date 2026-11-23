// The add-node palette: every node kind of SPEC section 6, grouped, with a sensible default and presets for the usual combinations.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import type { Scope } from './stage';
import type { LayoutNode } from './types';

export type PaletteGroup = 'containers' | 'content' | 'controls' | 'status' | 'special';

export interface PaletteItem {
	id: string;
	group: PaletteGroup;
	label: () => string;
	hint: () => string;
	/** Fresh nodes each time, never shared between insertions. */
	build: () => LayoutNode[];
	/** Where it belongs by default: the theme's stage (every page) or the page. */
	scope: Scope;
	/** The kinds it adds, for a test and for the tree. */
	kinds: string[];
}

export const GROUPS: { id: PaletteGroup; label: () => string }[] = [
	{ id: 'containers', label: () => t('palette.group.containers') },
	{ id: 'content', label: () => t('palette.group.content') },
	{ id: 'controls', label: () => t('palette.group.controls') },
	{ id: 'status', label: () => t('palette.group.status') },
	{ id: 'special', label: () => t('palette.group.special') },
];

/** A small pixel picture used as the default for side art. */
const SIDE_ART = {
	width: 12,
	height: 14,
	scale: 8,
	rects: [
		[1, 1, 10, 7, '#c0c0c0'],
		[2, 2, 8, 5, '#0000aa'],
		[3, 3, 6, 1, '#ffffff'],
		[4, 9, 4, 1, '#808080'],
		[2, 10, 8, 2, '#c0c0c0'],
	],
};

export const METER_SOURCES = ['file', 'total', 'free_space'] as const;
export type MeterSource = (typeof METER_SOURCES)[number];

/** The caption a meter of each kind starts with. */
export const meterLabel = (source: string): string =>
	source === 'file'
		? t('palette.meter.file')
		: source === 'total'
			? t('palette.meter.total')
			: t('palette.meter.free');

/** A status meter of one kind. On the backdrop by default, as the classic setups drew them. */
export function meterNode(
	source: MeterSource,
	place: 'backdrop' | 'dialog' = 'backdrop',
): LayoutNode {
	const base: Record<string, unknown> = {
		label: meterLabel(source),
		source,
		place,
	};
	if (source === 'file') base.caption = '{progress.file}';
	if (source === 'total') base.caption = '{progress.percent}%';
	if (source === 'free_space') {
		base.caption = '{system.free_space_after}';
		base.low_below = 0.05;
		base.warn_label = t('preview.low');
	}
	if (place === 'backdrop') base.region = 'status';
	return { kind: 'meter', bind: `engine.meter.${source}`, props: base };
}

const button = (label: string, action: string, hotkey?: string, dflt = false): LayoutNode => ({
	kind: 'button',
	props: { label, action, ...(hotkey ? { hotkey } : {}), ...(dflt ? { default: true } : {}) },
});

const item = (
	id: string,
	group: PaletteGroup,
	scope: Scope,
	kinds: string[],
	label: () => string,
	hint: () => string,
	build: () => LayoutNode[],
): PaletteItem => ({ id, group, scope, kinds, label, hint, build });

export const PALETTE: PaletteItem[] = [
	item(
		'stack-column',
		'containers',
		'page',
		['stack'],
		() => t('palette.stackColumn.label'),
		() => t('palette.stackColumn.hint'),
		() => [{ kind: 'stack', props: { direction: 'column', gap: 1 } }],
	),
	item(
		'stack-row',
		'containers',
		'page',
		['stack'],
		() => t('palette.stackRow.label'),
		() => t('palette.stackRow.hint'),
		() => [{ kind: 'stack', props: { direction: 'row', gap: 1 } }],
	),
	item(
		'grid',
		'containers',
		'page',
		['grid'],
		() => t('palette.grid.label'),
		() => t('palette.grid.hint'),
		() => [{ kind: 'grid', props: { columns: 2, gap: 1 } }],
	),
	item(
		'dialog',
		'containers',
		'page',
		['dialog'],
		() => t('palette.dialog.label'),
		() => t('palette.dialog.hint'),
		() => [
			{
				kind: 'dialog',
				props: { title: t('palette.dialog.title'), label: t('palette.dialog.title') },
			},
		],
	),
	item(
		'window',
		'containers',
		'page',
		['window'],
		() => t('palette.window.label'),
		() => t('palette.window.hint'),
		() => [
			{
				kind: 'window',
				props: { title: t('palette.dialog.title'), label: t('palette.dialog.title') },
			},
		],
	),
	item(
		'backdrop',
		'containers',
		'page',
		['backdrop'],
		() => t('palette.backdrop.label'),
		() => t('palette.backdrop.hint'),
		() => [
			{
				kind: 'backdrop',
				props: {
					title: '{product.name}',
					subtitle: '',
					credit: t('preview.credit'),
					taskbar: '{product.name} Setup',
				},
			},
		],
	),
	item(
		'text',
		'content',
		'page',
		['text'],
		() => t('palette.text.label'),
		() => t('palette.text.hint'),
		() => [{ kind: 'text', props: { text: t('palette.text.sample'), role: 'body' } }],
	),
	item(
		'heading',
		'content',
		'page',
		['text'],
		() => t('palette.heading.label'),
		() => t('palette.heading.hint'),
		() => [{ kind: 'text', props: { text: t('palette.heading.sample'), role: 'heading' } }],
	),
	item(
		'image',
		'content',
		'page',
		['image'],
		() => t('palette.image.label'),
		() => t('palette.image.hint'),
		() => [
			{
				kind: 'image',
				props: { label: t('palette.image.sample'), art: structuredClone(SIDE_ART) },
			},
		],
	),
	item(
		'side-art',
		'content',
		'page',
		['stack', 'image'],
		() => t('palette.sideArt.label'),
		() => t('palette.sideArt.hint'),
		() => [
			{
				kind: 'stack',
				props: { direction: 'column', gap: 0, region: 'side' },
				children: [
					{
						kind: 'image',
						props: {
							label: t('palette.image.sample'),
							art: structuredClone(SIDE_ART),
							panel: '#008080',
							ground: '#004040',
						},
					},
				],
			},
		],
	),
	item(
		'billboard',
		'content',
		'page',
		['billboard'],
		() => t('palette.billboard.label'),
		() => t('palette.billboard.hint'),
		() => [
			{
				kind: 'billboard',
				props: {
					label: t('palette.billboard.sample'),
					captions: [
						{ title: t('palette.billboard.title1'), text: t('palette.billboard.text1') },
						{ title: t('palette.billboard.title2'), text: t('palette.billboard.text2') },
					],
				},
			},
		],
	),
	item(
		'steps',
		'content',
		'page',
		['progress'],
		() => t('palette.steps.label'),
		() => t('palette.steps.hint'),
		() => [
			{
				kind: 'progress',
				props: { label: t('palette.steps.sample'), role: 'steps', value: 1, max: 6 },
			},
		],
	),
	item(
		'button',
		'controls',
		'page',
		['button'],
		() => t('palette.button.label'),
		() => t('palette.button.hint'),
		() => [button(t('palette.button.sample'), 'next')],
	),
	item(
		'button-row',
		'controls',
		'page',
		['stack', 'button'],
		() => t('palette.buttonRow.label'),
		() => t('palette.buttonRow.hint'),
		() => [
			{
				kind: 'stack',
				props: { direction: 'row', gap: 1, align: 'end', region: 'footer' },
				children: [
					button('Back', 'back', 'B'),
					button('Next', 'next', 'N', true),
					button('Cancel', 'cancel'),
				],
			},
		],
	),
	item(
		'checklist',
		'controls',
		'page',
		['checklist'],
		() => t('palette.checklist.label'),
		() => t('palette.checklist.hint'),
		() => [
			{
				kind: 'checklist',
				bind: 'components.selected',
				props: { label: t('palette.checklist.sample'), items: 'components', sunken: true },
			},
		],
	),
	item(
		'radio',
		'controls',
		'page',
		['radio'],
		() => t('palette.radio.label'),
		() => t('palette.radio.hint'),
		() => [
			{
				kind: 'radio',
				bind: 'choice',
				props: {
					label: t('palette.radio.sample'),
					default: 'a',
					options: [
						{ id: 'a', label: t('palette.radio.optionA') },
						{ id: 'b', label: t('palette.radio.optionB') },
					],
				},
			},
		],
	),
	item(
		'field',
		'controls',
		'page',
		['field'],
		() => t('palette.field.label'),
		() => t('palette.field.hint'),
		() => [
			{ kind: 'field', bind: 'name', props: { label: t('palette.field.sample'), default: '' } },
		],
	),
	item(
		'path',
		'controls',
		'page',
		['path'],
		() => t('palette.path.label'),
		() => t('palette.path.hint'),
		() => [
			{
				kind: 'path',
				bind: 'install.path',
				props: {
					label: t('palette.path.sample'),
					mode: 'directory',
					browse: 'Browse…',
					hotkey: 'r',
				},
			},
		],
	),
	item(
		'meter-file',
		'status',
		'stage',
		['meter'],
		() => t('palette.meterFile.label'),
		() => t('palette.meterFile.hint'),
		() => [meterNode('file')],
	),
	item(
		'meter-total',
		'status',
		'stage',
		['meter'],
		() => t('palette.meterTotal.label'),
		() => t('palette.meterTotal.hint'),
		() => [meterNode('total')],
	),
	item(
		'meter-free',
		'status',
		'stage',
		['meter'],
		() => t('palette.meterFree.label'),
		() => t('palette.meterFree.hint'),
		() => [meterNode('free_space')],
	),
	item(
		'status-meters',
		'status',
		'stage',
		['meter', 'meter', 'meter'],
		() => t('palette.statusMeters.label'),
		() => t('palette.statusMeters.hint'),
		() =>
			METER_SOURCES.map((s) => {
				const n = meterNode(s);
				n.props = { ...n.props, hide_on: ['finish'] };
				return n;
			}),
	),
	item(
		'hints',
		'special',
		'page',
		['stack', 'text'],
		() => t('palette.hints.label'),
		() => t('palette.hints.hint'),
		() => [
			{
				kind: 'stack',
				props: { direction: 'row', gap: 2, region: 'hints' },
				children: [
					{ kind: 'text', props: { key: 'Tab', text: t('palette.hints.move') } },
					{ kind: 'text', props: { key: 'Enter', text: t('palette.hints.choose') } },
				],
			},
		],
	),
	item(
		'slot',
		'special',
		'stage',
		['slot'],
		() => t('palette.slot.label'),
		() => t('palette.slot.hint'),
		() => [{ kind: 'slot', props: { label: t('palette.slot.sample') } }],
	),
	item(
		'custom',
		'special',
		'page',
		['custom'],
		() => t('palette.custom.label'),
		() => t('palette.custom.hint'),
		() => [{ kind: 'custom', props: { label: t('palette.custom.sample') } }],
	),
];

export const paletteItem = (id: string): PaletteItem | undefined =>
	PALETTE.find((p) => p.id === id);
