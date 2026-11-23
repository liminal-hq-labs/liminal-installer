// The editor's words: every string the script editor shows, in one catalogue that a language can replace.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { EditorState } from '@codemirror/state';

/** The English catalogue. Ids are stable; a language supplies the same ids. */
export const ENGLISH = {
	'editor.label': 'Lua source of {path}',
	'editor.tabFocus': 'Tab now moves focus out of the editor',
	'kind.module': 'module',
	'kind.function': 'function',
	'kind.hook': 'hook',
	'kind.field': 'field',
	'kind.capability': 'capability',
	'kind.keyword': 'keyword',
	'kind.global': 'global',
	'kind.variable': 'variable',
	'doc.syntax': 'Syntax',
	'doc.example': 'Example',
	'doc.needs': 'Needs the `{capability}` capability',
	'doc.popup': 'Documentation',
	'lint.syntax': 'Syntax error: {message}',
	'status.position': 'Ln {line}, Col {column}',
	'status.lua': 'Lua 5.4 · retro subset checked',
	'status.clean': 'No problems',
	'status.problems': '{errors} errors, {warnings} warnings',
	'status.problems.one': '1 error, {warnings} warnings',
	'status.warnings.one': '{errors} errors, 1 warning',
	'status.both.one': '1 error, 1 warning',
	// The editor library's own words.
	'cm.Fold line': 'Fold line',
	'cm.Unfold line': 'Unfold line',
	'cm.Folded lines': 'Folded lines',
	'cm.Unfolded lines': 'Unfolded lines',
	'cm.folded code': 'folded code',
	'cm.unfold': 'unfold',
	'cm.to': 'to',
	'cm.Go to line': 'Go to line',
	'cm.go': 'go',
	'cm.Find': 'Find',
	'cm.Replace': 'Replace',
	'cm.next': 'next',
	'cm.previous': 'previous',
	'cm.all': 'all',
	'cm.match case': 'match case',
	'cm.regexp': 'regexp',
	'cm.by word': 'by word',
	'cm.replace': 'replace',
	'cm.replace all': 'replace all',
	'cm.close': 'close',
	'cm.current match': 'current match',
	'cm.on line': 'on line',
	'cm.replaced match on line $': 'replaced match on line $',
	'cm.replaced $ matches': 'replaced $ matches',
	'cm.Diagnostics': 'Problems',
	'cm.No diagnostics': 'No problems',
	'cm.Completions': 'Completions',
	'cm.Selection deleted': 'Selection deleted',
	'cm.Control character': 'Control character',
} as const;

export type MessageId = keyof typeof ENGLISH;

let language: Partial<Record<MessageId, string>> = {};

/** Replaces the words; an id the language lacks keeps its English text. */
export function setLanguage(messages: Partial<Record<MessageId, string>>): void {
	language = messages;
}

/** The text for an id, with `{name}` placeholders filled from `vars`. */
export function t(id: MessageId, vars: Record<string, string | number> = {}): string {
	const text = language[id] ?? ENGLISH[id];
	return text.replace(/\{(\w+)\}/g, (whole, name: string) => (name in vars ? String(vars[name]) : whole));
}

/** The phrase table the editor library reads: its own English phrase to ours. */
export function editorPhrases(): Record<string, string> {
	const out: Record<string, string> = {};
	for (const id of Object.keys(ENGLISH) as MessageId[]) {
		if (id.startsWith('cm.')) out[id.slice(3)] = t(id);
	}
	return out;
}

export const phraseExtension = () => EditorState.phrases.of(editorPhrases());
