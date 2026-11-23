// The editor library's own words, from the message catalogue.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { EditorState } from '@codemirror/state';
import { t, type MessageId } from '../i18n/messages';

/** The editor library's English phrase to the catalogue message that replaces it. */
export const CM_PHRASES: Record<string, MessageId> = {
	'Fold line': 'editor.cm.foldLine',
	'Unfold line': 'editor.cm.unfoldLine',
	'Folded lines': 'editor.cm.foldedLines',
	'Unfolded lines': 'editor.cm.unfoldedLines',
	'folded code': 'editor.cm.foldedCode',
	unfold: 'editor.cm.unfold',
	to: 'editor.cm.to',
	'Go to line': 'editor.cm.goToLine',
	go: 'editor.cm.go',
	Find: 'editor.cm.find',
	Replace: 'editor.cm.replace',
	next: 'editor.cm.next',
	previous: 'editor.cm.previous',
	all: 'editor.cm.all',
	'match case': 'editor.cm.matchCase',
	regexp: 'editor.cm.regexp',
	'by word': 'editor.cm.byWord',
	replace: 'editor.cm.replaceButton',
	'replace all': 'editor.cm.replaceAll',
	close: 'editor.cm.close',
	'current match': 'editor.cm.currentMatch',
	'on line': 'editor.cm.onLine',
	'replaced match on line $': 'editor.cm.replacedMatchOnLine',
	'replaced $ matches': 'editor.cm.replacedMatches',
	Diagnostics: 'editor.cm.diagnostics',
	'No diagnostics': 'editor.cm.noDiagnostics',
	Completions: 'editor.cm.completions',
	'Selection deleted': 'editor.cm.selectionDeleted',
	'Control character': 'editor.cm.controlCharacter',
};

/** The phrase table the editor library reads: its own English phrase to ours. */
export function editorPhrases(): Record<string, string> {
	const out: Record<string, string> = {};
	for (const [phrase, id] of Object.entries(CM_PHRASES)) out[phrase] = t(id);
	return out;
}

export const phraseExtension = () => EditorState.phrases.of(editorPhrases());
