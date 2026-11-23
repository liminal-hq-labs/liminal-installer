// Paints a terminal cell grid on a canvas: exact colours, and box and block characters drawn as
// shapes so they meet without gaps at any size.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Cell, CellGrid } from '../lib/types';

/** The sixteen ANSI colours as VGA text mode drew them, and the terminal's default pair. */
export const PALETTE: Record<string, string> = {
	black: '#000000',
	red: '#aa0000',
	green: '#00aa00',
	yellow: '#aa5500',
	blue: '#0000aa',
	magenta: '#aa00aa',
	cyan: '#00aaaa',
	gray: '#aaaaaa',
	dark_gray: '#555555',
	light_red: '#ff5555',
	light_green: '#55ff55',
	light_yellow: '#ffff55',
	light_blue: '#5555ff',
	light_magenta: '#ff55ff',
	light_cyan: '#55ffff',
	white: '#ffffff',
};

export const DEFAULT_FG = '#d4d4d4';
export const DEFAULT_BG = '#0c0c10';

const hex2 = (n: number) => n.toString(16).padStart(2, '0');
const rgbHex = (r: number, g: number, b: number) => `#${hex2(r)}${hex2(g)}${hex2(b)}`;

/** The xterm 256-colour palette entry at `i`. */
export function indexedColour(i: number): string {
	if (i < 16) return Object.values(PALETTE)[i] ?? DEFAULT_FG;
	if (i >= 232) {
		const v = 8 + (i - 232) * 10;
		return rgbHex(v, v, v);
	}
	const n = i - 16;
	const level = (k: number) => (k === 0 ? 0 : 55 + k * 40);
	return rgbHex(level(Math.floor(n / 36)), level(Math.floor(n / 6) % 6), level(n % 6));
}

/** A wire colour (`default`, a name, `indexed:N` or `#rrggbb`) as a CSS colour, or `fallback`. */
export function cssColour(wire: string, fallback: string): string {
	if (wire === 'default') return fallback;
	if (wire.startsWith('#')) return wire;
	if (wire.startsWith('indexed:')) return indexedColour(Number(wire.slice(8)));
	return PALETTE[wire] ?? fallback;
}

/** The colours a cell is drawn in, with `reversed` applied and `dim` blended toward the background. */
export function cellColours(cell: Cell): { fg: string; bg: string } {
	let fg = cssColour(cell.fg, DEFAULT_FG);
	let bg = cssColour(cell.bg, DEFAULT_BG);
	if (cell.modifiers?.reversed) [fg, bg] = [bg, fg];
	return { fg, bg };
}

/** A rectangle inside one cell, in device pixels: x, y, width, height. */
export type Rect = [x: number, y: number, w: number, h: number];

// Arms of each box-drawing character: [up, down, left, right], 1 for a light line and 2 for a double one.
const ARMS: Record<string, [number, number, number, number]> = {
	'─': [0, 0, 1, 1],
	'│': [1, 1, 0, 0],
	'┌': [0, 1, 0, 1],
	'┐': [0, 1, 1, 0],
	'└': [1, 0, 0, 1],
	'┘': [1, 0, 1, 0],
	'├': [1, 1, 0, 1],
	'┤': [1, 1, 1, 0],
	'┬': [0, 1, 1, 1],
	'┴': [1, 0, 1, 1],
	'┼': [1, 1, 1, 1],
	'═': [0, 0, 2, 2],
	'║': [2, 2, 0, 0],
	'╔': [0, 2, 0, 2],
	'╗': [0, 2, 2, 0],
	'╚': [2, 0, 0, 2],
	'╝': [2, 0, 2, 0],
	'╠': [2, 2, 0, 2],
	'╣': [2, 2, 2, 0],
	'╦': [0, 2, 2, 2],
	'╩': [2, 0, 2, 2],
	'╬': [2, 2, 2, 2],
};

interface Axis {
	/** Cell length along the lines, and where the crossing lines sit along it. */
	len: number;
	centre: number;
}

/**
 * The lines that run along one axis. `a0` and `a1` are the arms toward the start and the end of
 * the axis, and `p0` and `p1` the arms of the crossing lines (toward the start and the end of the
 * other axis). Returns `[from, to, offset]` for each line, offsets measured from `cross`.
 */
function linesAlong(
	a0: number,
	a1: number,
	p0: number,
	p1: number,
	ax: Axis,
	d: number,
	t: number,
): [number, number, number][] {
	const n = Math.max(a0, a1);
	if (n === 0) return [];
	const half = Math.floor(t / 2);
	const offsets = n === 2 ? [-d, d] : [0];
	const out: [number, number, number][] = [];
	for (const o of offsets) {
		if (a0 > 0 && a1 > 0) {
			// Through line, cut where a crossing line leaves on its side.
			const cut = (o < 0 && p0 > 0) || (o > 0 && p1 > 0);
			if (!cut) out.push([0, ax.len, o]);
			else {
				out.push([0, ax.centre - d + t - half, o]);
				out.push([ax.centre + d - half, ax.len, o]);
			}
			continue;
		}
		const toEnd = a1 > 0;
		const through = p0 > 0 && p1 > 0;
		const pd = p0 > 0 ? -1 : 1;
		// The crossing lines are at centre - d and centre + d (or centre). A stub from the
		// start runs to one of them, and a stub to the end starts at one.
		let at: number;
		if (p0 === 0 && p1 === 0) at = ax.centre;
		else if (through) at = toEnd ? ax.centre - d : ax.centre + d;
		else {
			const outer = o * pd < 0;
			at = toEnd ? ax.centre + (outer ? -d : d) : ax.centre + (outer ? d : -d);
		}
		if (toEnd) out.push([at - half, ax.len, o]);
		else out.push([0, at + t - half, o]);
	}
	return out;
}

