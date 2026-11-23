// Tests the HTML preview of layouts in the retro and canvas styles.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { escapeHtml, fill, renderLayoutHtml, styleFor } from './previewRender';
import type { Layout } from './types';

const layout: Layout = {
	version: 1,
	id: 'copying',
	root: {
		kind: 'dialog',
		props: { title: 'Installing {product.name}' },
		children: [
			{ kind: 'text', props: { text: 'Copying {progress.file}', role: 'body' } },
			{ kind: 'meter', props: { label: 'Overall', source: 'total' } },
			{ kind: 'button', props: { label: 'Cancel', default: true } },
			{ kind: 'radio', props: { label: 'Scope', options: [{ id: 'u', label: 'Only me' }] } },
			{ kind: 'custom' },
		],
		overrides: { 'tui-retro': { kind: 'backdrop', props: { title: 'RETRO' } } },
	},
};

describe('styleFor', () => {
	it('uses the retro style for text-mode renderers only', () => {
		expect(styleFor('tui-retro')).toBe('retro');
		expect(styleFor('tui')).toBe('retro');
		expect(styleFor('canvas')).toBe('canvas');
		expect(styleFor('win32-classic')).toBe('canvas');
	});
});

describe('text helpers', () => {
	it('escapes HTML and fills known placeholders only', () => {
		expect(escapeHtml('<a href="x">&</a>')).toBe('&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;');
		expect(fill('v{product.version} {unknown.thing}')).toBe('v1.0.0 {unknown.thing}');
	});
});

describe('renderLayoutHtml', () => {
	it('draws the canvas style with graphical controls', () => {
		const html = renderLayoutHtml(layout, 'canvas');
		expect(html).toContain('pv-canvas');
		expect(html).toContain('Installing Sample Product');
		expect(html).toContain('Copying data/level-03.lvl');
		expect(html).toContain('pv-bar-fill');
		expect(html).toContain('>Cancel</button>');
		expect(html).toContain('pv-dot');
		expect(html).toContain('custom content');
	});

	it('draws the retro style with text-mode controls and skips custom nodes', () => {
		const flat: Layout = { ...layout, root: { ...layout.root, overrides: undefined } };
		const html = renderLayoutHtml(flat, 'tui-retro');
		expect(html).toContain('pv-retro');
		expect(html).toContain('[ Cancel ]');
		expect(html).toContain('█');
		expect(html).toContain('(•)');
		expect(html).not.toContain('custom content');
	});

	it('uses a renderer override where one exists', () => {
		expect(renderLayoutHtml(layout, 'tui-retro')).toContain('RETRO');
		expect(renderLayoutHtml(layout, 'canvas')).not.toContain('RETRO');
	});

	it('escapes content from the layout', () => {
		const evil: Layout = { ...layout, root: { kind: 'text', props: { text: '<script>x</script>' } } };
		expect(renderLayoutHtml(evil, 'canvas')).not.toContain('<script>');
	});
});
