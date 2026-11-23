// @vitest-environment happy-dom
// Tests the full-screen body: the View switch for terminal targets, the real terminal in it and how its outcomes step the flow.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, useState } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';
import type { TuiApi } from '../lib/api';
import type { ComponentInfo, Preview, TargetInfo, TuiOutcome } from '../lib/types';
import { FullScreenBody } from './FullScreenBody';
import { previewWindowQuery, parsePreviewQuery } from './fullScreen';
import type { TerminalView } from './TerminalPreview';
import { useRun } from './useRun';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root | undefined;
let host: HTMLElement | undefined;
afterEach(() => {
	act(() => root?.unmount());
	host?.remove();
});

const settle = () => act(async () => new Promise((r) => setTimeout(r, 0)));

const tree = { kind: 'dialog', props: { title: 'Page' }, children: [] };
const step = (id: string, extra: Record<string, unknown> = {}) => ({
	id,
	kind: 'page',
	layout: id,
	skipped: false,
	screenshot: null,
	layout_tree: tree,
	state: { page: id },
	...extra,
});
const pv = (renderer: string): Preview =>
	({
		target: `t:${renderer}`,
		renderer,
		theme: 'fizz',
		product: { id: 'p', name: 'P', version: '1', publisher: 'x' },
		vars: {},
		plan: { ops: [] },
		problems: [],
		steps: [step('welcome'), step('sound', { skipped: true }), step('ready'), step('done')],
	}) as unknown as Preview;
const targets = (renderer: string): TargetInfo[] =>
	[{ id: 't', full_id: `t:${renderer}`, renderer, theme: 'fizz' }] as unknown as TargetInfo[];
const components: ComponentInfo[] = [];

interface Calls {
	opened: { page: string; state: unknown; size: unknown }[];
	outcome: TuiOutcome;
}
function fakeApi(calls: Calls): TuiApi {
	let n = 0;
	const grid = {
		width: 2,
		height: 1,
		cells: [
			{ ch: 'a', fg: 'white', bg: 'blue' },
			{ ch: 'b', fg: 'white', bg: 'blue' },
		],
	};
	return {
		tuiSessionOpen: async (_t, page, state, size) => {
			calls.opened.push({ page, state, size });
			return { session: `s${++n}`, frame: { grid, focus: 'Next, button', state: {} } };
		},
		tuiSessionKey: async () => ({
			frame: { grid, focus: 'Next, button', state: {} },
			outcome: calls.outcome,
		}),
		tuiSessionResize: async () => ({ grid, focus: '', state: {} }),
		tuiSessionClose: async () => undefined,
	};
}

function Harness({
	renderer,
	api,
	first = 'rich',
	start = 0,
}: {
	renderer: string;
	api: TuiApi | null;
	first?: TerminalView;
	start?: number;
}) {
	const [preview] = useState(() => pv(renderer));
	const [index, setIndex] = useState(start);
	const [view, setView] = useState<TerminalView>(first);
	const run = useRun(preview, components, index, setIndex);
	return (
		<>
			<output data-testid="step">{preview.steps[index]!.id}</output>
			<FullScreenBody
				pv={preview}
				run={run}
				components={components}
				targets={targets(renderer)}
				target={`t:${renderer}`}
				index={index}
				setIndex={setIndex}
				onTarget={() => undefined}
				fullscreen={false}
				onToggleFullscreen={() => undefined}
				view={view}
				onView={setView}
				scheme="system"
				onScheme={() => undefined}
				api={api}
			/>
		</>
	);
}

function mount(ui: React.ReactNode): HTMLElement {
	host = document.createElement('div');
	document.body.append(host);
	root = createRoot(host);
	act(() => root!.render(ui));
	return host;
}

const viewSwitch = (dom: HTMLElement) =>
	[...dom.querySelectorAll('select')].find((s) =>
		[...s.options].some((o) => o.value === 'terminal'),
	);
const key = (dom: HTMLElement, k: string) =>
	act(async () => {
		dom
			.querySelector('[role="group"][aria-label^="Terminal screen"]')!
			.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
	});

