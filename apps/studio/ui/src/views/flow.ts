// The Flow screen: the step graph, its editor and the validation errors.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Ctx } from '../app';
import { h, svg } from '../lib/dom';
import {
	NODE_H,
	NODE_W,
	STEP_TYPES,
	addStep,
	connect,
	disconnect,
	layoutGraph,
	removeStep,
	renameStep,
	updateStep,
	validateFlow,
} from '../lib/flowEdit';
import type { Flow, Step } from '../lib/types';

function graph(ctx: Ctx, flow: Flow, errorSteps: Set<string>): SVGElement {
	const s = ctx.state;
	const g = layoutGraph(flow);
	const pos = new Map(g.nodes.map((n) => [n.id, n]));
	const root = svg('svg', { class: 'graph', width: g.width, height: g.height + 20, viewBox: `0 0 ${g.width} ${g.height + 20}`, role: 'group', 'aria-label': `${flow.id} flow graph` });
	root.append(svg('defs', {}, svg('marker', { id: 'arrow', viewBox: '0 0 10 10', refX: 9, refY: 5, markerWidth: 7, markerHeight: 7, orient: 'auto-start-reverse' }, svg('path', { d: 'M0 0L10 5L0 10z', fill: 'currentColor' }))));
	for (const e of g.edges) {
		const a = pos.get(e.from)!;
		const b = pos.get(e.to)!;
		let d: string;
		if (e.back) {
			const x1 = a.x + NODE_W / 2;
			const y1 = a.y + NODE_H;
			const x2 = b.x + NODE_W / 2;
			const y2 = b.y + NODE_H;
			const dip = Math.max(y1, y2) + 24;
			d = `M${x1} ${y1} C${x1} ${dip} ${x2} ${dip} ${x2} ${y2}`;
		} else {
			const x1 = a.x + NODE_W;
			const y1 = a.y + NODE_H / 2;
			const x2 = b.x;
			const y2 = b.y + NODE_H / 2;
			const mx = (x1 + x2) / 2;
			d = `M${x1} ${y1} C${mx} ${y1} ${mx} ${y2} ${x2} ${y2}`;
		}
		root.append(svg('path', { d, class: `edge${e.label ? ' branch' : ''}`, fill: 'none', 'marker-end': 'url(#arrow)' }));
		if (e.label) {
			const lx = e.back ? (a.x + b.x) / 2 + NODE_W / 2 : (a.x + NODE_W + b.x) / 2;
			const ly = e.back ? Math.max(a.y, b.y) + NODE_H + 22 : (a.y + b.y) / 2 + NODE_H / 2 - 6;
			root.append(svg('text', { x: lx, y: ly, class: 'edge-label', 'text-anchor': 'middle' }, e.label.length > 22 ? e.label.slice(0, 21) + '…' : e.label));
		}
	}
	for (const step of flow.steps) {
		const n = pos.get(step.id)!;
		const cls = ['node', `t-${step.type}`, s.selectedStep === step.id ? 'selected' : '', errorSteps.has(step.id) ? 'invalid' : '', s.connectFrom ? 'connectable' : ''].join(' ');
		const el = svg('g', { class: cls, transform: `translate(${n.x} ${n.y})`, tabindex: 0, role: 'button', 'aria-label': `${step.type} step ${step.id}` },
			svg('rect', { width: NODE_W, height: NODE_H, rx: 8 }),
			svg('text', { x: 12, y: 21, class: 'node-id' }, step.id.length > 17 ? step.id.slice(0, 16) + '…' : step.id),
			svg('text', { x: 12, y: 39, class: 'node-type' }, step.type + (step.when ? ' · conditional' : '')),
		);
		const activate = () => {
			if (s.connectFrom && s.connectFrom !== step.id) {
				const from = flow.steps.find((x) => x.id === s.connectFrom);
				const next = connect(flow, s.connectFrom, step.id, from?.type === 'branch' ? -1 : undefined);
				s.connectFrom = null;
				void ctx.run(() => ctx.saveFlow(next, `Connect ${from?.id} to ${step.id}`));
			} else {
				s.selectedStep = step.id;
				s.connectFrom = null;
				ctx.render();
			}
		};
		el.addEventListener('click', activate);
		el.addEventListener('keydown', (e) => {
			if ((e as KeyboardEvent).key === 'Enter' || (e as KeyboardEvent).key === ' ') {
				e.preventDefault();
				activate();
			}
		});
		root.append(el);
	}
	return root;
}

function field(label: string, control: HTMLElement): HTMLElement {
	return h('label', { class: 'field' }, h('span', { class: 'field-label' }, label), control);
}

