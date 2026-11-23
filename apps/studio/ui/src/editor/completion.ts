// Suggests the script API as it is typed: modules after `li.`, hooks on a blank line, capabilities in the header.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { autocompletion, snippetCompletion, type Completion, type CompletionContext, type CompletionResult } from '@codemirror/autocomplete';
import { syntaxTree } from '@codemirror/language';
import type { EditorState, Extension } from '@codemirror/state';
import { docElement } from './docElement';
import { API_DOCS, CAPABILITY_NAMES, KIND_MESSAGE, capabilityDoc, docFor, hookSignature, type ApiDoc, type DocKind } from './luaDocs';
import { t } from './phrases';

const TYPES: Record<DocKind, string> = {
	module: 'namespace',
	function: 'function',
	hook: 'function',
	field: 'property',
	capability: 'constant',
	keyword: 'keyword',
	global: 'variable',
	variable: 'variable',
};

export const LUA_KEYWORDS = [
	'and', 'break', 'do', 'else', 'elseif', 'end', 'false', 'for', 'function', 'goto', 'if', 'in', 'local', 'nil', 'not', 'or', 'repeat', 'return', 'then', 'true', 'until', 'while',
];

/** The globals and library tables a script can name, apart from what `API_DOCS` lists as dotted names. */
const LIBRARIES = ['string', 'table', 'math', 'utf8'];

function option(entry: ApiDoc, label: string): Completion {
	return {
		label,
		type: TYPES[entry.kind],
		detail: entry.capability ? `${t(KIND_MESSAGE[entry.kind])} · ${entry.capability}` : t(KIND_MESSAGE[entry.kind]),
		info: () => docElement(entry),
	};
}

function hookOption(entry: ApiDoc): Completion {
	const param = hookSignature(entry.name).slice(entry.name.length + 1, -1);
	return snippetCompletion(`function ${entry.name}(${param})\n\t\${}\nend`, {
		label: entry.name,
		type: 'function',
		detail: hookSignature(entry.name),
		info: () => docElement(entry),
		boost: 2,
	});
}

/** The dotted names one level below `parent`, such as `li.env.get` under `li.env`. */
function members(parent: string): ApiDoc[] {
	const prefix = `${parent}.`;
	return API_DOCS.filter((d) => d.name.startsWith(prefix) && !d.name.slice(prefix.length).includes('.'));
}

export interface Suggestion {
	from: number;
	options: Completion[];
	/** What the typed word must match to keep this list open. */
	validFor: RegExp;
}

function insideCommentOrString(state: EditorState, pos: number): boolean {
	const name = syntaxTree(state).resolveInner(pos, -1).name;
	return /comment|string/i.test(name);
}

/** The suggestions for the caret at `pos`, or null when none fit. */
export function suggest(state: EditorState, pos: number, explicit = false): Suggestion | null {
	const line = state.doc.lineAt(pos);
	const before = line.text.slice(0, pos - line.from);

	const header = /^\s*--+\s*capabilities:(.*)$/.exec(before);
	if (header) {
		const word = /[\w.]*$/.exec(before)![0];
		const named = header[1]!.split(/[,\s]+/).filter(Boolean);
		const options = CAPABILITY_NAMES.filter((c) => !named.includes(c) || c === word).map((c) => {
			const doc = capabilityDoc(c);
			return { label: c, type: 'constant', detail: t('kind.capability'), ...(doc ? { info: () => docElement(doc) } : {}) } as Completion;
		});
		return { from: pos - word.length, options, validFor: /^[\w.]*$/ };
	}
	if (insideCommentOrString(state, pos)) return null;

	const member = /([A-Za-z_]\w*(?:[.:][A-Za-z_]\w*)*)[.:](\w*)$/.exec(before);
	if (member) {
		const parent = member[1]!.replace(/:/g, '.');
		const found = members(parent);
		if (found.length === 0) return null;
		return { from: pos - member[2]!.length, options: found.map((d) => option(d, d.name.slice(parent.length + 1))), validFor: /^\w*$/ };
	}

	const word = /[A-Za-z_]\w*$/.exec(before);
	if (!word && !explicit) return null;
	const typed = word ? word[0] : '';
	const alone = before.trim() === typed;
	const options: Completion[] = [];
	if (alone) options.push(...API_DOCS.filter((d) => d.kind === 'hook').map(hookOption));
	for (const d of API_DOCS) {
		if (d.kind === 'global' || d.kind === 'variable') options.push(option(d, d.name));
		else if (!d.name.includes('.') && !d.name.includes(':') && d.kind === 'function') options.push(option(d, d.name));
	}
	for (const lib of LIBRARIES) options.push({ label: lib, type: 'namespace', detail: t('kind.module') });
	for (const k of LUA_KEYWORDS) options.push({ label: k, type: 'keyword', detail: t('kind.keyword'), boost: -1 });
	return { from: pos - typed.length, options, validFor: /^\w*$/ };
}

/** Library members such as `string.format` come from `API_DOCS`; this reaches them after `string.`. */
export function libraryMembers(name: string): ApiDoc[] {
	return members(name);
}

function source(context: CompletionContext): CompletionResult | null {
	const found = suggest(context.state, context.pos, context.explicit);
	if (!found || found.options.length === 0) return null;
	return found;
}

export const apiCompletion = (): Extension => autocompletion({ override: [source], icons: false, maxRenderedOptions: 80 });

export { docFor };
