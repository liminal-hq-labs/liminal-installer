// Small runs for the tests: an install of a made-up product through four pages.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { LayoutNode } from '@liminal-hq/installer-renderer';
import type { PageView } from '@liminal-hq/installer-protocol/generated/PageView';
import type { RunEvent } from '@liminal-hq/installer-protocol/generated/RunEvent';
import type { RunInfo } from '@liminal-hq/installer-protocol/generated/RunInfo';

export const info: RunInfo = {
	mode: 'install',
	product: { id: 'hq.test', name: 'Test Product', publisher: 'Liminal HQ', version: '1.0.0' },
	target: 'linux-x86_64:web',
	renderer: 'web',
	theme: 'afterglow',
	look: 'afterglow',
	window: { width: 1024, height: 720 },
	install_dir: '/tmp/test-product',
	components: [{ id: 'app', name: 'Program', required: true }],
};

const button = (
	label: string,
	action: string,
	extra: Record<string, unknown> = {},
): LayoutNode => ({
	kind: 'button',
	props: { label, action, ...extra },
});

/** A page: a window with a heading and the buttons given. */
export function windowOf(label: string, children: LayoutNode[], buttons: LayoutNode[]): LayoutNode {
	return {
		kind: 'window',
		props: { title: '{product.name} Setup', label, style: 'afterglow' },
		children: [
			{ kind: 'stack', props: { direction: 'column', gap: 1 }, children },
			{
				kind: 'stack',
				props: { direction: 'row', gap: 2, align: 'end', region: 'footer' },
				children: buttons,
			},
		],
	};
}

export const welcome: LayoutNode = windowOf(
	'Welcome',
	[{ kind: 'text', props: { role: 'heading', text: 'Welcome to {product.name}' } }],
	[button('Next', 'next', { default: true }), button('Cancel', 'cancel')],
);

/** A page whose Next is switched off until the licence is accepted. */
export const licence: LayoutNode = windowOf(
	'Licence',
	[
		{
			kind: 'radio',
			bind: 'licence.accepted',
			props: {
				label: 'Licence acceptance',
				options: [
					{ id: 'yes', label: 'I accept' },
					{ id: 'no', label: 'I do not accept' },
				],
			},
		},
	],
	[
		button('Back', 'back'),
		button('Next', 'next', { default: true, enabled_when: "state.licence.accepted == 'yes'" }),
		button('Cancel', 'cancel'),
	],
);

export const copying: LayoutNode = windowOf(
	'Installing',
	[{ kind: 'meter', props: { label: 'Overall progress', source: 'total' } }],
	[button('Cancel', 'cancel')],
);

export const done: LayoutNode = windowOf(
	'Done',
	[{ kind: 'text', props: { role: 'heading', text: '{product.name} is ready' } }],
	[button('Finish', 'finish', { default: true })],
);

export function page(
	step: string,
	kind: PageView['kind'],
	index: number,
	layout: LayoutNode,
	extra: Partial<PageView> = {},
): RunEvent {
	return {
		type: 'page',
		page: {
			step,
			kind,
			index,
			layout,
			state: {},
			vars: { 'product.name': 'Test Product', 'install.path': '/tmp/test-product' },
			problems: [],
			disabled: [],
			...extra,
		},
	};
}
