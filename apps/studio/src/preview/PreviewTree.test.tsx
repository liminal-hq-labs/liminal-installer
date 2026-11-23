// Tests the preview renderer: which look each target gets, interpolation, art, meters and the controls.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import type { Layout, LayoutNode } from '../lib/types';
import { fill, lookFor, type RenderOptions } from './model';
import { ArtSvg, LayoutPreview, PreviewTree } from './PreviewTree';

const renderTreeHtml = (root: LayoutNode, options: RenderOptions): string =>
	renderToStaticMarkup(<PreviewTree root={root} options={options} />);
const renderLayoutHtml = (layout: Layout, renderer: string): string =>
	renderToStaticMarkup(<LayoutPreview layout={layout} renderer={renderer} />);
const artSvg = (art: unknown, label?: string): string =>
	renderToStaticMarkup(<ArtSvg art={art} label={label} />);

const art = {
	width: 6,
	height: 10,
	scale: 4,
	rects: [
		[0, 0, 6, 1, '#aa0000'],
		[1, 4, 2, 2, '#ffffff'],
	],
};

const classic: LayoutNode = {
	kind: 'backdrop',
	props: {
		style: 'classic',
		title: 'Lieutenant Fizz',
		subtitle: 'Episode One',
		credit: 'Setup powered by Liminal Installer',
		taskbar: '{product.name} Setup',
	},
	children: [
		{
			kind: 'dialog',
			props: { title: 'Choose Destination Location', style: 'win32-classic' },
			children: [
				{
					kind: 'stack',
					props: { region: 'side' },
					children: [{ kind: 'image', props: { label: 'Fizz', art, panel: '#008080' } }],
				},
				{
					kind: 'stack',
					children: [
						{
							kind: 'text',
							props: { text: 'Version {product.version} on {target.id}', role: 'body' },
						},
						{
							kind: 'path',
							bind: 'install.path',
							props: { label: 'Destination Folder', browse: 'Browse…', hotkey: 'r' },
						},
						{
							kind: 'checklist',
							bind: 'components.selected',
							props: { label: 'Components', items: 'components', sunken: true },
						},
						{
							kind: 'billboard',
							props: {
								label: 'Highlights',
								captions: [
									{ title: 'Meet Ben', text: 'Ten years old.' },
									{ title: 'Two', text: 'Second.' },
								],
							},
						},
					],
				},
				{
					kind: 'stack',
					props: { region: 'footer', direction: 'row' },
					children: [
						{ kind: 'button', props: { label: 'Back', action: 'back', hotkey: 'B' } },
						{
							kind: 'button',
							props: { label: 'Next', action: 'next', default: true, hotkey: 'N' },
						},
						{ kind: 'button', props: { label: 'Cancel', action: 'cancel' } },
					],
				},
			],
		},
		{ kind: 'meter', bind: 'm.file', props: { label: 'Current file', source: 'file' } },
		{ kind: 'meter', bind: 'm.total', props: { label: 'Overall', source: 'total' } },
		{
			kind: 'meter',
			bind: 'm.free',
			props: { label: 'Free space', source: 'free_space', low_below: 0.05 },
		},
	],
};

const opts = {
	renderer: 'win32-classic',
	theme: 'fizz-win95',
	vars: { 'product.version': '2.3.4', 'product.name': 'Fizz', 'target.id': 'windows-x86_64' },
	state: {
		'components.selected': ['game'],
		'install.path': 'C:\\FIZZ',
		'm.file': 0.5,
		'm.total': 0.25,
		'm.free': 0.02,
	},
	components: [
		{ id: 'game', name: 'Game program', required: true },
		{ id: 'levels', name: 'Levels' },
	],
};

describe('lookFor', () => {
	it('gives text-mode, classic Windows and canvas renderers their own looks', () => {
		expect(lookFor('tui-retro')).toBe('dos');
		expect(lookFor('tui')).toBe('dos');
		expect(lookFor('win32-classic')).toBe('win95');
		expect(lookFor('win16')).toBe('win95');
		expect(lookFor('canvas', 'afterglow')).toBe('afterglow');
		expect(lookFor('web')).toBe('afterglow');
		expect(lookFor('canvas', 'neutral')).toBe('neutral');
		expect(lookFor('canvas', null, 'neutral')).toBe('neutral');
	});

	it('gives the Fizz Windows 11 theme, or a root styled win11, the Fluent look on web and canvas', () => {
		expect(lookFor('web', 'fizz-modern')).toBe('win11');
		expect(lookFor('canvas', null, 'win11')).toBe('win11');
		expect(lookFor('tui', 'fizz-modern')).toBe('dos');
		expect(lookFor('win32-classic', 'fizz-modern')).toBe('win95');
	});
});

