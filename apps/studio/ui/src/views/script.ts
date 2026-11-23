// The Script screen: a Lua editor beside the hook and capability lists, with a test run.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Ctx } from '../app';
import { h } from '../lib/dom';
import { lineCount } from '../lib/scriptText';
import type { Settings } from '../lib/settings';

const CAPABILITY_NOTES: Record<string, string> = {
	'li.env': 'Read the environment and system facts',
	'li.fs': 'Read anywhere; write only through the plan',
	'li.ui': 'Notes and dialogs',
	'li.plan': 'Add plan operations',
	'li.net': 'Network access; needs the `net` capability',
	'li.retro.sound': 'Sound probing; retro targets only',
	'li.retro.video': 'Video modes; retro targets only',
};

/** Numbers the lines in the gutter, giving a wrapped line the height of the rows it takes so the numbers stay level with it. */
export function refreshGutter(editor: HTMLElement): void {
	const area = editor.querySelector<HTMLTextAreaElement>('textarea');
	const gutter = editor.querySelector<HTMLElement>('.editor-gutter');
	if (!area || !gutter || editor.dataset.numbers === 'off') return;
	const count = lineCount(area.value);
	const heights: number[] = [];
	if (editor.dataset.wrap === 'on' && area.clientWidth > 0) {
		const style = getComputedStyle(area);
		const mirror = h('div', { 'aria-hidden': 'true' });
		Object.assign(mirror.style, {
			position: 'absolute', visibility: 'hidden', pointerEvents: 'none', left: '0', top: '0',
			width: `${area.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight)}px`,
			font: style.font, lineHeight: style.lineHeight, letterSpacing: style.letterSpacing, tabSize: style.tabSize,
			whiteSpace: 'pre-wrap', overflowWrap: 'anywhere',
		});
		for (const line of area.value.split('\n').slice(0, count)) mirror.append(h('div', {}, line === '' ? ' ' : line));
		editor.append(mirror);
		for (const row of Array.from(mirror.children)) heights.push((row as HTMLElement).offsetHeight);
		mirror.remove();
	}
	gutter.replaceChildren(...Array.from({ length: count }, (_, i) => h('div', heights[i] ? { style: `height:${heights[i]}px` } : {}, String(i + 1))));
	gutter.scrollTop = area.scrollTop;
}

/** Applies the Editor settings to the editor on screen, if there is one, without touching what is typed in it. */
export function applyEditorSettings(root: ParentNode, settings: Pick<Settings, 'editorFontSize' | 'tabWidth' | 'wrapLines' | 'lineNumbers'>): void {
	const editor = root.querySelector<HTMLElement>('.editor');
	if (!editor) return;
	editor.style.setProperty('--editor-font-size', `${settings.editorFontSize}px`);
	editor.style.setProperty('--editor-tab', String(settings.tabWidth));
	editor.dataset.wrap = settings.wrapLines ? 'on' : 'off';
	editor.dataset.numbers = settings.lineNumbers ? 'on' : 'off';
	refreshGutter(editor);
}

