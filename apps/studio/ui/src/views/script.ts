// The Script screen: a Lua editor with completion and diagnostics, the hook and capability lists, and the test console.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Ctx, State } from '../app';
import { countFindings } from '../editor/diagnostics';
import { HOOK_PARAMS, hookSignature } from '../editor/luaDocs';
import { t } from '../editor/phrases';
import { createScriptEditor, type EditorSettings, type ScriptEditor } from '../editor/scriptEditor';
import { h } from '../lib/dom';
import type { Settings } from '../lib/settings';
import type { ScriptDiagnostic } from '../lib/types';

const CAPABILITY_NOTES: Record<string, string> = {
	'li.env': 'Read the environment and system facts',
	'li.fs': 'Read anywhere; write only through the plan',
	'li.ui': 'Notes and dialogs',
	'li.plan': 'Add plan operations',
	'li.net': 'Network access; needs the `net` capability',
	'li.retro.sound': 'Sound probing; retro targets only',
	'li.retro.video': 'Video modes; retro targets only',
};

interface Held {
	editor: ScriptEditor;
	line: number;
	column: number;
	found: ScriptDiagnostic[];
	tabLeaves: boolean;
	/** Read before a redraw takes the editor out of the page: it had the focus, or was not on screen yet. */
	hadFocus: boolean;
}

/** One editor per app, made when the screen is first drawn and kept while the page lives. */
const held = new WeakMap<State, Held>();

/** The live editor, if the Script screen has made one. */
export function peekEditor(state: State): ScriptEditor | undefined {
	return held.get(state)?.editor;
}

const settingsOf = (
	s: Pick<Settings, 'editorFontSize' | 'tabWidth' | 'wrapLines' | 'lineNumbers'>,
): EditorSettings => ({
	fontSize: s.editorFontSize,
	tabWidth: s.tabWidth,
	wrap: s.wrapLines,
	lineNumbers: s.lineNumbers,
});

const labelFor = (state: State) => t('editor.label', { path: state.scriptPath });

function editorFor(ctx: Ctx): Held {
	const state = ctx.state;
	const existing = held.get(state);
	if (existing) return existing;
	const entry: Held = {
		editor: undefined as unknown as ScriptEditor,
		line: 1,
		column: 1,
		found: [],
		tabLeaves: false,
		hadFocus: true,
	};
	entry.editor = createScriptEditor(
		{ doc: state.scriptDraft, settings: settingsOf(ctx.settings()), label: labelFor(state) },
		{
			onChange: (text) => ctx.setDraft(text),
			onSave: () => void ctx.run(() => ctx.saveScript()),
			onRun: () => void ctx.run(() => ctx.testScript()),
			onFormat: () => ctx.formatDraft(),
			onCursor: (line, column) => {
				entry.line = line;
				entry.column = column;
				refreshFooter(entry);
			},
			onDiagnostics: (found) => {
				entry.found = found;
				refreshFooter(entry);
			},
			onTabFocus: (leaves) => {
				entry.tabLeaves = leaves;
				refreshFooter(entry);
			},
			check: (source) => ctx.api.checkScriptSyntax(source),
		},
	);
	held.set(state, entry);
	return entry;
}

function problemsText(found: ScriptDiagnostic[]): string {
	const { errors, warnings } = countFindings(found);
	if (errors === 0 && warnings === 0) return t('status.clean');
	const e = errors === 1 ? '1 error' : `${errors} errors`;
	const w = warnings === 1 ? '1 warning' : `${warnings} warnings`;
	return [errors ? e : '', warnings ? w : ''].filter(Boolean).join(', ');
}

function refreshFooter(entry: Held): void {
	const set = (id: string, text: string) => {
		const el = document.getElementById(id);
		if (el) el.textContent = text;
	};
	set('script-pos', t('status.position', { line: entry.line, column: entry.column }));
	set('script-problems', problemsText(entry.found));
	set('script-tab-hint', entry.tabLeaves ? t('editor.tabFocus') : '');
	document.getElementById('script-problems')?.classList.toggle(
		'err',
		entry.found.some((d) => d.severity === 'error'),
	);
}

const clock = (d: Date) =>
	[d.getHours(), d.getMinutes(), d.getSeconds()].map((n) => String(n).padStart(2, '0')).join(':');

/** True when the run on screen no longer matches what is in the editor or the chosen target. */
export function isStale(s: State): boolean {
	const run = s.testRun;
	return (
		!!run &&
		run.status !== 'running' &&
		(run.text !== s.scriptDraft || run.target !== s.scriptTarget)
	);
}

