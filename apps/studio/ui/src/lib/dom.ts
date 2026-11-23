// A tiny element builder so views stay plain functions that return DOM nodes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

type Child = Node | string | number | null | undefined | false | Child[];
type Attrs = Record<string, unknown> & { on?: Record<string, (e: Event) => void> };

export function h<K extends keyof HTMLElementTagNameMap>(
	tag: K,
	attrs: Attrs = {},
	...children: Child[]
): HTMLElementTagNameMap[K] {
	const el = document.createElement(tag);
	for (const [key, value] of Object.entries(attrs)) {
		if (key === 'on') {
			for (const [type, fn] of Object.entries(value as Record<string, (e: Event) => void>))
				el.addEventListener(type, fn);
		} else if (value === false || value === undefined || value === null) {
			continue;
		} else if (key === 'class') {
			el.className = String(value);
		} else if (key in el && key !== 'list' && key !== 'style') {
			(el as unknown as Record<string, unknown>)[key] = value;
		} else {
			el.setAttribute(key, value === true ? '' : String(value));
		}
	}
	append(el, children);
	return el;
}

function append(el: Element, children: Child[]): void {
	for (const c of children) {
		if (Array.isArray(c)) append(el, c);
		else if (c === null || c === undefined || c === false) continue;
		else el.append(typeof c === 'object' ? c : String(c));
	}
}

export const svg = (
	tag: string,
	attrs: Record<string, string | number> = {},
	...children: (Node | string)[]
): SVGElement => {
	const el = document.createElementNS('http://www.w3.org/2000/svg', tag);
	for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
	el.append(...children);
	return el;
};

/** Replaces everything inside `el` with `children`. */
export function mount(el: Element, ...children: Child[]): void {
	el.replaceChildren();
	append(el, children);
}