describe('the full-screen View switch', () => {
	it('is offered for terminal renderers only', () => {
		const calls: Calls = { opened: [], outcome: { kind: 'none' } };
		expect(viewSwitch(mount(<Harness renderer="tui" api={fakeApi(calls)} />))).toBeDefined();
		act(() => root!.unmount());
		host!.remove();
		expect(viewSwitch(mount(<Harness renderer="tui-retro" api={fakeApi(calls)} />))).toBeDefined();
		act(() => root!.unmount());
		host!.remove();
		expect(viewSwitch(mount(<Harness renderer="web" api={fakeApi(calls)} />))).toBeUndefined();
		act(() => root!.unmount());
		host!.remove();
		expect(viewSwitch(mount(<Harness renderer="tui" api={null} />))).toBeUndefined();
	});

	it('opens in the real terminal when the window was opened with that view', async () => {
		const calls: Calls = { opened: [], outcome: { kind: 'none' } };
		const view = parsePreviewQuery(`?${previewWindowQuery('t:tui', 2, 'terminal')}`)!.view;
		const dom = mount(<Harness renderer="tui" api={fakeApi(calls)} first={view} start={2} />);
		await settle();
		expect(calls.opened).toHaveLength(1);
		expect(calls.opened[0]).toMatchObject({
			page: 'ready',
			state: { page: 'ready' },
			size: { cols: 80, rows: 25 },
		});
		expect(dom.querySelector('canvas')).not.toBeNull();
		expect(dom.textContent).toContain('Esc goes to the installer');
	});

	it('switches between the rich screen and the terminal', async () => {
		const calls: Calls = { opened: [], outcome: { kind: 'none' } };
		const dom = mount(<Harness renderer="tui" api={fakeApi(calls)} />);
		expect(dom.querySelector('canvas')).toBeNull();
		const select = viewSwitch(dom)!;
		select.value = 'terminal';
		await act(async () => select.dispatchEvent(new Event('change', { bubbles: true })));
		await settle();
		expect(dom.querySelector('canvas')).not.toBeNull();
		select.value = 'rich';
		await act(async () => select.dispatchEvent(new Event('change', { bubbles: true })));
		expect(dom.querySelector('canvas')).toBeNull();
	});
});

describe('the terminal in the full-screen window', () => {
	it('steps the flow with what a key did, past skipped steps, and restarts on Esc', async () => {
		const calls: Calls = { opened: [], outcome: { kind: 'activate', action: 'next' } };
		const dom = mount(<Harness renderer="tui" api={fakeApi(calls)} first="terminal" />);
		await settle();
		expect(dom.querySelector('[data-testid="step"]')!.textContent).toBe('welcome');
		await key(dom, 'Enter');
		await settle();
		// The skipped step is passed over, and a session opens for the next page.
		expect(dom.querySelector('[data-testid="step"]')!.textContent).toBe('ready');
		expect(calls.opened.map((o) => o.page)).toEqual(['welcome', 'ready']);
		calls.outcome = { kind: 'activate', action: 'back' };
		await key(dom, 'b');
		await settle();
		expect(dom.querySelector('[data-testid="step"]')!.textContent).toBe('welcome');
		calls.outcome = { kind: 'activate', action: 'next' };
		await key(dom, 'Enter');
		await settle();
		calls.outcome = { kind: 'cancel' };
		await key(dom, 'Escape');
		await settle();
		expect(dom.querySelector('[data-testid="step"]')!.textContent).toBe('welcome');
	});

	it('keeps Esc for the installer: the window never sees a key the terminal took', async () => {
		const calls: Calls = { opened: [], outcome: { kind: 'none' } };
		const dom = mount(<Harness renderer="tui" api={fakeApi(calls)} first="terminal" />);
		await settle();
		const e = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
		await act(async () =>
			dom.querySelector('[role="group"][aria-label^="Terminal screen"]')!.dispatchEvent(e),
		);
		expect(e.defaultPrevented).toBe(true);
	});
});
