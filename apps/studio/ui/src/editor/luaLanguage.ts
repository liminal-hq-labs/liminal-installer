// Lua 5.4 in the editor: colouring, folding by block, and the keys a Lua line closes on.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	foldService,
	HighlightStyle,
	LanguageSupport,
	StreamLanguage,
	syntaxHighlighting,
} from '@codemirror/language';
import type { Text } from '@codemirror/state';
import { lua } from '@codemirror/legacy-modes/mode/lua';
import { tags } from '@lezer/highlight';
import { depthChange, lex } from './luaLex';

const language = StreamLanguage.define(lua);

/** The colours are tokens (`--ed-*`), set for each theme in `app.css`. */
export const luaHighlight = HighlightStyle.define([
	{
		tag: [tags.keyword, tags.controlKeyword, tags.definitionKeyword, tags.operatorKeyword],
		color: 'var(--ed-keyword)',
	},
	{ tag: [tags.atom, tags.bool, tags.null], color: 'var(--ed-atom)' },
	{ tag: tags.number, color: 'var(--ed-number)' },
	{ tag: [tags.string, tags.special(tags.string)], color: 'var(--ed-string)' },
	{
		tag: [tags.comment, tags.lineComment, tags.blockComment],
		color: 'var(--ed-comment)',
		fontStyle: 'italic',
	},
	{
		tag: [
			tags.standard(tags.variableName),
			tags.function(tags.variableName),
			tags.function(tags.propertyName),
		],
		color: 'var(--ed-function)',
	},
	{
		tag: [tags.definition(tags.variableName), tags.definition(tags.propertyName)],
		color: 'var(--ed-definition)',
	},
	{ tag: tags.propertyName, color: 'var(--ed-property)' },
	{ tag: [tags.operator, tags.punctuation, tags.bracket], color: 'var(--ed-punct)' },
	{ tag: tags.invalid, color: 'var(--ed-error)' },
]);

/** The ranges that fold: from the end of the line that opens a block to the end of the line before its closer. */
const foldCache = new WeakMap<Text, Map<number, { from: number; to: number }>>();

function folds(doc: Text): Map<number, { from: number; to: number }> {
	const cached = foldCache.get(doc);
	if (cached) return cached;
	const map = new Map<number, { from: number; to: number }>();
	const stack: number[] = [];
	lex(doc.toString()).forEach((line, index) => {
		if (line.startsInside) return;
		for (const t of line.tokens) {
			const d = depthChange(t);
			if (d > 0) stack.push(index + 1);
			else if (d < 0) {
				const open = stack.pop();
				// `else` branches fold with their block; a block that ends on its own line folds the lines between.
				if (open !== undefined && open < index + 1 && !map.has(open)) {
					const from = doc.line(open).to;
					const to = doc.line(index).to;
					if (to > from) map.set(open, { from, to });
				}
			}
		}
	});
	foldCache.set(doc, map);
	return map;
}

const luaFolding = foldService.of((state, lineStart) => {
	const line = state.doc.lineAt(lineStart);
	return folds(state.doc).get(line.number) ?? null;
});

/** The Lua language with its colouring and folding. */
export function luaSupport(): LanguageSupport {
	return new LanguageSupport(language, [syntaxHighlighting(luaHighlight), luaFolding]);
}
