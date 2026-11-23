// Pure edits and layout for flow graphs, with a mirror of the backend's validation rules.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Flow, Problem, Step, StepType } from './types';

export const STEP_TYPES: StepType[] = ['page', 'action', 'branch', 'commit', 'finish'];

const clone = <T>(v: T): T => structuredClone(v);

/** Every step id a step points at: `next` first, then each branch. */
export function outgoing(step: Step): string[] {
	return [...(step.next ? [step.next] : []), ...(step.branches ?? []).map((b) => b.next)];
}

/** An id like `page-2` that no step uses. */
export function freshId(flow: Flow, base: string): string {
	const used = new Set(flow.steps.map((s) => s.id));
	if (!used.has(base)) return base;
	for (let n = 2; ; n++) if (!used.has(`${base}-${n}`)) return `${base}-${n}`;
}

/**
 * Adds a step. With `after`, the new step is spliced into that step's `next` edge so the graph
 * stays connected; otherwise it is appended unconnected.
 */
export function addStep(flow: Flow, type: StepType, after?: string): { flow: Flow; id: string } {
	const out = clone(flow);
	const id = freshId(out, type);
	const step: Step = { id, type };
	if (type === 'page') step.layout = '';
	const prev = after ? out.steps.find((s) => s.id === after) : undefined;
	if (prev && prev.type !== 'finish') {
		if (type !== 'finish') step.next = prev.next;
		prev.next = id;
		out.steps.splice(out.steps.indexOf(prev) + 1, 0, step);
	} else {
		out.steps.push(step);
	}
	return { flow: out, id };
}

/** Removes a step; whatever pointed at it now points at its `next`, or nothing. */
export function removeStep(flow: Flow, id: string): Flow {
	const out = clone(flow);
	const gone = out.steps.find((s) => s.id === id);
	if (!gone) return out;
	const bypass = gone.next;
	out.steps = out.steps.filter((s) => s.id !== id);
	for (const s of out.steps) {
		if (s.next === id) {
			if (bypass) s.next = bypass;
			else delete s.next;
		}
		if (s.branches) {
			s.branches = s.branches.flatMap((b) =>
				b.next !== id ? [b] : bypass ? [{ ...b, next: bypass }] : [],
			);
			if (s.branches.length === 0) delete s.branches;
		}
	}
	if (out.start === id) {
		if (bypass) out.start = bypass;
		else delete out.start;
	}
	return out;
}

/** Points `from` at `to`. For a branch step, `branch` picks the branch (-1 adds one). */
export function connect(flow: Flow, from: string, to: string, branch?: number): Flow {
	const out = clone(flow);
	const step = out.steps.find((s) => s.id === from);
	if (!step) return out;
	if (branch === undefined) {
		step.next = to;
	} else if (branch < 0) {
		(step.branches ??= []).push({ when: 'true', next: to });
	} else if (step.branches?.[branch]) {
		step.branches[branch]!.next = to;
	}
	return out;
}

/** Removes the `next` edge, or the branch at `branch`. */
export function disconnect(flow: Flow, from: string, branch?: number): Flow {
	const out = clone(flow);
	const step = out.steps.find((s) => s.id === from);
	if (!step) return out;
	if (branch === undefined) delete step.next;
	else {
		step.branches?.splice(branch, 1);
		if (step.branches?.length === 0) delete step.branches;
	}
	return out;
}

/** Renames a step and every reference to it. */
export function renameStep(flow: Flow, id: string, to: string): Flow {
	if (!to || to === id || flow.steps.some((s) => s.id === to)) return flow;
	const out = clone(flow);
	for (const s of out.steps) {
		if (s.id === id) s.id = to;
		if (s.next === id) s.next = to;
		for (const b of s.branches ?? []) if (b.next === id) b.next = to;
	}
	if (out.start === id) out.start = to;
	return out;
}

export function updateStep(flow: Flow, id: string, patch: Partial<Step>): Flow {
	const out = clone(flow);
	const step = out.steps.find((s) => s.id === id);
	if (!step) return out;
	for (const [k, v] of Object.entries(patch)) {
		if (v === undefined || v === '') delete (step as unknown as Record<string, unknown>)[k];
		else (step as unknown as Record<string, unknown>)[k] = v;
	}
	return out;
}

