// @vitest-environment happy-dom
// Tests the real terminal view: the session it opens, the keys it forwards and what it shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { TuiApi } from '../lib/api';
import { StudioError } from '../lib/api';
import type { CellGrid, TuiFrame, TuiOutcome } from '../lib/types';
import {
	gridText,
	stepFor,
	TerminalPreview,
	TerminalViewSwitch,
	tuiKeyFrom,
} from './TerminalPreview';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const grid = (rows: string[]): CellGrid => ({
	width: rows[0]!.length,
	height: rows.length,
	cells: rows.flatMap((r) => [...r].map((ch) => ({ ch, fg: 'white', bg: 'blue' }))),
});

const frame = (rows: string[], focus = 'Next, button'): TuiFrame => ({
	grid: grid(rows),
	focus,
	state: {},
});

interface Fake {
	api: TuiApi;
	opened: unknown[][];
	keys: string[];
	closed: string[];
}

function fake(onKey: (key: string) => TuiOutcome = () => ({ kind: 'none' })): Fake {
	const f: Fake = { opened: [], keys: [], closed: [], api: undefined as unknown as TuiApi };
	let n = 0;
	f.api = {
		tuiSessionOpen: async (...args) => {
			f.opened.push(args);
			return { session: `s${++n}`, frame: frame([`┌─┐`, `│${args[1]![0]}│`, `└─┘`]) };
		},
		tuiSessionKey: async (_session, key) => {
			f.keys.push(`${key.shift ? 'Shift+' : ''}${key.key}`);
			return {
				frame: frame(['abc', 'def', `[${key.key}]`], 'Back, button'),
				outcome: onKey(key.key),
			};
		},
		tuiSessionResize: async () => frame(['x']),
		tuiSessionClose: async (s) => {
			f.closed.push(s);
		},
	};
	return f;
}

let root: Root | null = null;
let host: HTMLElement | null = null;

async function mount(node: React.ReactNode): Promise<HTMLElement> {
	host = document.createElement('div');
	document.body.append(host);
	root = createRoot(host);
	await act(async () => root!.render(node));
	return host;
}

const settle = () => act(async () => new Promise((r) => setTimeout(r, 0)));

afterEach(async () => {
	await act(async () => root?.unmount());
	host?.remove();
	root = null;
	host = null;
});

describe('key mapping', () => {
	const ev = (key: string, extra: Partial<Parameters<typeof tuiKeyFrom>[0]> = {}) => ({
		key,
		shiftKey: false,
		ctrlKey: false,
		metaKey: false,
		altKey: false,
		...extra,
	});

	it('forwards navigation keys, letters and space', () => {
		expect(tuiKeyFrom(ev('Tab'))).toEqual({ key: 'Tab', shift: false, ctrl: false, alt: false });
		expect(tuiKeyFrom(ev('Tab', { shiftKey: true }))?.shift).toBe(true);
		expect(tuiKeyFrom(ev('ArrowDown'))?.key).toBe('ArrowDown');
		expect(tuiKeyFrom(ev(' '))?.key).toBe(' ');
		expect(tuiKeyFrom(ev('Enter'))?.key).toBe('Enter');
		expect(tuiKeyFrom(ev('Escape'))?.key).toBe('Escape');
		expect(tuiKeyFrom(ev('y'))?.key).toBe('y');
		expect(tuiKeyFrom(ev('Y', { shiftKey: true }))).toMatchObject({ key: 'Y', shift: false });
	});

	it('leaves modifier keys, shortcuts and function keys to the app', () => {
		expect(tuiKeyFrom(ev('Shift'))).toBeNull();
		expect(tuiKeyFrom(ev('r', { ctrlKey: true }))).toBeNull();
		expect(tuiKeyFrom(ev('r', { metaKey: true }))).toBeNull();
		expect(tuiKeyFrom(ev('F5'))).toBeNull();
	});
});

describe('steps', () => {
	it('turns what a key did into a step through the flow', () => {
		expect(stepFor({ kind: 'activate', action: 'next' })).toBe('next');
		expect(stepFor({ kind: 'activate', action: 'install' })).toBe('next');
		expect(stepFor({ kind: 'activate', action: 'back' })).toBe('back');
		expect(stepFor({ kind: 'activate', action: 'cancel' })).toBe('cancel');
		expect(stepFor({ kind: 'cancel' })).toBe('cancel');
		expect(stepFor({ kind: 'none' })).toBeNull();
		expect(stepFor({ kind: 'changed', key: 'a' })).toBeNull();
	});
});

