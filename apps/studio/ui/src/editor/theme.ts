// The editor's look, written only in the Studio's design tokens so it follows the theme as it changes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `--lh-*` are the design tokens; `--ed-*` (syntax colours) are set per theme in `app.css` from those tokens.

import { EditorView } from '@codemirror/view';

const wash = (percent: number) => `color-mix(in srgb, var(--lh-accent) ${percent}%, transparent)`;

export const editorTheme = EditorView.theme({
	'&': {
		color: 'var(--lh-text-primary)',
		backgroundColor: 'var(--lh-bg-sidebar)',
		fontSize: 'var(--editor-font-size, 13px)',
		height: '100%',
	},
	'&.cm-focused': { outline: '2px solid var(--lh-border-focus)', outlineOffset: '-2px' },
	'.cm-scroller': { fontFamily: 'var(--lh-font-mono)', lineHeight: '1.5' },
	'.cm-content': { caretColor: 'var(--lh-text-bright)' },
	'.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--lh-text-bright)' },
	'.cm-selectionBackground, .cm-content ::selection': { backgroundColor: wash(22) },
	'&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground': { backgroundColor: wash(34) },
	'.cm-activeLine': { backgroundColor: 'var(--lh-bg-hover-strong)' },
	'.cm-selectionMatch': { backgroundColor: wash(16) },
	'.cm-matchingBracket': { backgroundColor: wash(24), outline: '1px solid var(--lh-accent)' },
	'.cm-nonmatchingBracket': { outline: '1px solid var(--lh-colour-error)' },
	'.cm-searchMatch': { backgroundColor: 'color-mix(in srgb, var(--lh-colour-warning) 30%, transparent)', outline: '1px solid var(--lh-colour-warning)' },
	'.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'color-mix(in srgb, var(--lh-colour-warning) 50%, transparent)' },
	'.cm-gutters': { color: 'var(--lh-text-muted)', backgroundColor: 'var(--lh-bg-sidebar)', borderRight: '1px solid var(--lh-border-subtle)' },
	'.cm-activeLineGutter': { color: 'var(--lh-text-bright)', backgroundColor: 'var(--lh-bg-hover-strong)' },
	'.cm-foldPlaceholder': { color: 'var(--lh-text-secondary)', backgroundColor: 'var(--lh-bg-hover)', border: '1px solid var(--lh-border-subtle)' },
	'.cm-lintRange-error': { backgroundImage: 'none', textDecoration: 'underline wavy var(--lh-colour-error)', textUnderlineOffset: '3px' },
	'.cm-lintRange-warning': { backgroundImage: 'none', textDecoration: 'underline wavy var(--lh-colour-warning)', textUnderlineOffset: '3px' },
	'.cm-lint-marker': { width: '0.7em', height: '0.7em', content: 'none', backgroundImage: 'none', borderRadius: '50%' },
	'.cm-lint-marker-error': { backgroundColor: 'var(--lh-colour-error)' },
	'.cm-lint-marker-warning': { backgroundColor: 'var(--lh-colour-warning)', borderRadius: '2px' },
	'.cm-tooltip': { color: 'var(--lh-text-primary)', backgroundColor: 'var(--lh-bg-tooltip)', border: '1px solid var(--lh-border-default)', borderRadius: 'var(--lh-radius-sm)' },
	'.cm-tooltip:has(.ed-doc)': { pointerEvents: 'none' },
	'.cm-tooltip.cm-tooltip-lint': { maxWidth: '32rem' },
	'.cm-diagnostic': { color: 'var(--lh-text-primary)' },
	'.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--lh-font-mono)' },
	'.cm-tooltip-autocomplete > ul > li[aria-selected]': { color: 'var(--lh-text-bright)', backgroundColor: wash(24) },
	'.cm-completionDetail': { color: 'var(--lh-text-muted)', fontStyle: 'normal' },
	'.cm-completionInfo': { color: 'var(--lh-text-primary)', backgroundColor: 'var(--lh-bg-tooltip)', border: '1px solid var(--lh-border-default)' },
	'.ed-doc': { padding: '6px 10px', maxWidth: '28rem', lineHeight: '1.4', fontFamily: 'var(--lh-font-body, inherit)' },
	'.ed-doc-name': { fontWeight: '700', fontFamily: 'var(--lh-font-mono)' },
	'.ed-doc-kind': { marginLeft: '8px', color: 'var(--lh-text-muted)' },
	'.ed-doc-note': { margin: '4px 0 0' },
	'.ed-doc-label': { display: 'block', marginTop: '6px', fontSize: '0.8em', color: 'var(--lh-text-muted)' },
	'.ed-doc code': { display: 'block', whiteSpace: 'pre-wrap', fontFamily: 'var(--lh-font-mono)', color: 'var(--ed-keyword)' },
	'.ed-doc-needs': { display: 'block', marginTop: '6px', fontSize: '0.8em', color: 'var(--lh-text-secondary)' },
	'.cm-panels': { color: 'var(--lh-text-primary)', backgroundColor: 'var(--lh-bg-panel)' },
	'.cm-panels-top': { borderBottom: '1px solid var(--lh-border-subtle)' },
	'.cm-panels-bottom': { borderTop: '1px solid var(--lh-border-subtle)' },
	'.cm-panel.cm-search input, .cm-panel.cm-gotoLine input': { color: 'var(--lh-text-primary)', backgroundColor: 'var(--lh-bg-input)', border: '1px solid var(--lh-border-default)', borderRadius: 'var(--lh-radius-sm)' },
	'.cm-panel.cm-search input:focus-visible': { outline: '2px solid var(--lh-border-focus)', outlineOffset: '1px' },
	'.cm-panel button': { color: 'var(--lh-text-primary)', backgroundColor: 'var(--lh-bg-hover)', backgroundImage: 'none', border: '1px solid var(--lh-border-default)', borderRadius: 'var(--lh-radius-sm)' },
	'.cm-panel button:hover': { backgroundColor: 'var(--lh-bg-hover-strong)' },
	'.cm-panel label': { color: 'var(--lh-text-secondary)' },
});
