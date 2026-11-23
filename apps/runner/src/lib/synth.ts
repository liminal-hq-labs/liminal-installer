// The screens the runner makes for itself: progress with no flow, the end of a run, an error.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { RunInfo } from '@liminal-hq/installer-protocol/generated/RunInfo';
import {
	composeScreen,
	resolveForRenderer,
	type LayoutNode,
	type StepInfo,
} from '@liminal-hq/installer-renderer';
import { t } from '../i18n/messages';

export type SynthKind = 'status' | 'done' | 'failed';

const text = (value: string, role: string): LayoutNode => ({
	kind: 'text',
	props: { role, text: value },
});

/** The page, as a window the look draws like any other, inside the theme's stage when it has one. */
export function synthScreen(kind: SynthKind, info: RunInfo, message = ''): LayoutNode {
	const name = info.product.name;
	const mode = info.mode;
	const heading =
		kind === 'status'
			? t(`runner.working.${mode}`, { name })
			: kind === 'done'
				? t(`runner.done.${mode}`, { name })
				: t('runner.failed.title');
	const body =
		kind === 'status'
			? t('runner.working.body')
			: kind === 'done'
				? t('runner.done.body')
				: t('runner.failed.body', { message });
	const lead: LayoutNode = {
		kind: 'stack',
		props: { direction: 'column', gap: 0, group: 'lead' },
		children: [text(heading, 'heading'), text(body, 'body')],
	};
	const content: LayoutNode[] = [lead];
	if (kind === 'status') {
		content.push({
			kind: 'progress',
			props: { label: t('runner.progress'), source: 'total', caption: '{progress.percent}%' },
		});
		content.push(text('{progress.file}', 'note'));
	}
	const footer: LayoutNode = {
		kind: 'stack',
		props: { direction: 'row', gap: 2, align: 'end', region: 'footer' },
		children: [
			kind === 'status'
				? {
						kind: 'button',
						props: { label: t('runner.cancel'), action: 'cancel' },
					}
				: {
						kind: 'button',
						props: { label: t('runner.close'), action: 'close', default: true },
					},
		],
	};
	const page: LayoutNode = {
		kind: 'window',
		props: {
			title: t(`runner.title.${mode}`, { name }),
			label: heading,
			style: info.look,
		},
		children: [
			{ kind: 'stack', props: { direction: 'column', gap: 1 }, children: content },
			footer,
		],
	};
	const step: StepInfo = { id: kind, kind: kind === 'status' ? 'commit' : 'finish' };
	const stage = info.stage ? resolveForRenderer(info.stage as LayoutNode, info.renderer) : null;
	return composeScreen(stage, page, step).root;
}
