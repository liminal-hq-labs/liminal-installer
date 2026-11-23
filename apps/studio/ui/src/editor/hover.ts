// Documentation for the name under the pointer, as a popup.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { EditorState, Extension } from '@codemirror/state';
import { hoverTooltip, type Tooltip } from '@codemirror/view';
import { docElement } from './docElement';
import { HEADER_DOC, capabilityDoc, docFor } from './luaDocs';
import { t } from './phrases';

const WORD = /[A-Za-z0-9_]/;

/** The qualified name at `pos`, such as `li.env.get` when the pointer is on `get`, and the span of the word. */
export function nameAt(state: EditorState, pos: number): { name: string; from: number; to: number } | null {
	const line = state.doc.lineAt(pos);
	const text = line.text;
	let start = pos - line.from;
	let end = start;
	while (start > 0 && WORD.test(text[start - 1]!)) start--;
	while (end < text.length && WORD.test(text[end]!)) end++;
	if (start === end || /^\d/.test(text.slice(start, end))) return null;
	let begin = start;
	// Walk back over `ident.` and `ident:` segments.
	while (begin > 1 && (text[begin - 1] === '.' || text[begin - 1] === ':')) {
		let k = begin - 1;
		while (k > 0 && WORD.test(text[k - 1]!)) k--;
		if (k === begin - 1) break;
		begin = k;
	}
	return { name: text.slice(begin, end), from: line.from + start, to: line.from + end };
}

/** The popup for `pos`, or null when nothing there is documented. */
export function docTooltip(state: EditorState, pos: number): Tooltip | null {
	const line = state.doc.lineAt(pos);
	const header = /^\s*--+\s*capabilities:/.exec(line.text);
	if (header) {
		const col = pos - line.from;
		const word = /[\w.]+/g;
		for (let m = word.exec(line.text); m; m = word.exec(line.text)) {
			if (col < m.index || col > m.index + m[0].length) continue;
			const doc = m[0] === 'capabilities' ? HEADER_DOC : capabilityDoc(m[0]);
			if (!doc) return null;
			return tooltip(line.from + m.index, line.from + m.index + m[0].length, doc);
		}
		return null;
	}
	if (/^\s*--/.test(line.text) && !/\S\s*--/.test(line.text.slice(0, pos - line.from))) return null;
	const found = nameAt(state, pos);
	if (!found) return null;
	// `li.env.get` first, then what the hovered word is on its own (`page`, `detect`).
	const doc = docFor(found.name) ?? docFor(found.name.split(/[.:]/).pop() ?? '');
	if (!doc) return null;
	return tooltip(found.from, found.to, doc);
}

function tooltip(pos: number, end: number, doc: Parameters<typeof docElement>[0]): Tooltip {
	return {
		pos,
		end,
		above: true,
		create() {
			const dom = docElement(doc);
			dom.setAttribute('role', 'tooltip');
			dom.setAttribute('aria-label', t('doc.popup'));
			return { dom };
		},
	};
}

export const hoverDocs = (): Extension => hoverTooltip((view, pos) => docTooltip(view.state, pos), { hoverTime: 350 });