describe('TerminalPreview', () => {
	it('opens a session for the page and offers the screen as text', async () => {
		const f = fake();
		const el = await mount(
			<TerminalPreview
				api={f.api}
				target="dos-x86:tui-retro"
				page="welcome"
				label="Terminal screen of welcome"
				state={{ a: 1 }}
			/>,
		);
		await settle();
		expect(f.opened).toEqual([['dos-x86:tui-retro', 'welcome', { a: 1 }, { cols: 80, rows: 24 }]]);
		const term = el.querySelector('[aria-label="Terminal screen of welcome"]') as HTMLElement;
		expect(term.tabIndex).toBe(0);
		expect(term.textContent).toContain('┌─┐');
		expect(term.textContent).toContain('Next, button');
		expect(el.querySelector('canvas')).not.toBeNull();
	});

	it('sends keys in order and reports what they did', async () => {
		const outcomes: string[] = [];
		const f = fake((k) =>
			k === 'Enter' ? { kind: 'activate', action: 'next' } : { kind: 'none' },
		);
		const el = await mount(
			<TerminalPreview
				api={f.api}
				target="t:tui"
				page="p"
				label="T"
				onOutcome={(o) => outcomes.push(o.kind)}
			/>,
		);
		await settle();
		const term = el.querySelector('[aria-label="T"]')!;
		await act(async () => {
			term.dispatchEvent(
				new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true }),
			);
			term.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true }));
			term.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
			term.dispatchEvent(new KeyboardEvent('keydown', { key: 'r', ctrlKey: true, bubbles: true }));
		});
		await settle();
		expect(f.keys).toEqual(['Shift+Tab', ' ', 'Enter']);
		expect(outcomes).toEqual(['none', 'none', 'activate']);
		expect(term.textContent).toContain('[En');
		expect(term.textContent).toContain('Back, button');
	});

	it('does not let the page handle a key the terminal took', async () => {
		const f = fake();
		const el = await mount(<TerminalPreview api={f.api} target="t:tui" page="p" label="T" />);
		await settle();
		const e = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
		await act(async () => el.querySelector('[aria-label="T"]')!.dispatchEvent(e));
		expect(e.defaultPrevented).toBe(true);
		const f6 = new KeyboardEvent('keydown', { key: 'F6', bubbles: true, cancelable: true });
		await act(async () => el.querySelector('[aria-label="T"]')!.dispatchEvent(f6));
		expect(f6.defaultPrevented).toBe(false);
	});

	it('closes its session when the page changes and when it unmounts', async () => {
		const f = fake();
		const el = await mount(<TerminalPreview api={f.api} target="t:tui" page="a" label="T" />);
		await settle();
		await act(async () =>
			root!.render(<TerminalPreview api={f.api} target="t:tui" page="b" label="T" />),
		);
		await settle();
		expect(f.opened.map((o) => o[1])).toEqual(['a', 'b']);
		expect(f.closed).toEqual(['s1']);
		expect(el.textContent).toContain('│b│');
		await act(async () => root!.unmount());
		root = null;
		expect(f.closed).toEqual(['s1', 's2']);
	});

	it('closes a session that opened after the view went away', async () => {
		const f = fake();
		let release: () => void = () => undefined;
		const gate = new Promise<void>((r) => (release = r));
		const slow: TuiApi = {
			...f.api,
			tuiSessionOpen: async (...a) => {
				await gate;
				return f.api.tuiSessionOpen(...a);
			},
		};
		await mount(<TerminalPreview api={slow} target="t:tui" page="a" label="T" />);
		await act(async () => root!.unmount());
		root = null;
		release();
		await settle();
		expect(f.closed).toEqual(['s1']);
	});

	it('shows why a session could not open', async () => {
		const api: TuiApi = {
			...fake().api,
			tuiSessionOpen: async () => {
				throw new StudioError('backend', 'not a terminal target');
			},
		};
		const el = await mount(<TerminalPreview api={api} target="t:canvas" page="a" label="T" />);
		await settle();
		expect(el.querySelector('[role="alert"]')?.textContent).toBe('not a terminal target');
	});

	it('writes a grid as trimmed rows', () => {
		expect(gridText(grid(['ab ', '   ']))).toBe('ab\n');
	});
});

describe('TerminalViewSwitch', () => {
	it('offers the rich screen and the real terminal', async () => {
		const onChange = vi.fn();
		const el = await mount(<TerminalViewSwitch value="rich" onChange={onChange} />);
		const select = el.querySelector('select')!;
		expect([...select.options].map((o) => o.textContent)).toEqual(['Rich', 'Terminal (real)']);
		select.value = 'terminal';
		await act(async () => select.dispatchEvent(new Event('change', { bubbles: true })));
		expect(onChange).toHaveBeenCalledWith('terminal');
	});
});