/** The line beside the Test run button. */
export function testStatus(s: State): string {
	const run = s.testRun;
	if (!run) return '';
	if (run.status === 'running') return 'Running…';
	const when = clock(run.at ?? new Date());
	const base =
		run.status === 'error'
			? `Could not run at ${when}`
			: `Tested at ${when} · ${run.ms ?? 0} ms (${run.unsaved ? 'unsaved text' : 'saved file'})`;
	return isStale(s) ? `${base} · out of date` : base;
}

/** Updates the labels that follow typing, without redrawing the screen or touching the editor. */
export function refreshScriptStatus(s: State): void {
	const dirty = document.getElementById('script-dirty');
	if (dirty) dirty.textContent = s.scriptDirty ? 'Unsaved changes' : 'Saved';
	const status = document.getElementById('script-test-status');
	if (status) status.textContent = testStatus(s);
	const stale = isStale(s);
	document.getElementById('script-console')?.classList.toggle('stale', stale);
	const note = document.getElementById('script-stale');
	if (note) note.hidden = !stale;
}

/** Applies the Editor settings to the editor, if there is one, without touching what is typed in it. */
export function applyEditorSettings(
	ctx: Ctx,
	settings: Pick<Settings, 'editorFontSize' | 'tabWidth' | 'wrapLines' | 'lineNumbers'>,
): void {
	held.get(ctx.state)?.editor.configure(settingsOf(settings), labelFor(ctx.state));
}

/** Called after the screen is in the page: puts the settings and the focus back on the editor. */
export function afterScriptRender(ctx: Ctx): void {
	const entry = held.get(ctx.state);
	if (!entry) return;
	applyEditorSettings(ctx, ctx.settings());
	entry.editor.view.requestMeasure();
	refreshFooter(entry);
	if (entry.hadFocus) entry.editor.focus();
}

function consoleView(s: State): HTMLElement {
	const run = s.testRun;
	const where = run ? (run.target ? `on ${run.target}` : 'on the sample machine') : '';
	const body: (HTMLElement | string)[] = [];
	if (!run) {
		body.push(h('p', { class: 'dim small' }, 'Not run yet. Press Test run or Ctrl+Enter.'));
	} else if (run.status === 'running') {
		body.push(h('p', { class: 'dim' }, 'Running…'));
	} else if (run.status === 'error') {
		body.push(
			h('p', { class: 'err', role: 'alert' }, `✗ ${run.error ?? 'The test could not run.'}`),
		);
	} else if (run.report) {
		const r = run.report;
		const tests = r.tests ?? [];
		for (const test of tests) {
			body.push(
				h(
					'div',
					{ class: `line ${test.ok ? 'ok' : 'err'}` },
					h('span', { class: 'mark' }, test.ok ? '✓' : '✗'),
					` ${test.hook}(${HOOK_PARAMS[test.hook] ?? 'page'}) — ${test.summary}`,
				),
			);
			for (const line of test.lines) body.push(h('div', { class: 'line sub dim' }, line));
		}
		if (tests.length === 0)
			for (const line of r.output) body.push(h('div', { class: 'line' }, line));
		const hookErrors = new Set(tests.filter((x) => !x.ok).map((x) => x.summary));
		for (const d of r.diagnostics) {
			if (d.severity === 'error' && [...hookErrors].some((m) => d.message.endsWith(m))) continue;
			body.push(
				h(
					'div',
					{ class: `line ${d.severity === 'error' ? 'err' : 'warn'}` },
					h('span', { class: 'mark' }, d.severity === 'error' ? '✗' : '!'),
					` ${d.message}`,
				),
			);
		}
		const passed = tests.filter((x) => x.ok).length;
		const failed = tests.length - passed;
		const errors = r.diagnostics.filter((d) => d.severity === 'error').length;
		body.push(
			h(
				'div',
				{ class: `summary ${r.ok ? 'ok' : 'err'}` },
				r.ok ? `${passed} passed` : `${failed || errors} failed · ${passed} passed`,
			),
		);
	}
	return h(
		'section',
		{
			id: 'script-console',
			class: `console ${isStale(s) ? 'stale' : ''}`,
			'aria-label': 'Test console',
		},
		h(
			'div',
			{ class: 'console-head' },
			h('span', { class: 'dim small' }, `Console${where ? ` · test run ${where}` : ''}`),
		),
		h(
			'p',
			{ id: 'script-stale', class: 'warn small', hidden: !isStale(s) },
			'The script or target has changed since this run.',
		),
		h('div', { class: 'console-body', 'aria-live': 'polite' }, body),
	);
}

