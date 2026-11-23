// Pseudo-locale transforms: accented, lengthened text, and a right-to-left wrapper, both made from English.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Catalogue } from './active';

/** `{name}` interpolation tokens, which a transform must leave exactly as they are. */
const TOKEN = /(\{\w+\})/;

const PLAIN = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ';
const ACCENTED = 'àƀçďéƒĝĥîĵķĺḿñöþǫŕšţûṽŵẋýžÀƁÇĎÉƑĜĤÎĴĶĹḾÑÖÞǪŔŠŢÛṼŴẊÝŽ';
const ACCENTS = new Map([...PLAIN].map((letter, index) => [letter, [...ACCENTED][index]!]));

/** How much longer than English the accented pseudo-locale runs; German and Finnish do this. */
export const EXPANSION = 0.3;

/**
 * English with accented letters, about 30% more text and `[` `]` markers at both ends. Text that
 * is not from the catalogue shows up unmarked, clipped text loses its closing bracket, and the
 * layout meets the longer strings of other languages. `{name}` tokens are left alone.
 */
export function accentMessage(message: string): string {
	const accented = message
		.split(TOKEN)
		.map((part, index) =>
			index % 2 === 1 ? part : [...part].map((char) => ACCENTS.get(char) ?? char).join(''),
		)
		.join('');
	const visible = message.replace(/\{\w+\}/g, '').length;
	const padding = '·'.repeat(Math.ceil(visible * EXPANSION));
	return `[${accented}${padding ? ` ${padding}` : ''}]`;
}

const RIGHT_TO_LEFT_EMBEDDING = '‫';
const POP_DIRECTIONAL_FORMATTING = '‬';
const RIGHT_TO_LEFT_MARK = '‏';

const ARABIC_PUNCTUATION: Record<string, string> = { '?': '؟', ',': '،', ';': '؛' };

/**
 * English wrapped in a right-to-left embedding so the line runs from the right, with the Arabic
 * question mark, comma and semicolon and a right-to-left mark around each `{name}` token. The
 * words stay readable.
 */
export function rightToLeftMessage(message: string): string {
	const marked = message
		.split(TOKEN)
		.map((part, index) =>
			index % 2 === 1
				? `${RIGHT_TO_LEFT_MARK}${part}${RIGHT_TO_LEFT_MARK}`
				: [...part].map((char) => ARABIC_PUNCTUATION[char] ?? char).join(''),
		)
		.join('');
	return `${RIGHT_TO_LEFT_EMBEDDING}${marked}${POP_DIRECTIONAL_FORMATTING}`;
}

/** A whole catalogue through one of the transforms above. */
export function pseudoCatalogue(
	source: Catalogue,
	transform: (message: string) => string,
): Catalogue {
	const out: Catalogue = {};
	for (const [id, message] of Object.entries(source)) {
		out[id as keyof Catalogue] = transform(message as string);
	}
	return out;
}
