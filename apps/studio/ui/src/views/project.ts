// The Project screen: targets, components, themes, flows and checks at a glance.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Ctx } from '../app';
import { h } from '../lib/dom';
import { errorCount, targetStatus } from '../lib/format';

const STATUS_LABEL = { ok: 'Checks pass', warning: 'Needs review', error: 'Has errors' } as const;

export function projectView(ctx: Ctx): HTMLElement {
	const s = ctx.state;
	const sum = s.summary!;
	const required = sum.components.filter((c) => c.required).length;
	const stat = (label: string, value: string | number, note: string) =>
		h('div', { class: 'card stat' }, h('div', { class: 'dim small' }, label), h('div', { class: 'stat-value' }, String(value)), h('div', { class: 'muted small' }, note));
	const errors = errorCount(s.problems);
	return h(
		'div',
		{ class: 'screen' },
		h('div', { class: 'screen-head' }, h('div', {}, h('h1', {}, sum.name), h('p', { class: 'muted' }, `${sum.id} · ${sum.publisher} · version ${sum.version}`)), h('button', { class: 'btn primary', on: { click: () => void ctx.run(async () => { await ctx.runPreview(s.previewTarget); ctx.go('preview'); }) } }, 'Dry-run a target')),
		h(
			'div',
			{ class: 'stats' },
			stat('Targets', sum.targets.length, [...new Set(sum.targets.map((t) => t.id))].join(' · ')),
			stat('Themes', sum.themes.length, sum.themes.map((t) => t.name).join(' · ')),
			stat('Components', sum.components.length, `${required} required · ${sum.components.length - required} optional`),
			stat('Checks', errors === 0 ? 'Pass' : `${errors} error${errors === 1 ? '' : 's'}`, `${s.problems.length} finding${s.problems.length === 1 ? '' : 's'}`),
		),
		h(
			'section',
			{ class: 'card', 'aria-labelledby': 'targets-h' },
			h('h2', { id: 'targets-h' }, 'Targets'),
			h(
				'table',
				{},
				h('thead', {}, h('tr', {}, ['Target', 'Renderer', 'Theme', 'Outputs', 'Components', 'Status'].map((c) => h('th', { scope: 'col' }, c)))),
				h(
					'tbody',
					{},
					sum.targets.map((t) => {
						const st = targetStatus(s.problems, t.full_id);
						return h(
							'tr',
							{},
							h('td', {}, t.id),
							h('td', { class: 'mono violet' }, t.renderer),
							h('td', {}, t.theme ?? '—'),
							h('td', { class: 'muted' }, t.outputs.join(' · ')),
							h('td', { class: 'muted' }, t.components.join(', ')),
							h('td', {}, h('span', { class: `pill ${st}` }, STATUS_LABEL[st])),
						);
					}),
				),
			),
		),
		h(
			'div',
			{ class: 'columns' },
			h(
				'section',
				{ class: 'card', 'aria-labelledby': 'comp-h' },
				h('h2', { id: 'comp-h' }, 'Components'),
				h('ul', { class: 'plain' }, sum.components.map((c) => h('li', {}, h('strong', {}, c.name), h('span', { class: 'dim' }, ` ${c.id}${c.required ? ' · required' : ''}${c.targets.length ? ' · ' + c.targets.join(', ') + ' only' : ''}`)))),
			),
			h(
				'section',
				{ class: 'card', 'aria-labelledby': 'theme-h' },
				h('h2', { id: 'theme-h' }, 'Themes'),
				h('ul', { class: 'plain' }, sum.themes.map((t) => h('li', {}, h('strong', {}, t.name), h('span', { class: 'dim mono' }, ` ${t.renderers.join(' ')}`)))),
			),
			h(
				'section',
				{ class: 'card', 'aria-labelledby': 'flows-h' },
				h('h2', { id: 'flows-h' }, 'Flows'),
				h('ul', { class: 'plain' }, sum.flows.map((f) => h('li', {}, h('a', { href: '#', on: { click: (e) => { e.preventDefault(); s.flowId = f.id; s.flow = null; ctx.go('flow'); } } }, f.id), h('span', { class: f.errors ? 'err' : 'dim' }, ` ${f.steps} steps${f.errors ? ` · ${f.errors} error${f.errors === 1 ? '' : 's'}` : ''}`)))),
			),
		),
		h(
			'section',
			{ class: 'card', 'aria-labelledby': 'check-h' },
			h('h2', { id: 'check-h' }, 'Checks'),
			s.problems.length === 0
				? h('p', { class: 'muted' }, 'Nothing to report.')
				: h('ul', { class: 'problems' }, s.problems.map((p) => h('li', { class: p.severity }, h('span', { class: 'mono small' }, p.scope), ' ', p.message))),
		),
		h(
			'section',
			{ class: 'card', 'aria-labelledby': 'agent-h' },
			h('h2', { id: 'agent-h' }, 'Agents'),
			s.mcp
				? h('div', {}, h('p', { class: 'muted' }, 'Agents edit this project over MCP, through the same undo history as you.'), h('p', {}, 'Endpoint ', h('code', {}, s.mcp.url)), h('p', {}, 'Token ', h('code', { class: 'token' }, s.mcp.token)))
				: h('p', { class: 'muted' }, 'The MCP server is not running.'),
		),
	);
}
