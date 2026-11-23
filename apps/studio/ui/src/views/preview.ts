// The Dry-run screen: the plan, the problems and each step's screen in order.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Ctx } from '../app';
import { h } from '../lib/dom';
import { formatOp, screenshotSrc } from '../lib/format';
import { fitStages, lookFor, renderTreeHtml } from '../lib/previewRender';

export function previewView(ctx: Ctx): HTMLElement {
	const s = ctx.state;
	const targets = s.summary!.targets;
	const pv = s.preview;
	const shown = pv?.steps[s.previewStep];
	const go = (delta: number) => {
		if (!pv) return;
		let i = s.previewStep;
		do i += delta;
		while (pv.steps[i]?.skipped);
		if (i >= 0 && i < pv.steps.length) {
			s.previewStep = i;
			ctx.render();
		}
	};
	const targetInfo = targets.find((t) => t.full_id === s.previewTarget);
	const stale = Boolean(pv && pv.target !== s.previewTarget);
	const rich = h('div', {
		class: 'preview-host',
		'aria-label': shown ? `Screen of step ${shown.id}` : 'Screen',
	});
	if (pv && shown?.layout_tree) {
		const renderer = pv.renderer ?? targetInfo?.renderer ?? 'canvas';
		rich.innerHTML = renderTreeHtml(shown.layout_tree, {
			renderer,
			theme: pv.theme ?? targetInfo?.theme,
			vars: pv.vars,
			state: shown.state,
			components: s.summary!.components,
		});
		fitStages(rich);
	}
	const skippedCount = pv?.steps.filter((st) => st.skipped).length ?? 0;
	return h(
		'div',
		{ class: 'screen' },
		h(
			'div',
			{ class: 'screen-head' },
			h(
				'div',
				{},
				h('h1', {}, 'Dry run'),
				h(
					'p',
					{ class: 'muted' },
					'The plan and the screens a person would see, without changing anything.',
				),
			),
		),
		h(
			'div',
			{ class: 'toolbar' },
			h(
				'select',
				{
					'aria-label': 'Target',
					on: {
						change: (e) => {
							const t = (e.target as HTMLSelectElement).value;
							s.previewTarget = t;
							void ctx.run(() => ctx.runPreview(t));
						},
					},
				},
				targets.map((t) =>
					h(
						'option',
						{ value: t.full_id, selected: t.full_id === s.previewTarget },
						`${t.id} · ${t.renderer}`,
					),
				),
			),
			h(
				'button',
				{
					class: 'btn primary',
					on: { click: () => void ctx.run(() => ctx.runPreview(s.previewTarget)) },
				},
				'Run preview',
			),
			pv
				? h(
						'span',
						{ class: stale ? 'chip warn' : 'dim small' },
						stale
							? `Stale: showing ${pv.target}`
							: `Showing ${pv.target}${pv.theme ? ` · theme ${pv.theme}` : ''} · ${pv.renderer ?? ''} look: ${lookFor(pv.renderer ?? 'canvas', pv.theme)}`,
					)
				: null,
		),
		!pv
			? h('p', { class: 'muted' }, 'Run a preview to see the plan. Editing the project clears it.')
			: h(
					'div',
					{ class: 'preview-grid' },
					h(
						'section',
						{ class: 'card', 'aria-label': 'Steps' },
						h('h2', {}, 'Steps'),
						h(
							'ol',
							{ class: 'steps' },
							pv.steps.flatMap((st, i) =>
								st.skipped && !ctx.settings().showSkippedSteps
									? []
									: [
											h(
												'li',
												{
													class: `${i === s.previewStep ? 'active' : ''} ${st.skipped ? 'skipped' : ''}`,
												},
												h(
													'button',
													{
														class: 'linklike',
														disabled: st.skipped,
														on: {
															click: () => {
																s.previewStep = i;
																ctx.render();
															},
														},
													},
													st.id,
												),
												h(
													'span',
													{ class: 'dim small' },
													` ${st.kind}${st.skipped ? ' · skipped for this target' : ''}`,
												),
											),
										],
							),
						),
					),
					h(
						'section',
						{ class: 'card', 'aria-label': 'Screen' },
						h(
							'div',
							{ class: 'toolbar' },
							h('button', { class: 'btn', on: { click: () => go(-1) } }, '← Back'),
							h('strong', {}, shown?.id ?? ''),
							h('button', { class: 'btn', on: { click: () => go(1) } }, 'Next →'),
						),
						shown?.layout_tree
							? rich
							: h('p', { class: 'muted' }, shown ? 'This step has no screen.' : ''),
						skippedCount > 0
							? h(
									'p',
									{ class: 'dim small' },
									`${skippedCount} step${skippedCount === 1 ? ' is' : 's are'} skipped for this target and have no screen.`,
								)
							: null,
						shown?.screenshot
							? h(
									'details',
									{ class: 'text-version' },
									h('summary', {}, 'Text version'),
									h('img', {
										class: 'shot',
										src: screenshotSrc(shown.screenshot),
										alt: `Text version of step ${shown.id}`,
									}),
								)
							: null,
					),
					h(
						'section',
						{ class: 'card', 'aria-label': 'Plan' },
						h('h2', {}, `Plan · ${pv.plan.ops.length} operations`),
						h(
							'ol',
							{ class: 'ops' },
							pv.plan.ops.map((op) => {
								const f = formatOp(op);
								return h(
									'li',
									{},
									h('code', { class: 'op-name' }, f.name),
									' ',
									h('span', { class: 'mono small' }, f.detail),
								);
							}),
						),
						h('h2', {}, `Problems · ${pv.problems.length}`),
						pv.problems.length === 0
							? h('p', { class: 'ok' }, 'None.')
							: h(
									'ul',
									{ class: 'problems' },
									pv.problems.map((p) =>
										h(
											'li',
											{ class: p.severity },
											h('span', { class: 'mono small' }, p.scope),
											' ',
											p.message,
										),
									),
								),
					),
				),
	);
}