function stepEditor(ctx: Ctx, flow: Flow, step: Step): HTMLElement {
	const layouts = ctx.state.summary!.layouts;
	const save = (next: Flow, label: string) => void ctx.run(() => ctx.saveFlow(next, label));
	const ids = flow.steps.map((x) => x.id);
	const select = (value: string | undefined, choices: string[], onChange: (v: string) => void, blank = '(none)') =>
		h('select', { on: { change: (e) => onChange((e.target as HTMLSelectElement).value) } }, h('option', { value: '', selected: !value }, blank), choices.map((c) => h('option', { value: c, selected: c === value }, c)));
	const text = (value: string | undefined, onChange: (v: string) => void, mono = true) =>
		h('input', { type: 'text', value: value ?? '', class: mono ? 'mono' : '', on: { change: (e) => onChange((e.target as HTMLInputElement).value.trim()) } });
	return h(
		'div',
		{ class: 'editor' },
		h('h3', {}, 'Step'),
		field('Id', text(step.id, (v) => { ctx.state.selectedStep = v; save(renameStep(flow, step.id, v), `Rename step ${step.id} to ${v}`); })),
		field('Type', select(step.type, [...STEP_TYPES], (v) => save(updateStep(flow, step.id, { type: v as Step['type'] }), `Change ${step.id} to ${v}`))),
		step.type !== 'action' && step.type !== 'branch' ? field('Layout', select(step.layout, layouts, (v) => save(updateStep(flow, step.id, { layout: v }), `Set layout of ${step.id}`))) : null,
		field('Skip unless', text(step.when, (v) => save(updateStep(flow, step.id, { when: v }), `Set condition of ${step.id}`))),
		field('On enter', text(step.on_enter, (v) => save(updateStep(flow, step.id, { on_enter: v }), `Set hook of ${step.id}`))),
		step.type !== 'finish' ? field(step.type === 'branch' ? 'Otherwise go to' : 'Next', select(step.next, ids.filter((i) => i !== step.id), (v) => save(v ? connect(flow, step.id, v) : disconnect(flow, step.id), `Connect ${step.id}`))) : null,
		step.type === 'branch' || (step.branches?.length ?? 0) > 0
			? h(
					'div',
					{},
					h('h3', {}, 'Branches'),
					(step.branches ?? []).map((b, i) =>
						h(
							'div',
							{ class: 'branch-row' },
							text(b.when, (v) => { const f = structuredClone(flow); f.steps.find((x) => x.id === step.id)!.branches![i]!.when = v; save(f, `Edit branch of ${step.id}`); }),
							select(b.next, ids, (v) => save(connect(flow, step.id, v, i), `Connect branch of ${step.id}`), '(choose)'),
							h('button', { class: 'btn ghost small', 'aria-label': 'Remove branch', on: { click: () => save(disconnect(flow, step.id, i), `Remove branch of ${step.id}`) } }, '×'),
						),
					),
					h('button', { class: 'btn ghost', on: { click: () => save(connect(flow, step.id, ids.find((i) => i !== step.id) ?? step.id, -1), `Add branch to ${step.id}`) } }, 'Add branch'),
				)
			: null,
		h(
			'div',
			{ class: 'row-actions' },
			h('button', { class: 'btn', on: { click: () => { ctx.state.connectFrom = step.id; ctx.render(); } } }, 'Connect to…'),
			h('button', { class: 'btn danger', on: { click: () => { ctx.state.selectedStep = null; save(removeStep(flow, step.id), `Remove step ${step.id}`); } } }, 'Remove step'),
		),
	);
}

export function flowView(ctx: Ctx): HTMLElement {
	const s = ctx.state;
	const sum = s.summary!;
	const view = s.flow;
	if (!view) return h('p', { class: 'muted' }, 'Loading…');
	const flow = view.flow;
	// Show the backend's problems, plus any the local rules find that it has not seen yet.
	const problems = view.errors.length ? view.errors : validateFlow(flow);
	const errorSteps = new Set(problems.flatMap((p) => (p.step ? [p.step] : [])));
	const selected = flow.steps.find((x) => x.id === s.selectedStep);
	const add = (type: Step['type']) => {
		const { flow: next, id } = addStep(flow, type, s.selectedStep ?? undefined);
		s.selectedStep = id;
		void ctx.run(() => ctx.saveFlow(next, `Add ${type} step ${id}`));
	};
	return h(
		'div',
		{ class: 'screen' },
		h('div', { class: 'screen-head' }, h('div', {}, h('h1', {}, 'Flow'), h('p', { class: 'muted' }, 'Every path must reach a finish step, and there must be exactly one commit step.'))),
		h('div', { class: 'tabs', role: 'tablist' }, sum.flows.map((f) => h('button', { role: 'tab', 'aria-selected': f.id === s.flowId, class: f.id === s.flowId ? 'tab active' : 'tab', on: { click: () => void ctx.run(async () => { s.selectedStep = null; await ctx.loadFlow(f.id); }) } }, f.id, f.errors ? h('span', { class: 'badge' }, String(f.errors)) : null))),
		h('div', { class: 'toolbar' }, h('span', { class: 'dim small' }, 'Add after selection'), STEP_TYPES.map((t) => h('button', { class: 'btn', on: { click: () => add(t) } }, `+ ${t}`)), s.connectFrom ? h('span', { class: 'hint' }, `Pick the step to connect ${s.connectFrom} to`, h('button', { class: 'btn ghost small', on: { click: () => { s.connectFrom = null; ctx.render(); } } }, 'Cancel')) : null),
		h(
			'div',
			{ class: 'flow-layout' },
			h('div', { class: 'graph-wrap card' }, graph(ctx, flow, errorSteps)),
			h(
				'aside',
				{ class: 'card side' },
				selected ? stepEditor(ctx, flow, selected) : h('p', { class: 'muted' }, 'Select a step to edit it.'),
				h('h3', {}, problems.length === 0 ? 'Valid' : `Problems (${problems.length})`),
				problems.length === 0
					? h('p', { class: 'ok' }, 'The flow passes validation.')
					: h('ul', { class: 'problems' }, problems.map((p) => h('li', { class: p.severity }, p.step ? h('a', { href: '#', on: { click: (e) => { e.preventDefault(); s.selectedStep = p.step!; ctx.render(); } } }, p.step) : null, p.step ? ' ' : '', p.message))),
			),
		),
	);
}