export function scriptView(ctx: Ctx): HTMLElement {
	const s = ctx.state;
	const sum = s.summary!;
	const editor = h('textarea', {
		id: 'script-editor',
		class: 'code',
		spellcheck: false,
		'aria-label': `Lua source of ${s.scriptPath}`,
		value: s.scriptSource,
		on: {
			input: (e) => {
				const v = (e.target as HTMLTextAreaElement).value;
				s.scriptDirty = v !== s.scriptSource;
				const wrapper = (e.target as HTMLElement).closest<HTMLElement>('.editor');
				if (wrapper) refreshGutter(wrapper);
				const flag = document.getElementById('script-dirty');
				if (flag) flag.textContent = s.scriptDirty ? 'Unsaved changes' : 'Saved';
				ctx.syncChrome();
			},
			keydown: (e) => {
				const k = e as KeyboardEvent;
				const area = e.target as HTMLTextAreaElement;
				if ((k.ctrlKey || k.metaKey) && k.key.toLowerCase() === 's') {
					k.preventDefault();
					void ctx.run(() => ctx.saveScript(area.value));
				} else if (k.key === 'Tab') {
					k.preventDefault();
					const { selectionStart: a, selectionEnd: b } = area;
					area.setRangeText('\t', a, b, 'end');
					area.dispatchEvent(new Event('input'));
				}
			},
		},
	});
	editor.addEventListener('scroll', () => {
		const gutter = editor.parentElement?.querySelector<HTMLElement>('.editor-gutter');
		if (gutter) gutter.scrollTop = editor.scrollTop;
	});
	const frame = h('div', { class: 'editor' }, h('div', { class: 'editor-gutter', 'aria-hidden': 'true' }), editor);
	if (typeof ResizeObserver !== 'undefined') new ResizeObserver(() => refreshGutter(frame)).observe(editor);
	const value = () => (document.getElementById('script-editor') as HTMLTextAreaElement | null)?.value ?? s.scriptSource;
	const r = s.report;
	return h(
		'div',
		{ class: 'screen' },
		h('div', { class: 'screen-head' }, h('div', {}, h('h1', {}, 'Script'), h('p', { class: 'muted' }, 'Lua 5.4. A script names the capabilities it needs; everything else is refused.'))),
		h(
			'div',
			{ class: 'toolbar' },
			h('select', { 'aria-label': 'Script', on: { change: (e) => void ctx.run(() => ctx.loadScript((e.target as HTMLSelectElement).value)) } }, sum.scripts.length === 0 ? h('option', {}, '(no scripts)') : sum.scripts.map((p) => h('option', { value: p, selected: p === s.scriptPath }, p))),
			h('button', { class: 'btn', on: { click: () => { const name = window.prompt('New script name (letters, digits, - and _)', 'my-script'); if (name && /^[A-Za-z0-9_-]+$/.test(name)) void ctx.run(async () => { const path = `scripts/${name}.lua`; await ctx.api.writeScript(path, `-- ${name}\n\nfunction detect(page)\nend\n`, `Create ${path}`); await ctx.loadScript(path); }); } } }, 'New script'),
			h('button', { class: 'btn primary', disabled: !s.scriptPath, on: { click: () => void ctx.run(() => ctx.saveScript(value())) } }, 'Save'),
			h('button', { class: 'btn', disabled: !s.scriptPath, on: { click: () => void ctx.run(() => ctx.testScript(value())) } }, 'Test run'),
			h('span', { id: 'script-dirty', class: 'dim small' }, s.scriptDirty ? 'Unsaved changes' : 'Saved'),
		),
		h(
			'div',
			{ class: 'script-grid' },
			s.scriptPath ? frame : h('p', { class: 'muted' }, 'This project has no scripts yet.'),
			h(
				'aside',
				{ class: 'card side' },
				h('h3', {}, 'Hooks'),
				h('ul', { class: 'plain' }, (r?.hooks ?? []).map((k) => h('li', {}, h('span', { class: `dot ${k.present ? 'on' : ''}` }), h('code', {}, `${k.name}(page)`), h('span', { class: 'dim small' }, ` ${k.when}`)))),
				h('h3', {}, 'Capabilities'),
				(r?.capabilities.length ?? 0) === 0
					? h('p', { class: 'dim small' }, 'None requested.')
					: h('ul', { class: 'plain' }, r!.capabilities.map((c) => h('li', {}, h('code', {}, c), h('div', { class: 'dim small' }, CAPABILITY_NOTES[c] ?? 'Unknown module')))),
				h('h3', {}, 'Test result'),
				r
					? h('div', {}, h('p', { class: r.ok ? 'ok' : 'err' }, r.ok ? 'Passed' : 'Failed'), r.diagnostics.length ? h('ul', { class: 'problems' }, r.diagnostics.map((d) => h('li', { class: d.severity }, d.message))) : null, r.output.length ? h('pre', { class: 'output' }, r.output.join('\n')) : null)
					: h('p', { class: 'dim small' }, 'Not run yet.'),
			),
		),
	);
}