export function scriptView(ctx: Ctx): HTMLElement {
	const s = ctx.state;
	const sum = s.summary!;
	const entry = s.scriptPath ? editorFor(ctx) : undefined;
	if (entry) entry.hadFocus = entry.editor.view.hasFocus || !entry.editor.el.isConnected;
	const r = s.report;
	const targets = sum.targets.map((x) => x.full_id);
	const frame = entry ? h('div', { class: 'editor-frame' }, entry.editor.el) : null;
	const busy = s.testRun?.status === 'running';
	return h(
		'div',
		{ class: 'screen' },
		h(
			'div',
			{ class: 'screen-head' },
			h(
				'div',
				{},
				h('h1', {}, 'Script'),
				h(
					'p',
					{ class: 'muted' },
					'Lua 5.4. A script names the capabilities it needs; everything else is refused.',
				),
			),
		),
		h(
			'div',
			{ class: 'toolbar' },
			h(
				'select',
				{
					'aria-label': 'Script',
					on: {
						change: (e) =>
							void ctx.run(() => ctx.selectScript((e.target as HTMLSelectElement).value)),
					},
				},
				sum.scripts.length === 0
					? h('option', {}, '(no scripts)')
					: sum.scripts.map((p) => h('option', { value: p, selected: p === s.scriptPath }, p)),
			),
			h(
				'button',
				{
					class: 'btn',
					on: {
						click: () => {
							const name = window.prompt('New script name (letters, digits, - and _)', 'my-script');
							if (name && /^[A-Za-z0-9_-]+$/.test(name))
								void ctx.run(async () => {
									const path = `scripts/${name}.lua`;
									await ctx.api.writeScript(
										path,
										`-- ${name}\n--\n-- capabilities: env\n\nfunction detect(page)\nend\n`,
										`Create ${path}`,
									);
									await ctx.selectScript(path);
								});
						},
					},
				},
				'New script',
			),
			h(
				'button',
				{
					class: 'btn primary',
					disabled: !s.scriptPath,
					title: 'Save (Ctrl+S)',
					on: { click: () => void ctx.run(() => ctx.saveScript()) },
				},
				'Save',
			),
			h(
				'button',
				{
					class: 'btn',
					disabled: !s.scriptPath,
					title: 'Format (Shift+Alt+F)',
					on: { click: () => ctx.formatDraft() },
				},
				'Format',
			),
			h(
				'label',
				{ class: 'dim small target-pick' },
				'Test on ',
				h(
					'select',
					{
						'aria-label': 'Test target',
						disabled: !s.scriptPath,
						on: {
							change: (e) => {
								s.scriptTarget = (e.target as HTMLSelectElement).value;
								refreshScriptStatus(s);
							},
						},
					},
					h('option', { value: '', selected: s.scriptTarget === '' }, 'Sample machine'),
					targets.map((id) => h('option', { value: id, selected: id === s.scriptTarget }, id)),
				),
			),
			h(
				'button',
				{
					class: 'btn',
					disabled: !s.scriptPath || busy,
					title: 'Test run (Ctrl+Enter)',
					on: { click: () => void ctx.run(() => ctx.testScript()) },
				},
				busy ? 'Running…' : 'Test run',
			),
			h('span', { id: 'script-test-status', class: 'dim small', role: 'status' }, testStatus(s)),
			h(
				'span',
				{ id: 'script-dirty', class: 'dim small' },
				s.scriptDirty ? 'Unsaved changes' : 'Saved',
			),
		),
		h(
			'div',
			{ class: 'script-grid' },
			h(
				'div',
				{ class: 'script-main' },
				frame ?? h('p', { class: 'muted' }, 'This project has no scripts yet.'),
				entry
					? h(
							'div',
							{ class: 'editor-foot dim small' },
							h(
								'span',
								{ id: 'script-pos' },
								t('status.position', { line: entry.line, column: entry.column }),
							),
							h('span', {}, t('status.lua')),
							h(
								'span',
								{
									id: 'script-problems',
									class: entry.found.some((d) => d.severity === 'error') ? 'err' : '',
								},
								problemsText(entry.found),
							),
							h('span', { id: 'script-tab-hint' }, entry.tabLeaves ? t('editor.tabFocus') : ''),
						)
					: null,
				s.scriptPath ? consoleView(s) : null,
			),
			h(
				'aside',
				{ class: 'card side' },
				h('h3', {}, 'Hooks'),
				h(
					'ul',
					{ class: 'plain' },
					(r?.hooks ?? []).map((k) =>
						h(
							'li',
							{},
							h('span', { class: `dot ${k.present ? 'on' : ''}` }),
							h('code', {}, hookSignature(k.name)),
							h('span', { class: 'dim small' }, ` ${k.when}`),
						),
					),
				),
				h('h3', {}, 'Capabilities'),
				(r?.capabilities.length ?? 0) === 0
					? h('p', { class: 'dim small' }, 'None requested.')
					: h(
							'ul',
							{ class: 'plain' },
							r!.capabilities.map((c) =>
								h(
									'li',
									{},
									h('code', {}, c),
									h('div', { class: 'dim small' }, CAPABILITY_NOTES[c] ?? 'Unknown module'),
								),
							),
						),
			),
		),
	);
}
