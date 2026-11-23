// The editor's look, written only in the Studio's design tokens so it follows the theme as it changes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `--lis-*` are the design tokens; `--lis-ed-*` (syntax colours) are set per theme in `app.css` from those tokens.

import { EditorView } from '@codemirror/view';

const wash = (percent: number) => `color-mix(in srgb, var(--lis-accent) ${percent}%, transparent)`;

export const editorTheme = EditorView.theme({
	'&': {
		color: 'var(--lis-text-primary)',
		backgroundColor: 'var(--lis-bg-sidebar)',
		fontSize: 'var(--editor-font-size, 13px)',
		height: '100%',
	},
	'&.cm-focused': { outline: '2px solid var(--lis-border-focus)', outlineOffset: '-2px' },
	'.cm-scroller': { fontFamily: 'var(--lis-font-mono)', lineHeight: '1.5' },
	'.cm-content': { caretColor: 'var(--lis-text-bright)' },
	'.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--lis-text-bright)' },
	'.cm-selectionBackground, .cm-content ::selection': { backgroundColor: wash(22) },
	'&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground': {
		backgroundColor: wash(34),
	},
	'.cm-activeLine': { backgroundColor: 'var(--lis-bg-hover-strong)' },
	'.cm-selectionMatch': { backgroundColor: wash(16) },
	'.cm-matchingBracket': { backgroundColor: wash(24), outline: '1px solid var(--lis-accent)' },
	'.cm-nonmatchingBracket': { outline: '1px solid var(--lis-colour-error)' },
	'.cm-searchMatch': {
		backgroundColor: 'color-mix(in srgb, var(--lis-colour-warning) 30%, transparent)',
		outline: '1px solid var(--lis-colour-warning)',
	},
	'.cm-searchMatch.cm-searchMatch-selected': {
		backgroundColor: 'color-mix(in srgb, var(--lis-colour-warning) 50%, transparent)',
	},
	'.cm-gutters': {
		color: 'var(--lis-text-muted)',
		backgroundColor: 'var(--lis-bg-sidebar)',
		borderRight: '1px solid var(--lis-border-subtle)',
	},
	'.cm-activeLineGutter': {
		color: 'var(--lis-text-bright)',
		backgroundColor: 'var(--lis-bg-hover-strong)',
	},
	'.cm-foldPlaceholder': {
		color: 'var(--lis-text-secondary)',
		backgroundColor: 'var(--lis-bg-hover)',
		border: '1px solid var(--lis-border-subtle)',
	},
	'.cm-lintRange-error': {
		backgroundImage: 'none',
		textDecoration: 'underline wavy var(--lis-colour-error)',
		textUnderlineOffset: '3px',
	},
	'.cm-lintRange-warning': {
		backgroundImage: 'none',
		textDecoration: 'underline wavy var(--lis-colour-warning)',
		textUnderlineOffset: '3px',
	},
	'.cm-lint-marker': {
		width: '0.7em',
		height: '0.7em',
		content: 'none',
		backgroundImage: 'none',
		borderRadius: '50%',
	},
	'.cm-lint-marker-error': { backgroundColor: 'var(--lis-colour-error)' },
	'.cm-lint-marker-warning': { backgroundColor: 'var(--lis-colour-warning)', borderRadius: '2px' },
	'.cm-tooltip': {
		color: 'var(--lis-text-primary)',
		backgroundColor: 'var(--lis-bg-tooltip)',
		border: '1px solid var(--lis-border-default)',
		borderRadius: 'var(--lis-radius-sm)',
	},
	'.cm-tooltip:has(.ed-doc)': { pointerEvents: 'none' },
	'.cm-tooltip.cm-tooltip-lint': { maxWidth: '32rem' },
	'.cm-diagnostic': { color: 'var(--lis-text-primary)' },
	'.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--lis-font-mono)' },
	'.cm-tooltip-autocomplete > ul > li[aria-selected]': {
		color: 'var(--lis-text-bright)',
		backgroundColor: wash(24),
	},
	'.cm-completionDetail': { color: 'var(--lis-text-muted)', fontStyle: 'normal' },
	'.cm-completionInfo': {
		color: 'var(--lis-text-primary)',
		backgroundColor: 'var(--lis-bg-tooltip)',
		border: '1px solid var(--lis-border-default)',
	},
	'.ed-doc': {
		padding: '6px 10px',
		maxWidth: '28rem',
		lineHeight: '1.4',
		fontFamily: 'var(--lis-font-body, inherit)',
	},
	'.ed-doc-name': { fontWeight: '700', fontFamily: 'var(--lis-font-mono)' },
	'.ed-doc-kind': { marginLeft: '8px', color: 'var(--lis-text-muted)' },
	'.ed-doc-note': { margin: '4px 0 0' },
	'.ed-doc-label': {
		display: 'block',
		marginTop: '6px',
		fontSize: '0.8em',
		color: 'var(--lis-text-muted)',
	},
	'.ed-doc code': {
		display: 'block',
		whiteSpace: 'pre-wrap',
		fontFamily: 'var(--lis-font-mono)',
		color: 'var(--lis-ed-keyword)',
	},
	'.ed-doc-needs': {
		display: 'block',
		marginTop: '6px',
		fontSize: '0.8em',
		color: 'var(--lis-text-secondary)',
	},
	'.cm-panels': { color: 'var(--lis-text-primary)', backgroundColor: 'var(--lis-bg-panel)' },
	'.cm-panels-top': { borderBottom: '1px solid var(--lis-border-subtle)' },
	'.cm-panels-bottom': { borderTop: '1px solid var(--lis-border-subtle)' },
	'.cm-panel.cm-search input, .cm-panel.cm-gotoLine input': {
		color: 'var(--lis-text-primary)',
		backgroundColor: 'var(--lis-bg-input)',
		border: '1px solid var(--lis-border-default)',
		borderRadius: 'var(--lis-radius-sm)',
	},
	'.cm-panel.cm-search input:focus-visible': {
		outline: '2px solid var(--lis-border-focus)',
		outlineOffset: '1px',
	},
	'.cm-panel button': {
		color: 'var(--lis-text-primary)',
		backgroundColor: 'var(--lis-bg-hover)',
		backgroundImage: 'none',
		border: '1px solid var(--lis-border-default)',
		borderRadius: 'var(--lis-radius-sm)',
	},
	'.cm-panel button:hover': { backgroundColor: 'var(--lis-bg-hover-strong)' },
	'.cm-panel label': { color: 'var(--lis-text-secondary)' },
});