describe('fill', () => {
	it('fills known placeholders only', () => {
		expect(fill('v{product.version} {unknown.thing}')).toBe('v1.0.0 {unknown.thing}');
		expect(fill('{target.id}', { 'target.id': 'dos-x86' })).toBe('dos-x86');
	});
});

describe('artSvg', () => {
	it('draws pixel rects and shapes as crisp inline SVG and rejects odd colours', () => {
		const svg = artSvg(art, 'Lt. Fizz');
		expect(svg).toContain('viewBox="0 0 6 10"');
		expect(svg).toContain('width="24" height="40"');
		expect(svg).toContain('shape-rendering="crispEdges"');
		expect(svg).toContain('<rect x="1" y="4" width="2" height="2" fill="#ffffff"></rect>');
		expect(svg).toContain('aria-label="Lt. Fizz"');
		expect(artSvg({ width: 2, height: 2, rects: [[0, 0, 1, 1, 'red" onload="x']] })).not.toContain(
			'onload',
		);
		expect(artSvg({ viewBox: [4, 4], shapes: [{ path: 'M0 0h4', stroke: '#fff' }] })).toContain(
			'<path d="M0 0h4"',
		);
		expect(artSvg(undefined)).toBe('');
	});
});

describe('the Windows 95 classic look', () => {
	const html = renderTreeHtml(classic, opts);
	it('draws the wordmark, bevelled dialog, side panel, taskbar and buttons', () => {
		expect(html).toContain('pvWin95');
		expect(html).toContain('data-w="1024" data-h="768"');
		expect(html).toContain('>Lieutenant Fizz</div>');
		expect(html).toContain('Setup powered by Liminal Installer');
		expect(html).toContain('pvWTitle');
		expect(html).toContain('Choose Destination Location');
		expect(html).toContain('pvWSide');
		expect(html).toContain('background:#008080');
		expect(html).toContain('Fizz Setup');
		expect(html).toContain('<u>B</u>ack');
		expect(html).toContain('<u>N</u>ext &gt;');
		expect(html).toContain('pvDefault');
		expect(html).toContain('<u>r</u>');
	});
	it('interpolates placeholders from the run and never leaves one raw', () => {
		expect(html).toContain('Version 2.3.4 on windows-x86_64');
		expect(html).not.toContain('{product.');
		expect(html).not.toContain('{target.');
	});
	it('expands the component list and checks what the state selects', () => {
		expect(html).toContain('Game program');
		expect(html).toContain('Levels');
		expect(html.match(/pvBox on/g)).toHaveLength(1);
		expect(html).toContain('pvSunken');
	});
	it('draws the three status meters as lit-block columns with a Low warning', () => {
		expect(html.match(/class="pvVmeter"/g)).toHaveLength(3);
		expect(html).toContain('aria-valuenow="50"');
		expect(html).toContain('aria-valuenow="25"');
		expect(html).toContain('aria-valuenow="2"');
		expect(html).toContain('fill="#ff4040"');
		expect(html).toContain('>Low<');
	});
	it('shows the first billboard caption', () => {
		expect(html).toContain('Meet Ben');
		expect(html).not.toContain('Second.');
	});
});

