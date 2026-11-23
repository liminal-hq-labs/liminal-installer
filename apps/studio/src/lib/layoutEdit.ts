// Pure edits for layout node trees, addressed by paths of child indexes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Layout, LayoutNode } from './types';

export type NodePath = number[];

export const NODE_KINDS = [
	'window',
	'backdrop',
	'dialog',
	'stack',
	'grid',
	'text',
	'image',
	'billboard',
	'meter',
	'button',
	'checklist',
	'radio',
	'field',
	'path',
	'progress',
	'custom',
] as const;

export const RENDERERS = ['tui', 'tui-retro', 'win16', 'win32-classic', 'canvas', 'web'] as const;

const clone = <T>(v: T): T => structuredClone(v);

export function getNode(root: LayoutNode, path: NodePath): LayoutNode | undefined {
	let node: LayoutNode | undefined = root;
	for (const i of path) node = node?.children?.[i];
	return node;
}

/** Applies `edit` to a copy of the layout's node at `path`. Returns the layout unchanged if absent. */
function editNode(layout: Layout, path: NodePath, edit: (node: LayoutNode) => void): Layout {
	const out = clone(layout);
	const node = getNode(out.root, path);
	if (node) edit(node);
	return out;
}

/** The node as a renderer sees it: its override if one exists, otherwise the base node. */
export function effectiveNode(node: LayoutNode, renderer: string | null): LayoutNode {
	return (renderer && node.overrides?.[renderer]) || node;
}

/** The whole tree as one renderer draws it: overrides applied, override tables removed. */
export function resolveForRenderer(node: LayoutNode, renderer: string): LayoutNode {
	const base = node.overrides?.[renderer] ?? node;
	const out: LayoutNode = { kind: base.kind };
	if (base.bind) out.bind = base.bind;
	if (base.props) out.props = base.props;
	if (base.children) out.children = base.children.map((c) => resolveForRenderer(c, renderer));
	return out;
}

export function setProp(
	layout: Layout,
	path: NodePath,
	key: string,
	value: unknown,
	renderer: string | null = null,
): Layout {
	return editNode(layout, path, (node) => {
		const target = effectiveNode(node, renderer);
		const props = (target.props ??= {});
		if (value === undefined || value === '') delete props[key];
		else props[key] = value;
		if (Object.keys(props).length === 0) delete target.props;
	});
}

export function setKind(
	layout: Layout,
	path: NodePath,
	kind: string,
	renderer: string | null = null,
): Layout {
	return editNode(layout, path, (node) => {
		effectiveNode(node, renderer).kind = kind;
	});
}

export function setBind(
	layout: Layout,
	path: NodePath,
	bind: string,
	renderer: string | null = null,
): Layout {
	return editNode(layout, path, (node) => {
		const target = effectiveNode(node, renderer);
		if (bind) target.bind = bind;
		else delete target.bind;
	});
}

export function newNode(kind: string): LayoutNode {
	const node: LayoutNode = { kind };
	const named = ['button', 'checklist', 'radio', 'field', 'path', 'progress', 'meter'];
	if (named.includes(kind)) node.props = { label: 'Label' };
	if (kind === 'text') node.props = { text: 'Text', role: 'body' };
	if (kind === 'button') node.props = { label: 'Button', action: 'next' };
	if (kind === 'meter') node.props = { label: 'Meter', source: 'total' };
	return node;
}

export function addChild(
	layout: Layout,
	path: NodePath,
	kind: string,
): { layout: Layout; path: NodePath } {
	let added: NodePath = path;
	const out = editNode(layout, path, (node) => {
		(node.children ??= []).push(newNode(kind));
		added = [...path, node.children.length - 1];
	});
	return { layout: out, path: added };
}

/** Removes the node at `path`. The root cannot be removed. */
export function removeNode(layout: Layout, path: NodePath): Layout {
	if (path.length === 0) return layout;
	const parent = path.slice(0, -1);
	const index = path[path.length - 1]!;
	return editNode(layout, parent, (node) => {
		node.children?.splice(index, 1);
		if (node.children?.length === 0) delete node.children;
	});
}

/** Moves a node among its siblings; returns the layout and the node's new path. */
export function moveNode(
	layout: Layout,
	path: NodePath,
	delta: -1 | 1,
): { layout: Layout; path: NodePath } {
	if (path.length === 0) return { layout, path };
	const parent = path.slice(0, -1);
	const from = path[path.length - 1]!;
	const to = from + delta;
	let moved = path;
	const out = editNode(layout, parent, (node) => {
		const kids = node.children;
		if (!kids || to < 0 || to >= kids.length) return;
		const [item] = kids.splice(from, 1);
		kids.splice(to, 0, item!);
		moved = [...parent, to];
	});
	return { layout: out, path: moved };
}

export function hasOverride(node: LayoutNode, renderer: string): boolean {
	return Boolean(node.overrides?.[renderer]);
}

/** Turns an override for `renderer` on (a copy of the node's current look) or off. */
export function toggleOverride(
	layout: Layout,
	path: NodePath,
	renderer: string,
	on: boolean,
): Layout {
	return editNode(layout, path, (node) => {
		if (on && !hasOverride(node, renderer)) {
			const copy: LayoutNode = { kind: node.kind };
			if (node.bind) copy.bind = node.bind;
			if (node.props) copy.props = clone(node.props);
			if (node.children) copy.children = clone(node.children);
			(node.overrides ??= {})[renderer] = copy;
		} else if (!on && node.overrides) {
			delete node.overrides[renderer];
			if (Object.keys(node.overrides).length === 0) delete node.overrides;
		}
	});
}

/** A short label for a node in the tree view. */
export function describeNode(node: LayoutNode): string {
	const p = node.props ?? {};
	const text = [p.title, p.label, p.text, p.source].find(
		(v) => typeof v === 'string' && v.length > 0,
	);
	return typeof text === 'string'
		? `${node.kind} · ${text.length > 36 ? text.slice(0, 35) + '…' : text}`
		: node.kind;
}

/** Every node with its path and depth, in document order, for rendering a tree. */
export function flattenTree(
	root: LayoutNode,
): { path: NodePath; depth: number; node: LayoutNode }[] {
	const out: { path: NodePath; depth: number; node: LayoutNode }[] = [];
	const walk = (node: LayoutNode, path: NodePath) => {
		out.push({ path, depth: path.length, node });
		node.children?.forEach((c, i) => walk(c, [...path, i]));
	};
	walk(root, []);
	return out;
}

/** The text shown for a prop value in an input: strings as they are, anything else as JSON. */
export function propToText(value: unknown): string {
	return typeof value === 'string' ? value : JSON.stringify(value);
}

/** Reads an edited prop value back. A string prop stays a string; others are parsed as JSON. */
export function parsePropValue(text: string, previous: unknown): unknown {
	if (typeof previous === 'string') return text;
	const t = text.trim();
	if (previous === undefined && !/^(true|false|null|-?\d+(\.\d+)?|\[.*\]|\{.*\})$/s.test(t))
		return text;
	try {
		return JSON.parse(t);
	} catch {
		return text;
	}
}
