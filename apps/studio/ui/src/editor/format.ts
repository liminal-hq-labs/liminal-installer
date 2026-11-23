// The Lua formatter: re-indents by block depth, trims trailing space and settles line endings.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// It changes whitespace only: never the order or spelling of code, and never the inside of a long string or comment.

import { dedentsLine, depthChange, lex } from './luaLex';

/**
 * The script with every line indented one unit per open block or bracket, no trailing spaces, line feeds only and
 * one newline at the end. A line that continues inside an open bracket is indented one level. The text inside a
 * long string or long comment is left as it is.
 */
export function formatLua(source: string, unit = '\t'): string {
	const lines = lex(source.replace(/\r\n?/g, '\n'));
	let depth = 0;
	const out = lines.map((line) => {
		if (line.startsInside) return line.text;
		const first = line.tokens[0];
		const body = line.text.trim();
		if (body === '') return '';
		const level = Math.max(0, depth - (first && dedentsLine(first) ? 1 : 0));
		for (const t of line.tokens) {
			// `else` and `elseif` close one block and open the next.
			if (t.kind === 'word' && (t.text === 'else' || t.text === 'elseif')) continue;
			depth = Math.max(0, depth + depthChange(t));
		}
		return `${unit.repeat(level)}${body}`;
	});
	while (out.length > 0 && out[out.length - 1]!.trim() === '' && !lines[out.length - 1]!.startsInside) out.pop();
	return out.length === 0 ? '' : `${out.join('\n')}\n`;
}