/**
 * The rectangles of a box-drawing character in a `w` by `h` cell: light (`─│┌┐└┘├┤┬┴┼`) or
 * double (`═║╔╗╚╝╠╣╦╩╬`) lines `t` pixels thick. Anything else gives `null`. Rectangles share
 * their edges, so lines in neighbouring cells meet without a gap.
 */
export function boxRects(ch: string, w: number, h: number, t: number): Rect[] | null {
	const arms = ARMS[ch];
	if (!arms) return null;
	const [up, down, left, right] = arms;
	const half = Math.floor(t / 2);
	const double = Math.max(...arms) === 2;
	const d = double ? Math.max(t + 1, Math.round(t * 1.5)) : 0;
	const cx = Math.floor(w / 2);
	const cy = Math.floor(h / 2);
	const out: Rect[] = [];
	for (const [from, to, o] of linesAlong(left, right, up, down, { len: w, centre: cx }, d, t)) {
		out.push([from, cy + o - half, to - from, t]);
	}
	for (const [from, to, o] of linesAlong(up, down, left, right, { len: h, centre: cy }, d, t)) {
		out.push([cx + o - half, from, t, to - from]);
	}
	return out;
}

/** The fraction of a cell a block character fills, and how opaque, or `null`. */
export function blockFill(
	ch: string,
): { x: number; y: number; w: number; h: number; a: number } | null {
	switch (ch) {
		case '█':
			return { x: 0, y: 0, w: 1, h: 1, a: 1 };
		case '▀':
			return { x: 0, y: 0, w: 1, h: 0.5, a: 1 };
		case '▄':
			return { x: 0, y: 0.5, w: 1, h: 0.5, a: 1 };
		case '▌':
			return { x: 0, y: 0, w: 0.5, h: 1, a: 1 };
		case '▐':
			return { x: 0.5, y: 0, w: 0.5, h: 1, a: 1 };
		case '░':
			return { x: 0, y: 0, w: 1, h: 1, a: 0.25 };
		case '▒':
			return { x: 0, y: 0, w: 1, h: 1, a: 0.5 };
		case '▓':
			return { x: 0, y: 0, w: 1, h: 1, a: 0.75 };
		default:
			return null;
	}
}

export interface Metrics {
	/** Cell width and height in device pixels. */
	cellW: number;
	cellH: number;
	font: string;
}

/** The slice of the 2D canvas context the painter uses, so tests can record calls. */
export interface PaintContext {
	fillStyle: string | CanvasGradient | CanvasPattern;
	font: string;
	textAlign: CanvasTextAlign;
	textBaseline: CanvasTextBaseline;
	globalAlpha: number;
	fillRect(x: number, y: number, w: number, h: number): void;
	fillText(text: string, x: number, y: number): void;
}

/** Draws every cell of `grid`. Backgrounds are merged along a row so no seam shows between them. */
export function paintGrid(ctx: PaintContext, grid: CellGrid, m: Metrics): void {
	const { cellW, cellH } = m;
	const t = Math.max(1, Math.round(cellW / 8));
	ctx.textAlign = 'center';
	ctx.textBaseline = 'middle';
	ctx.font = m.font;
	ctx.globalAlpha = 1;
	for (let y = 0; y < grid.height; y++) {
		const top = y * cellH;
		let x = 0;
		while (x < grid.width) {
			const bg = cellColours(grid.cells[y * grid.width + x]!).bg;
			let end = x + 1;
			while (end < grid.width && cellColours(grid.cells[y * grid.width + end]!).bg === bg) end++;
			ctx.fillStyle = bg;
			ctx.fillRect(x * cellW, top, (end - x) * cellW, cellH);
			x = end;
		}
		for (let x = 0; x < grid.width; x++) {
			const cell = grid.cells[y * grid.width + x]!;
			const ch = cell.ch;
			const { fg, bg } = cellColours(cell);
			const left = x * cellW;
			const underline = () => ctx.fillRect(left, top + cellH - t - 1, cellW, t);
			ctx.fillStyle = fg;
			if (ch === '' || ch === ' ') {
				if (cell.modifiers?.underline) underline();
				continue;
			}
			const block = blockFill(ch);
			if (block) {
				ctx.fillStyle = block.a === 1 ? fg : mix(fg, bg, block.a);
				ctx.fillRect(
					left + Math.round(block.x * cellW),
					top + Math.round(block.y * cellH),
					Math.round(block.w * cellW),
					Math.round(block.h * cellH),
				);
				continue;
			}
			const rects = boxRects(ch, cellW, cellH, t);
			if (rects) {
				for (const [rx, ry, rw, rh] of rects) ctx.fillRect(left + rx, top + ry, rw, rh);
				continue;
			}
			ctx.fillStyle = cell.modifiers?.dim ? mix(fg, bg, 0.5) : fg;
			const baseline = top + Math.round(cellH / 2) + 1;
			ctx.fillText(ch, left + cellW / 2, baseline);
			if (cell.modifiers?.bold) ctx.fillText(ch, left + cellW / 2 + 1, baseline);
			if (cell.modifiers?.underline) underline();
		}
	}
}

const hex = (c: string): number[] | null =>
	/^#[0-9a-f]{6}$/i.test(c) ? [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16)) : null;

/** `a` over `b` at opacity `alpha`, when both are `#rrggbb`; otherwise `a`. */
export function mix(a: string, b: string, alpha: number): string {
	const pa = hex(a);
	const pb = hex(b);
	if (!pa || !pb) return a;
	const c = pa.map((v, i) => Math.round(v * alpha + pb[i]! * (1 - alpha)));
	return `#${c.map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}
