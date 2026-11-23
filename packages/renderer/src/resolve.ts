// Applies a node's `@renderer` overrides, so a tree is what one renderer should draw.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { LayoutNode } from './types';

/** The node as `renderer` draws it: its override if one exists, otherwise the base node, all the way down. */
export function resolveForRenderer(node: LayoutNode, renderer: string): LayoutNode {
	const base = node.overrides?.[renderer] ?? node;
	const out: LayoutNode = { kind: base.kind };
	if (base.bind) out.bind = base.bind;
	if (base.props) out.props = base.props;
	if (base.children) out.children = base.children.map((c) => resolveForRenderer(c, renderer));
	return out;
}
