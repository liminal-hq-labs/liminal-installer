// A small Lua scanner: the words and brackets of each line, with strings and comments left out.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export interface LexToken {
	text: string;
	kind: 'word' | 'punct';
	/** Offsets within the line. */
	from: number;
	to: number;
}

export interface LexLine {
	text: string;
	/** True when the line begins inside a long string or long comment, so its text is not code. */
	startsInside: boolean;
	tokens: LexToken[];
}

const closer = (level: number) => `]${'='.repeat(level)}]`;

/** Reads `source` line by line. Words and single punctuation marks come back; strings, comments and numbers do not. */
export function lex(source: string): LexLine[] {
	const lines = source.split('\n');
	const out: LexLine[] = [];
	let inside: string | null = null;
	for (const raw of lines) {
		const text = raw.endsWith('\r') ? raw.slice(0, -1) : raw;
		const line: LexLine = { text, startsInside: inside !== null, tokens: [] };
		let i = 0;
		while (i < text.length) {
			if (inside !== null) {
				const end = text.indexOf(inside, i);
				if (end < 0) {
					i = text.length;
				} else {
					i = end + inside.length;
					inside = null;
				}
				continue;
			}
			const c = text[i]!;
			const long = /^\[(=*)\[/.exec(text.slice(i));
			if (c === '-' && text[i + 1] === '-') {
				const open = /^\[(=*)\[/.exec(text.slice(i + 2));
				if (open) {
					inside = closer(open[1]!.length);
					i += 2 + open[0].length;
				} else {
					i = text.length;
				}
			} else if (c === '"' || c === "'") {
				i++;
				while (i < text.length && text[i] !== c) i += text[i] === '\\' ? 2 : 1;
				i++;
			} else if (c === '[' && long) {
				inside = closer(long[1]!.length);
				i += long[0].length;
			} else if (/[A-Za-z_]/.test(c)) {
				const from = i;
				while (i < text.length && /[A-Za-z0-9_]/.test(text[i]!)) i++;
				line.tokens.push({ text: text.slice(from, i), kind: 'word', from, to: i });
			} else if (/[0-9]/.test(c)) {
				while (i < text.length && /[A-Za-z0-9_.]/.test(text[i]!)) i++;
			} else if (/\s/.test(c)) {
				i++;
			} else {
				line.tokens.push({ text: c, kind: 'punct', from: i, to: i + 1 });
				i++;
			}
		}
		out.push(line);
	}
	return out;
}

/** What a token does to the block depth: +1 opens, -1 closes, 0 neither. */
export function depthChange(t: LexToken): number {
	if (t.kind === 'punct') return '([{'.includes(t.text) ? 1 : ')]}'.includes(t.text) ? -1 : 0;
	switch (t.text) {
		case 'function':
		case 'if':
		case 'do':
		case 'repeat':
			return 1;
		case 'end':
		case 'until':
			return -1;
		default:
			return 0;
	}
}

/** A token that closes a block at the start of a line, so the line sits one level out. `else` and `elseif` close and reopen. */
export const dedentsLine = (t: LexToken): boolean =>
	t.kind === 'punct' ? ')]}'.includes(t.text) : ['end', 'until', 'else', 'elseif'].includes(t.text);