describe('the DOS text-mode look', () => {
	const dosTree: LayoutNode = {
		kind: 'backdrop',
		props: { style: 'text-mode', title: '{product.name}  Setup', credit: 'Liminal Installer' },
		children: [
			{
				kind: 'stack',
				props: { region: 'side' },
				children: [
					{ kind: 'text', props: { text: 'Lt. Fizz', role: 'title' } },
					{
						kind: 'progress',
						props: { label: 'Setup steps', role: 'steps', value: 5, max: 8, text: 'Step 5 of 8' },
					},
				],
			},
			{
				kind: 'dialog',
				props: { title: 'Sound Card', border: 'double' },
				children: [
					{
						kind: 'stack',
						children: [
							{
								kind: 'radio',
								bind: 'sound.card',
								props: {
									label: 'Sound card',
									options: [
										{ id: 'none', label: 'No sound' },
										{ id: 'sbpro', label: 'Sound Blaster / Pro / 16' },
									],
								},
							},
							{
								kind: 'stack',
								props: { direction: 'row', group: 'fields' },
								children: [
									{ kind: 'field', bind: 'sound.port', props: { label: 'Port', default: '220' } },
								],
							},
							{ kind: 'meter', bind: 'm', props: { label: 'Total', source: 'total' } },
						],
					},
					{
						kind: 'stack',
						props: { region: 'footer', direction: 'row' },
						children: [{ kind: 'button', props: { label: 'OK', default: true, hotkey: 'O' } }],
					},
				],
			},
			{
				kind: 'stack',
				props: { region: 'hints', direction: 'row' },
				children: [{ kind: 'text', props: { key: 'Enter', text: 'Accept' } }],
			},
		],
	};
	const html = renderTreeHtml(dosTree, {
		renderer: 'tui-retro',
		theme: 'fizz-dos',
		vars: { 'product.name': 'Lt. Fizz' },
		state: { 'sound.card': 'sbpro', m: 0.5 },
	});
	it('draws the title bar, ░ backdrop, double-border dialog with the title in the border and key hints', () => {
		expect(html).toContain('pvDos');
		expect(html).toContain('data-w="960" data-h="600"');
		expect(html).toContain('Lt. Fizz  Setup');
		expect(html).toContain('░░░░');
		expect(html).toContain('[ Sound Card ]');
		expect(html).toContain('pvDosHints');
		expect(html).toContain('<span class="pvKeycap">Enter</span> Accept');
	});
	it('draws step progress, text-mode controls and a block meter', () => {
		expect(html).toContain('Step 5 of 8');
		expect(html.match(/<i class="pvOn">/g)).toHaveLength(5);
		expect(html).toContain('(•)');
		expect(html).toContain('( )');
		expect(html).toContain('&lt; <span class="pvHot">O</span>K &gt;');
		expect(html).toContain('█'.repeat(12));
		expect(html).toContain('pvGFields');
		expect(html).toContain('>220</span>');
	});
	it('marks the chosen radio row', () => {
		expect(html).toMatch(
			/pvChoice pvSel"[^>]*><span class="pvGlyph">\(•\)<\/span><span class="pvChoiceBody"><span class="pvChoiceLabel">Sound Blaster/,
		);
	});
});

describe('Afterglow and Neutral', () => {
	const modern: LayoutNode = {
		kind: 'window',
		props: {
			title: 'Planboard Setup',
			style: 'afterglow',
			icon: { viewBox: [20, 20], shapes: [{ rect: [1, 1, 5, 5], fill: '#ffaa40' }] },
		},
		children: [
			{
				kind: 'stack',
				props: { region: 'side' },
				children: [
					{ kind: 'text', props: { text: 'Welcome', role: 'step-item', state: 'done' } },
					{ kind: 'text', props: { text: 'Components', role: 'step-item', state: 'current' } },
					{ kind: 'text', props: { text: 'Done', role: 'step-item', state: 'todo' } },
				],
			},
			{
				kind: 'stack',
				children: [
					{
						kind: 'checklist',
						bind: 'c',
						props: {
							label: 'Components',
							cards: true,
							items: [
								{ id: 'a', label: 'App', description: 'The app.', required: true, checked: true },
								{
									id: 'b',
									label: 'CLI',
									description: 'A tool.',
									badges: [{ text: 'Adds to PATH' }],
								},
							],
						},
					},
					{
						kind: 'meter',
						bind: 'm',
						props: { label: 'Overall', source: 'total', caption: '{progress.percent}%' },
					},
				],
			},
			{
				kind: 'stack',
				props: { region: 'footer', direction: 'row' },
				children: [
					{ kind: 'text', props: { text: 'No administrator needed', role: 'note' } },
					{ kind: 'button', props: { label: 'Install', default: true } },
				],
			},
		],
	};
	it('draws the dark rounded Afterglow window with a step list, cards and an accent bar', () => {
		const html = renderTreeHtml(modern, {
			renderer: 'canvas',
			theme: 'afterglow',
			vars: { 'progress.percent': '62' },
			state: { m: 0.62 },
		});
		expect(html).toContain('pvAfterglow');
		expect(html).toContain('data-w="1024" data-h="720"');
		expect(html).toContain('aria-current="step"');
		expect(html.match(/pvStateDone/g)).toHaveLength(1);
		expect(html).toContain('pvCards');
		expect(html).toContain('· required');
		expect(html).toContain('Adds to PATH');
		expect(html).toContain('style="width:62%"');
		expect(html).toContain('62%</span>');
		expect(html).toContain('No administrator needed');
		expect(html).toContain('pvDefault');
	});
	it('draws the same tree as Neutral for the Neutral theme', () => {
		const html = renderTreeHtml(modern, { renderer: 'canvas', theme: 'neutral' });
		expect(html).toContain('pvNeutral');
		expect(html).not.toContain('pvAfterglow');
	});
	it('uses a renderer override where one exists and escapes content', () => {
		const layout: Layout = {
			version: 1,
			id: 'x',
			root: {
				kind: 'text',
				props: { text: '<script>x</script>' },
				overrides: { web: { kind: 'text', props: { text: 'WEB' } } },
			},
		};
		expect(renderLayoutHtml(layout, 'canvas')).not.toContain('<script>');
		expect(renderLayoutHtml(layout, 'web')).toContain('WEB');
	});
	it('skips custom nodes outside canvas and web', () => {
		const layout: Layout = {
			version: 1,
			id: 'x',
			root: { kind: 'window', props: { title: 'T' }, children: [{ kind: 'custom' }] },
		};
		expect(renderLayoutHtml(layout, 'tui-retro')).not.toContain('custom content');
		expect(renderLayoutHtml(layout, 'canvas')).toContain('custom content');
	});
});

