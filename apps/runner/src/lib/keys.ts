// Keyboard navigation: which button Enter, Esc and Alt+Left press on a page.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { LayoutNode } from '@liminal-hq/installer-renderer';

/** Actions that move the flow on. */
export const FORWARD = ['next', 'commit', 'finish', 'skip'] as const;

const FORWARD_SET: ReadonlySet<string> = new Set(FORWARD);
export const isForward = (action: string): boolean => FORWARD_SET.has(action);

/** Every node in the tree with its path, as the shell numbers them: child indexes joined by `.`. */
export function pathsOf(root: LayoutNode): Map<LayoutNode, string> {
	const out = new Map<LayoutNode, string>();
	const walk = (node: LayoutNode, path: string) => {
		out.set(node, path);
		(node.children ?? []).forEach((c, i) => walk(c, path === '' ? String(i) : `${path}.${i}`));
	};
	walk(root, '');
	return out;
}

interface Button {
	action: string;
	isDefault: boolean;
}

/** The buttons a page has that are switched on, in reading order. */
export function buttons(root: LayoutNode, disabled: ReadonlySet<string>): Button[] {
	const paths = pathsOf(root);
	const out: Button[] = [];
	for (const [node, path] of paths) {
		if (node.kind !== 'button' || disabled.has(path)) continue;
		const action = node.props?.action;
		out.push({
			action: typeof action === 'string' && action ? action : 'next',
			isDefault: node.props?.default === true,
		});
	}
	return out;
}

/** What Enter does when focus is not on a control: the default button, else the first forward one. */
export function enterAction(root: LayoutNode, disabled: ReadonlySet<string>): string | null {
	const all = buttons(root, disabled);
	return (
		all.find((b) => b.isDefault)?.action ?? all.find((b) => isForward(b.action))?.action ?? null
	);
}

/** What Esc does: the page's Cancel button, if it has one that is on. */
export function escapeAction(root: LayoutNode, disabled: ReadonlySet<string>): string | null {
	return buttons(root, disabled).find((b) => b.action === 'cancel')?.action ?? null;
}

/** What Alt+Left does: the page's Back button, if it has one that is on. */
export function backAction(root: LayoutNode, disabled: ReadonlySet<string>): string | null {
	return buttons(root, disabled).find((b) => b.action === 'back')?.action ?? null;
}

/** The control to focus when a page appears: its first input or choice, else its default button. */
export const FIRST_FOCUS =
	'input:not([type=hidden]), select, textarea, [role=radio], [role=checkbox], button[data-kind=button]';
