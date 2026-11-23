// The script editor as an object the page keeps alive: one CodeMirror view, mounted by hand and moved between renders.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { closeBrackets, closeBracketsKeymap, completionKeymap } from '@codemirror/autocomplete';
import {
	defaultKeymap,
	history,
	historyKeymap,
	indentLess,
	indentMore,
	insertTab,
} from '@codemirror/commands';
import {
	bracketMatching,
	foldGutter,
	foldKeymap,
	indentOnInput,
	indentUnit,
} from '@codemirror/language';
import { lintKeymap } from '@codemirror/lint';
import { highlightSelectionMatches, searchKeymap, search } from '@codemirror/search';
import { Annotation, Compartment, EditorState, Prec, type Extension } from '@codemirror/state';
import {
	EditorView,
	drawSelection,
	highlightActiveLine,
	highlightActiveLineGutter,
	keymap,
	lineNumbers,
} from '@codemirror/view';
import type { ScriptDiagnostic } from '../lib/types';
import { apiCompletion } from './completion';
import { checkExtension } from './diagnostics';
import { formatLua } from './format';
import { hoverDocs } from './hover';
import { luaSupport } from './luaLanguage';
import { phraseExtension } from './phrases';
import { editorTheme } from './theme';

export interface EditorSettings {
	fontSize: number;
	tabWidth: number;
	wrap: boolean;
	lineNumbers: boolean;
}

export interface EditorHooks {
	/** The text changed because of typing, a paste, undo or Format; never for `setText`. */
	onChange(text: string): void;
	/** Ctrl+S. */
	onSave(): void;
	/** Ctrl+Enter. */
	onRun(): void;
	/** Shift+Alt+F. */
	onFormat(): void;
	/** The caret moved; line and column start at 1. */
	onCursor(line: number, column: number): void;
	/** The backend answered a syntax check. */
	onDiagnostics(found: ScriptDiagnostic[]): void;
	/** Tab leaves the editor (after Esc) or returns to indenting. */
	onTabFocus(leaves: boolean): void;
	check(source: string): Promise<ScriptDiagnostic[]>;
}

export interface ScriptEditor {
	/** The element to place in the page. It keeps its state when it is moved. */
	readonly el: HTMLElement;
	readonly view: EditorView;
	text(): string;
	/** Replaces the whole text without calling `onChange`, and clears the undo history when `resetHistory` is set. */
	setText(text: string, resetHistory?: boolean): void;
	/** Runs the formatter over the text as an edit, so it can be undone. */
	format(): string;
	configure(settings: EditorSettings, label: string): void;
	focus(): void;
	destroy(): void;
}

const external = Annotation.define<boolean>();

/** The extensions that depend on the Editor settings, kept in compartments so they change without a new view. */
function settingsExtensions(s: EditorSettings): Extension[] {
	return [
		EditorState.tabSize.of(s.tabWidth),
		indentUnit.of('\t'),
		s.wrap ? EditorView.lineWrapping : [],
		s.lineNumbers ? [lineNumbers(), highlightActiveLineGutter()] : [],
	];
}

export function createScriptEditor(
	initial: { doc: string; settings: EditorSettings; label: string },
	hooks: EditorHooks,
): ScriptEditor {
	const configurable = new Compartment();
	const labelled = new Compartment();
	let tabLeaves = false;
	const setTabLeaves = (v: boolean) => {
		if (tabLeaves === v) return;
		tabLeaves = v;
		hooks.onTabFocus(v);
	};

	const label = (text: string) =>
		EditorView.contentAttributes.of({
			'aria-label': text,
			'aria-multiline': 'true',
			spellcheck: 'false',
		});
	const run = (fn: () => void) => () => {
		fn();
		return true;
	};
	let settings = initial.settings;
	let ariaLabel = initial.label;
	const build = (): Extension[] => [
		phraseExtension(),
		luaSupport(),
		configurable.of(settingsExtensions(settings)),
		labelled.of(label(ariaLabel)),
		foldGutter(),
		drawSelection(),
		EditorState.allowMultipleSelections.of(true),
		bracketMatching(),
		closeBrackets(),
		indentOnInput(),
		highlightActiveLine(),
		highlightSelectionMatches(),
		search({ top: true }),
		history(),
		apiCompletion(),
		hoverDocs(),
		checkExtension({ check: hooks.check, onResult: hooks.onDiagnostics }),
		editorTheme,
		Prec.highest(
			keymap.of([
				{ key: 'Mod-s', run: run(hooks.onSave), preventDefault: true },
				{ key: 'Mod-Enter', run: run(hooks.onRun), preventDefault: true },
				{ key: 'Shift-Alt-f', run: run(hooks.onFormat), preventDefault: true },
			]),
		),
		keymap.of([
			...closeBracketsKeymap,
			...historyKeymap,
			...completionKeymap,
			...foldKeymap,
			...searchKeymap,
			// Tab indents; Esc then Tab moves focus on, so the keyboard is never trapped.
			{
				key: 'Tab',
				run: (v) =>
					tabLeaves
						? false
						: v.state.selection.ranges.some((r) => !r.empty)
							? indentMore(v)
							: insertTab(v),
			},
			{ key: 'Shift-Tab', run: (v) => (tabLeaves ? false : indentLess(v)) },
			...lintKeymap.filter((b) => b.key !== 'Mod-Shift-m'),
			...defaultKeymap,
			{
				key: 'Escape',
				run: () => {
					setTabLeaves(true);
					return true;
				},
			},
		]),
		EditorView.domEventHandlers({
			keydown(event) {
				if (event.key !== 'Tab' && event.key !== 'Escape' && event.key !== 'Shift')
					setTabLeaves(false);
				return false;
			},
			blur() {
				setTabLeaves(false);
				return false;
			},
		}),
		EditorView.updateListener.of((update) => {
			if (update.docChanged && !update.transactions.some((tr) => tr.annotation(external)))
				hooks.onChange(update.state.doc.toString());
			if (update.selectionSet || update.docChanged) {
				const head = update.state.selection.main.head;
				const line = update.state.doc.lineAt(head);
				hooks.onCursor(line.number, head - line.from + 1);
			}
		}),
	];

	const el = document.createElement('div');
	el.className = 'script-editor';
	const view = new EditorView({
		parent: el,
		state: EditorState.create({ doc: initial.doc, extensions: build() }),
	});

	return {
		el,
		view,
		text: () => view.state.doc.toString(),
		setText(text, resetHistory = false) {
			if (view.state.doc.toString() === text && !resetHistory) return;
			if (resetHistory) {
				// A new state drops the undo history, as opening another script should.
				view.setState(EditorState.create({ doc: text, extensions: build() }));
				return;
			}
			view.dispatch({
				changes: { from: 0, to: view.state.doc.length, insert: text },
				annotations: external.of(true),
			});
		},
		format() {
			const before = view.state.doc.toString();
			const after = formatLua(before);
			if (after !== before) {
				const head = Math.min(view.state.selection.main.head, after.length);
				view.dispatch({
					changes: { from: 0, to: before.length, insert: after },
					selection: { anchor: head },
				});
			}
			return after;
		},
		configure(next, text) {
			settings = next;
			ariaLabel = text;
			view.dispatch({
				effects: [
					configurable.reconfigure(settingsExtensions(next)),
					labelled.reconfigure(label(text)),
				],
			});
			el.style.setProperty('--editor-font-size', `${next.fontSize}px`);
		},
		focus: () => view.focus(),
		destroy: () => view.destroy(),
	};
}