/** The same rules as `Flow::validate`, for instant feedback before the backend replies. */
export function validateFlow(flow: Flow): Problem[] {
	const scope = `flow:${flow.id}`;
	const err = (message: string, step?: string): Problem => ({
		severity: 'error',
		scope,
		message,
		...(step ? { step } : {}),
	});
	if (flow.steps.length === 0) return [err('flow has no steps')];
	const out: Problem[] = [];
	const ids = new Set<string>();
	for (const s of flow.steps) {
		if (ids.has(s.id)) out.push(err(`duplicate step id \`${s.id}\``, s.id));
		ids.add(s.id);
	}
	const commits = flow.steps.filter((s) => s.type === 'commit').length;
	if (commits !== 1) out.push(err(`a flow needs exactly one commit step, found ${commits}`));
	for (const s of flow.steps) {
		const to = outgoing(s);
		if (to.length === 0 && s.type !== 'finish')
			out.push(err(`step \`${s.id}\` has no next step and is not a finish step`, s.id));
		for (const t of to)
			if (!ids.has(t)) out.push(err(`step \`${s.id}\` points at unknown step \`${t}\``, s.id));
	}
	const reaches = new Set(flow.steps.filter((s) => s.type === 'finish').map((s) => s.id));
	for (let changed = true; changed; ) {
		changed = false;
		for (const s of flow.steps)
			if (!reaches.has(s.id) && outgoing(s).some((t) => reaches.has(t))) {
				reaches.add(s.id);
				changed = true;
			}
	}
	for (const s of flow.steps)
		if (!reaches.has(s.id)) out.push(err(`step \`${s.id}\` cannot reach a finish step`, s.id));
	return out;
}

export interface GraphNode {
	id: string;
	x: number;
	y: number;
	column: number;
	row: number;
}

export interface GraphEdge {
	from: string;
	to: string;
	label?: string;
	/** True when the edge goes back to an earlier column. */
	back: boolean;
}

export const NODE_W = 150;
export const NODE_H = 52;
const GAP_X = 70;
const GAP_Y = 28;

/** Layers steps by their distance from the start step; unreachable steps go in a final column. */
export function layoutGraph(flow: Flow): { nodes: GraphNode[]; edges: GraphEdge[]; width: number; height: number } {
	const byId = new Map(flow.steps.map((s) => [s.id, s]));
	const start = flow.start ?? flow.steps[0]?.id;
	const column = new Map<string, number>();
	if (start && byId.has(start)) {
		column.set(start, 0);
		const queue = [start];
		while (queue.length) {
			const id = queue.shift()!;
			for (const t of outgoing(byId.get(id)!)) {
				if (byId.has(t) && !column.has(t)) {
					column.set(t, column.get(id)! + 1);
					queue.push(t);
				}
			}
		}
	}
	const loose = Math.max(-1, ...column.values()) + 1;
	for (const s of flow.steps) if (!column.has(s.id)) column.set(s.id, loose);
	const rows = new Map<number, number>();
	const nodes: GraphNode[] = flow.steps.map((s) => {
		const c = column.get(s.id)!;
		const r = rows.get(c) ?? 0;
		rows.set(c, r + 1);
		return { id: s.id, column: c, row: r, x: 16 + c * (NODE_W + GAP_X), y: 16 + r * (NODE_H + GAP_Y) };
	});
	const edges: GraphEdge[] = [];
	for (const s of flow.steps) {
		if (s.next && byId.has(s.next))
			edges.push({ from: s.id, to: s.next, back: column.get(s.next)! <= column.get(s.id)! });
		for (const b of s.branches ?? [])
			if (byId.has(b.next))
				edges.push({ from: s.id, to: b.next, label: b.when, back: column.get(b.next)! <= column.get(s.id)! });
	}
	const maxCol = Math.max(0, ...column.values());
	const maxRow = Math.max(1, ...rows.values());
	return {
		nodes,
		edges,
		width: 32 + (maxCol + 1) * (NODE_W + GAP_X) - GAP_X,
		height: 32 + maxRow * (NODE_H + GAP_Y) - GAP_Y,
	};
}
