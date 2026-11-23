// @vitest-environment happy-dom
// Tests the clickable preview: clicking selects the node behind the element, and a running preview's controls change its state.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, useState } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';
import { composeScreen, parseRef } from '../lib/stage';
import type { ComponentInfo, Layout, LayoutNode, Preview } from '../lib/types';
import { LayoutPreview, PreviewTree } from './PreviewTree';
import { RunStage } from './RunStage';
import { useRun } from './useRun';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root | undefined;
let host: HTMLElement | undefined;
afterEach(() => {
	act(() => root?.unmount());
	host?.remove();
});
function mount(ui: React.ReactNode): HTMLElement {
	host = document.createElement('div');
	document.body.append(host);
	root = createRoot(host);
	act(() => root!.render(ui));
	return host;
}

const stage: Layout = {
	version: 1,
	id: 'fizz/stage',
	root: {
		kind: 'backdrop',
		props: { title: 'Fizz', taskbar: 'Setup' },
		children: [
			{ kind: 'meter', props: { label: 'Free', source: 'free_space', region: 'status' } },
			{ kind: 'slot' },
		],
	},
};
const page: Layout = {
	version: 1,
	id: 'fizz/components',
	root: {
		kind: 'dialog',
		props: { title: 'Components' },
		children: [
			{
				kind: 'stack',
				children: [
					{ kind: 'text', props: { text: 'Pick', role: 'body' } },
					{
						kind: 'checklist',
						bind: 'components.selected',
						props: { label: 'Components', items: 'components' },
					},
					{ kind: 'text', props: { text: 'Space required: {install.size}', role: 'caption' } },
				],
			},
			{
				kind: 'stack',
				props: { region: 'footer' },
				children: [
					{ kind: 'button', props: { label: 'Back', action: 'back' } },
					{ kind: 'button', props: { label: 'Next', action: 'next' } },
				],
			},
		],
	},
};

describe('click to select', () => {
	it('tags every drawn node with the key of its layout and path', () => {
		const dom = mount(
			<LayoutPreview
				layout={page}
				stage={stage}
				renderer="win16"
				options={{ interact: { mode: 'edit' } }}
			/>,
		);
		const keys = [...dom.querySelectorAll('[data-node-path]')].map((e) =>
			e.getAttribute('data-node-path'),
		);
		// The stage's backdrop and meter, the page's dialog, its checklist and both buttons.
		expect(keys).toContain('s:');
		expect(keys).toContain('s:0');
		expect(keys).toContain('p:');
		expect(keys).toContain('p:0.1');
		expect(keys).toContain('p:1.0');
		expect(keys).toContain('p:1.1');
	});

	it('selects the node under the click and outlines the selection', () => {
		const picked: string[] = [];
		const dom = mount(
			<LayoutPreview
				layout={page}
				stage={stage}
				renderer="win16"
				options={{
					interact: {
						mode: 'edit',
						selected: 'p:0.1',
						onSelect: (key) => picked.push(key),
					},
				}}
			/>,
		);
		const box = dom.querySelector('[data-node-path="p:0.1"]')!;
		expect(box.getAttribute('data-selected')).toBe('true');
		act(() => (dom.querySelector('[data-node-path="s:0"]') as HTMLElement).click());
		act(() => (dom.querySelector('[data-node-path="p:1.1"]') as HTMLElement).click());
		expect(picked.map((k) => parseRef(k))).toEqual([
			{ scope: 'stage', path: [0] },
			{ scope: 'page', path: [1, 1] },
		]);
	});

	it('outlines the node under the pointer', () => {
		const dom = mount(
			<LayoutPreview
				layout={page}
				stage={stage}
				renderer="win16"
				options={{ interact: { mode: 'edit' } }}
			/>,
		);
		const target = dom.querySelector('[data-node-path="p:1.0"]') as HTMLElement;
		act(() => {
			target.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
		});
		expect(target.hasAttribute('data-hover')).toBe(true);
	});
});

const components: ComponentInfo[] = [
	{ id: 'game', name: 'Game', required: true, targets: [], files: 2 },
	{ id: 'music', name: 'Music', required: false, targets: [], files: 1 },
];
const tree: LayoutNode = composeScreen(stage.root, page.root, {
	id: 'components',
	kind: 'page',
}).root;
const preview: Preview = {
	target: 'win16-x86:win16',
	renderer: 'win16',
	theme: 'fizz',
	product: { id: 'p', name: 'P', version: '1', publisher: 'x' },
	vars: { 'install.path': 'C:\\FIZZ' },
	plan: { ops: [] },
	problems: [],
	steps: [
		{
			id: 'welcome',
			kind: 'page',
			layout: 'welcome',
			skipped: false,
			screenshot: null,
			layout_tree: tree,
			state: { 'components.selected': ['game', 'music'] },
		},
		{ id: 'sound', kind: 'page', layout: 'sound', skipped: true, screenshot: null },
		{
			id: 'components',
			kind: 'page',
			layout: 'components',
			skipped: false,
			screenshot: null,
			layout_tree: tree,
			state: { 'components.selected': ['game', 'music'] },
		},
	],
};

function Harness() {
	const [index, setIndex] = useState(0);
	const run = useRun(preview, components, index, setIndex);
	const step = preview.steps[index]!;
	return (
		<>
			<output data-testid="step">{step.id}</output>
			<output data-testid="summary">{run.vars['plan.summary']}</output>
			{step.layout_tree ? (
				<RunStage
					pv={preview}
					step={step}
					run={run}
					components={components}
					renderer="win16"
					theme="fizz"
				/>
			) : null}
		</>
	);
}
const text = (dom: HTMLElement, id: string) =>
	dom.querySelector(`[data-testid="${id}"]`)!.textContent;

describe('a running preview', () => {
	it('steps with Next and Back and passes over skipped steps', () => {
		const dom = mount(<Harness />);
		expect(text(dom, 'step')).toBe('welcome');
		const click = (action: string) =>
			act(() => (dom.querySelector(`button[data-action="${action}"]`) as HTMLElement).click());
		click('next');
		expect(text(dom, 'step')).toBe('components');
		click('next');
		expect(text(dom, 'step')).toBe('components');
		click('back');
		expect(text(dom, 'step')).toBe('welcome');
	});

	it('updates the space required and the plan when a component is toggled', () => {
		const dom = mount(<Harness />);
		expect(dom.textContent).toContain('Space required: 48 MB');
		expect(text(dom, 'summary')).toBe('2 components · 3 files · 48 MB');
		const boxes = dom.querySelectorAll('[role="checkbox"]');
		// The first is required and stays on; the second is optional.
		act(() => (boxes[0] as HTMLElement).click());
		expect(text(dom, 'summary')).toBe('2 components · 3 files · 48 MB');
		act(() => (boxes[1] as HTMLElement).click());
		expect(dom.textContent).toContain('Space required: 32 MB');
		expect(text(dom, 'summary')).toBe('1 components · 2 files · 32 MB');
		expect(dom.querySelectorAll('[role="checkbox"]')[1]!.getAttribute('aria-checked')).toBe(
			'false',
		);
	});

	it('draws the free-space meter from the chosen folder', () => {
		const dom = mount(
			<PreviewTree
				root={tree}
				options={{
					renderer: 'win16',
					theme: 'fizz',
					live: { freeSpace: 0.01 },
					interact: { mode: 'run' },
				}}
			/>,
		);
		const meter = dom.querySelector('[data-source="free_space"]')!;
		expect(meter.getAttribute('aria-valuenow')).toBe('1');
	});
});
