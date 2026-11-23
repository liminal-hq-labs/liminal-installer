// A theme's stage and a page's layout composed into one screen, with each node tied to its source.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Layout, LayoutNode } from './types';

/** Which layout a node belongs to: the theme's stage (every page of the theme) or the page. */
export type Scope = 'stage' | 'page';

/** A node's place in its own layout: the scope and the path of child indexes from that layout's root. */
export interface Ref {
	scope: Scope;
	path: number[];
}

/** The step a screen is composed for: stage nodes may be limited by `show_on` and `hide_on`. */
export interface StepInfo {
	id: string;
	kind: string;
}

export const STAGE_NAME = 'stage';

/** The id of the stage layout of the theme a layout id (`theme/name`) is in, or null for a shared layout. */
export function stageIdFor(layoutId: string): string | null {
	const i = layoutId.indexOf('/');
	return i > 0 ? `${layoutId.slice(0, i)}/${STAGE_NAME}` : null;
}

export const isStageId = (layoutId: string): boolean => layoutId.endsWith(`/${STAGE_NAME}`);

/** `a.b.c` addresses, as used in `data-node-path`, and back. */
export const refKey = (r: Ref): string => `${r.scope === 'stage' ? 's' : 'p'}:${r.path.join('.')}`;
export function parseRef(key: string | null | undefined): Ref | null {
	const m = /^([sp]):(\d+(?:\.\d+)*)?$/.exec(key ?? '');
	if (!m) return null;
	return {
		scope: m[1] === 's' ? 'stage' : 'page',
		path: m[2] ? m[2].split('.').map(Number) : [],
	};
}

function listed(node: LayoutNode, key: string, step: StepInfo): boolean | null {
	const v = node.props?.[key];
	if (!Array.isArray(v)) return null;
	return v.some((s) => s === step.id || s === step.kind);
}

/** Whether a stage node is drawn on a step, by its `show_on` and `hide_on` lists. */
export function shownOn(node: LayoutNode, step: StepInfo): boolean {
	return (listed(node, 'show_on', step) ?? true) && !(listed(node, 'hide_on', step) ?? false);
}

export interface Screen {
	root: LayoutNode;
	/** The source of a node in `root`. The composed nodes are copies, so this is keyed by them. */
	refs: Map<LayoutNode, Ref>;
}

const placeholder = (): LayoutNode => ({
	kind: 'dialog',
	props: { title: 'Page', label: 'Page' },
	children: [{ kind: 'text', props: { role: 'body', text: '…' } }],
});

/**
 * The whole screen: the stage with `page` in its `slot` (or last, when it has none), and a map from
 * each node back to its place in the stage or the page. A page whose root is a `backdrop`, and a
 * project with no stage, is the page alone. With no page, a placeholder dialog stands in the slot.
 */
export function composeScreen(
	stage: LayoutNode | null,
	page: LayoutNode | null,
	step: StepInfo,
): Screen {
	const refs = new Map<LayoutNode, Ref>();
	const build = (node: LayoutNode, scope: Scope, path: number[]): LayoutNode => {
		const out: LayoutNode = { ...node };
		refs.set(out, { scope, path });
		if (node.children) {
			const kids: LayoutNode[] = [];
			node.children.forEach((c, i) => {
				if (scope === 'stage' && !shownOn(c, step)) return;
				const at = [...path, i];
				if (scope === 'stage' && c.kind === 'slot') {
					kids.push(page ? build(page, 'page', []) : tag(placeholder(), 'stage', at));
					slotted = true;
				} else kids.push(build(c, scope, at));
			});
			out.children = kids;
		}
		return out;
	};
	const tag = (node: LayoutNode, scope: Scope, path: number[]): LayoutNode => {
		refs.set(node, { scope, path });
		return node;
	};
	let slotted = false;
	if (!stage || (page && page.kind === 'backdrop')) {
		const root = build(page ?? placeholder(), page ? 'page' : 'stage', []);
		return { root, refs };
	}
	const root = build(stage, 'stage', []);
	if (!slotted) {
		const last = page ? build(page, 'page', []) : tag(placeholder(), 'stage', []);
		root.children = [...(root.children ?? []), last];
	}
	return { root, refs };
}

export interface Row {
	ref: Ref;
	depth: number;
	node: LayoutNode;
}

/** The composed screen's nodes in document order, for the tree: each with its own layout's node and path. */
export function screenRows(
	stage: LayoutNode | null,
	page: LayoutNode | null,
	step: StepInfo,
): Row[] {
	const { root, refs } = composeScreen(stage, page, step);
	const source = (r: Ref): LayoutNode | undefined => {
		let n: LayoutNode | undefined =
			r.scope === 'stage' ? (stage ?? undefined) : (page ?? undefined);
		for (const i of r.path) n = n?.children?.[i];
		return n;
	};
	const rows: Row[] = [];
	const walk = (n: LayoutNode, depth: number) => {
		const ref = refs.get(n);
		if (ref) rows.push({ ref, depth, node: source(ref) ?? n });
		n.children?.forEach((c) => walk(c, depth + 1));
	};
	walk(root, 0);
	return rows;
}

/** The page layout with the stage's nodes copied in around it, so the page draws its own backdrop. */
export function detachStage(stage: Layout, page: Layout, step: StepInfo): Layout {
	const { root } = composeScreen(structuredClone(stage.root), structuredClone(page.root), step);
	return { ...page, root };
}

/** The page layout without its own backdrop: its dialog or window becomes the root, and the theme's stage draws the rest. */
export function attachStage(page: Layout): Layout {
	if (page.root.kind !== 'backdrop') return page;
	const frame = page.root.children?.find((c) => c.kind === 'dialog' || c.kind === 'window');
	return frame ? { ...page, root: structuredClone(frame) } : page;
}