describe('the Windows 11 look', () => {
	const stage: LayoutNode = {
		kind: 'backdrop',
		props: { style: 'win11', title: 'Lieutenant Fizz', subtitle: 'Episode One', credit: 'Powered' },
		children: [
			{
				kind: 'meter',
				bind: 'engine.meter.free_space',
				props: { label: 'Free space', source: 'free_space', region: 'status', warn_label: 'Low' },
			},
			{ kind: 'slot' },
		],
	};
	const page: LayoutNode = {
		kind: 'window',
		props: { title: 'Lieutenant Fizz Setup', style: 'win11' },
		children: [
			{
				kind: 'stack',
				props: { region: 'side' },
				children: [
					{ kind: 'text', props: { text: 'Welcome', role: 'step-item', state: 'done' } },
					{ kind: 'text', props: { text: 'Components', role: 'step-item', state: 'current' } },
				],
			},
			{
				kind: 'stack',
				children: [
					{
						kind: 'checklist',
						bind: 'c',
						props: {
							label: 'Components',
							cards: true,
							control: 'toggle',
							items: [
								{ id: 'a', label: 'Game', required: true, checked: true },
								{ id: 'b', label: 'Levels' },
							],
						},
					},
					{
						kind: 'checklist',
						bind: 'f',
						props: { label: 'After setup', items: [{ id: 'launch', label: 'Launch' }] },
					},
					{ kind: 'button', props: { label: 'Open README', variant: 'link' } },
					{
						kind: 'image',
						props: { label: 'Hero', art, panel: '#101040', variant: 'hero' },
					},
				],
			},
			{
				kind: 'stack',
				props: { region: 'footer', direction: 'row' },
				children: [
					{ kind: 'text', props: { text: 'No administrator', role: 'note' } },
					{ kind: 'button', props: { label: 'Next', default: true } },
				],
			},
		],
	};
	const html = (extra: Partial<RenderOptions> = {}): string =>
		renderToStaticMarkup(
			<LayoutPreview
				layout={{ version: 1, id: 'fizz-modern/components', root: page }}
				stage={{ version: 1, id: 'fizz-modern/stage', root: stage }}
				renderer="web"
				options={{ theme: 'fizz-modern', ...extra }}
			/>,
		);

	it('draws a Mica window with caption buttons, a rail and the status band', () => {
		const out = html();
		expect(out).toContain('data-look="win11"');
		expect(out).toContain('pvModern');
		expect(out).toContain('pvKWindow');
		expect(out).toContain('pvKCaption');
		expect(out).toContain('pvKRail');
		expect(out).toContain('pvKStatus');
		expect(out).toContain('aria-current="step"');
		expect(out).toContain('Lieutenant Fizz · Episode One');
	});

	it('follows the system scheme unless one is pinned', () => {
		expect(html()).toContain('data-scheme="system"');
		expect(html({ scheme: 'dark' })).toContain('data-scheme="dark"');
		expect(html({ scheme: 'light' })).toContain('data-scheme="light"');
	});

	it('draws optional components as toggles and other checklists as checkboxes', () => {
		const out = html();
		expect(out.match(/pvSwitch/g)?.length).toBe(2);
		expect(out).toContain('pvToggles');
		expect(out).toContain('pvLocked');
		expect(out.match(/pvBox/g)?.length).toBe(1);
	});

	it('draws a link button and a hero image', () => {
		const out = html();
		expect(out).toContain('pvLink');
		expect(out).toContain('pvHero');
	});

	it('badges free space as Low when the drive is nearly full', () => {
		expect(html({ live: { freeSpace: 0.02 } })).toContain('pvLow');
		expect(html({ live: { freeSpace: 0.6 } })).not.toContain('pvLow');
	});
});
