// Tests the terminal painter: colours, box and block characters, and that lines meet without gaps.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import type { Cell, CellGrid } from '../lib/types';
import {
	boxRects,
	cellColours,
	cssColour,
	DEFAULT_BG,
	DEFAULT_FG,
	indexedColour,
	mix,
	paintGrid,
	type PaintContext,
} from './terminalPaint';

const cell = (ch: string, fg = 'default', bg = 'default', modifiers?: Cell['modifiers']): Cell => ({
	ch,
	fg,
	bg,
	...(modifiers ? { modifiers } : {}),
});

const gridOf = (rows: string[], fg = 'white', bg = 'blue'): CellGrid => ({
	width: rows[0]!.length,
	height: rows.length,
	cells: rows.flatMap((r) => [...r].map((ch) => cell(ch, fg, bg))),
});

interface Call {
	op: 'rect' | 'text';
	args: (number | string)[];
	style: string;
}

function recorder(): { ctx: PaintContext; calls: Call[] } {
	const calls: Call[] = [];
	const ctx: PaintContext = {
		fillStyle: '',
		font: '',
		textAlign: 'left',
		textBaseline: 'alphabetic',
		globalAlpha: 1,
		fillRect(x, y, w, h) {
			calls.push({ op: 'rect', args: [x, y, w, h], style: String(ctx.fillStyle) });
		},
		fillText(text, x, y) {
			calls.push({ op: 'text', args: [text, x, y], style: String(ctx.fillStyle) });
		},
	};
	return { ctx, calls };
}

/** Paints the grid into a bitmap of the pixels the foreground colour covers. */
function raster(grid: CellGrid, w: number, h: number, t: number): boolean[][] {
	const bits = Array.from({ length: grid.height * h }, () =>
		Array<boolean>(grid.width * w).fill(false),
	);
	grid.cells.forEach((c, i) => {
		const rects = boxRects(c.ch, w, h, t);
		if (!rects) return;
		const [cx, cy] = [(i % grid.width) * w, Math.floor(i / grid.width) * h];
		for (const [x, y, rw, rh] of rects)
			for (let j = y; j < y + rh; j++)
				for (let k = x; k < x + rw; k++) bits[cy + j]![cx + k] = true;
	});
	return bits;
}

describe('colours', () => {
	it('maps wire colours to CSS and the default pair', () => {
		expect(cssColour('blue', 'x')).toBe('#0000aa');
		expect(cssColour('white', 'x')).toBe('#ffffff');
		expect(cssColour('#012bff', 'x')).toBe('#012bff');
		expect(cssColour('default', 'x')).toBe('x');
		expect(cssColour('nonsense', 'x')).toBe('x');
		expect(cssColour('indexed:9', 'x')).toBe('#ff5555');
		expect(indexedColour(196)).toBe('#ff0000');
		expect(indexedColour(244)).toBe('#808080');
	});

	it('swaps a reversed cell and leaves defaults to the terminal', () => {
		expect(cellColours(cell('x', 'white', 'blue'))).toEqual({ fg: '#ffffff', bg: '#0000aa' });
		expect(cellColours(cell('x', 'white', 'blue', { reversed: true }))).toEqual({
			fg: '#0000aa',
			bg: '#ffffff',
		});
		expect(cellColours(cell('x'))).toEqual({ fg: DEFAULT_FG, bg: DEFAULT_BG });
	});

	it('mixes hex colours and leaves others alone', () => {
		expect(mix('#ffffff', '#000000', 0.5)).toBe('#808080');
		expect(mix('red', '#000000', 0.5)).toBe('red');
	});
});

describe('box drawing', () => {
	const [w, h, t] = [10, 20, 1];

	it('draws light lines across the cell centre', () => {
		expect(boxRects('─', w, h, t)).toEqual([[0, 10, 10, 1]]);
		expect(boxRects('│', w, h, t)).toEqual([[5, 0, 1, 20]]);
		expect(boxRects('a', w, h, t)).toBeNull();
	});

	it('closes corners and joins tees', () => {
		const rects = boxRects('┌', w, h, t)!;
		expect(rects).toContainEqual([5, 10, 5, 1]);
		expect(rects).toContainEqual([5, 10, 1, 10]);
		// A cross is two full lines, a tee has the stem only on its side.
		expect(boxRects('┼', w, h, t)).toHaveLength(2);
		expect(boxRects('┴', w, h, t)).toContainEqual([5, 0, 1, 11]);
	});

	it('draws double lines as two lines a gap apart', () => {
		expect(boxRects('═', w, h, t)).toHaveLength(2);
		expect(boxRects('╬', w, h, t)).toHaveLength(8);
	});

	it('leaves no gap along a row or down a column of box characters', () => {
		for (const [cw, ch, t] of [
			[10, 20, 1],
			[20, 40, 2],
			[9, 18, 1],
		] as const) {
			const g = gridOf(['╔═══╗', '║   ║', '╠═══╣', '║   ║', '╚═══╝']);
			const bits = raster(g, cw, ch, t);
			// Every column along the top and bottom edge is covered on a row of the outer line.
			const outerTop = bits.findIndex((row) => row.some(Boolean));
			expect(bits[outerTop]!.slice(cw >> 1, 4 * cw + (cw >> 1)).every(Boolean)).toBe(true);
			const outerLeft = bits[outerTop]!.findIndex(Boolean);
			for (let y = outerTop; y < bits.length - 1; y++) {
				if (y >= 4 * ch + (ch >> 1) - 1 && y > 4 * ch) break;
				expect(bits[y]![outerLeft], `column at y=${y} cell ${cw}x${ch}`).toBe(true);
			}
			// The tee joins the left line with the middle line.
			const midY = 2 * ch + (ch >> 1);
			expect(
				bits.slice(midY - 4, midY + 4).some((row) => row.slice(cw, 4 * cw).every(Boolean)),
			).toBe(true);
		}
	});

	it('light frames line up too', () => {
		const g = gridOf(['┌──┐', '│  │', '├──┤', '└──┘']);
		const bits = raster(g, 10, 20, 1);
		const top = bits.findIndex((r) => r.some(Boolean));
		expect(bits[top]!.slice(5, 35).every(Boolean)).toBe(true);
		expect(bits.slice(top, 3 * 20 + 11).every((r) => r[5])).toBe(true);
	});
});

describe('painting', () => {
	const m = { cellW: 10, cellH: 20, font: '20px VT323' };

	it('fills each row background once per colour run', () => {
		const { ctx, calls } = recorder();
		paintGrid(ctx, gridOf(['abc', 'def']), m);
		const bgs = calls.filter((c) => c.op === 'rect' && c.style === '#0000aa');
		expect(bgs.map((c) => c.args)).toEqual([
			[0, 0, 30, 20],
			[0, 20, 30, 20],
		]);
	});

	it('draws letters at the middle of their cell and box characters as shapes', () => {
		const { ctx, calls } = recorder();
		paintGrid(ctx, gridOf(['a─']), m);
		const texts = calls.filter((c) => c.op === 'text');
		expect(texts).toHaveLength(1);
		expect(texts[0]!.args[0]).toBe('a');
		expect(texts[0]!.args[1]).toBe(5);
		expect(texts[0]!.style).toBe('#ffffff');
		expect(calls.some((c) => c.op === 'rect' && c.style === '#ffffff' && c.args[0] === 10)).toBe(
			true,
		);
	});

	it('draws shades as a blend and full blocks in the foreground', () => {
		const { ctx, calls } = recorder();
		paintGrid(ctx, gridOf(['█░'], 'white', 'blue'), m);
		const fg = calls.filter((c) => c.op === 'rect').slice(1);
		expect(fg[0]).toMatchObject({ args: [0, 0, 10, 20], style: '#ffffff' });
		expect(fg[1]!.style).toBe(mix('#ffffff', '#0000aa', 0.25));
	});

	it('skips the right half of a wide character and underlines blanks when asked', () => {
		const { ctx, calls } = recorder();
		const g: CellGrid = {
			width: 3,
			height: 1,
			cells: [
				cell('語', 'white', 'blue'),
				cell('', 'white', 'blue'),
				cell(' ', 'white', 'blue', { underline: true }),
			],
		};
		paintGrid(ctx, g, m);
		expect(calls.filter((c) => c.op === 'text')).toHaveLength(1);
		expect(calls.some((c) => c.op === 'rect' && c.args[0] === 20 && c.args[3] === 1)).toBe(true);
	});
});
