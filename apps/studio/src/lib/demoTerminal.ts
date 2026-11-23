// A stand-in terminal for the browser demo: a framed screen that names the step, to try the Studio's terminal view without the renderer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TuiApi } from './api';
import type { Cell, CellGrid, TuiFrame } from './types';

const BOX = { tl: '╔', tr: '╗', bl: '╚', br: '╝', h: '═', v: '║' };

/** A blue screen with a double-line box holding the step, the way the real renderer frames a page. */
export function demoGrid(page: string, cols: number, rows: number): CellGrid {
	const cells: Cell[] = Array.from({ length: cols * rows }, () => ({
		ch: '░',
		fg: 'blue',
		bg: 'black',
	}));
	const put = (x: number, y: number, ch: string, fg = 'white', bg = 'blue') => {
		if (x >= 0 && x < cols && y >= 0 && y < rows) cells[y * cols + x] = { ch, fg, bg };
	};
	const w = Math.min(cols - 4, 60);
	const h = Math.min(rows - 4, 11);
	const x0 = Math.floor((cols - w) / 2);
	const y0 = Math.floor((rows - h) / 2);
	for (let y = 0; y < h; y++)
		for (let x = 0; x < w; x++) {
			const edgeX = x === 0 || x === w - 1;
			const edgeY = y === 0 || y === h - 1;
			const ch =
				edgeX && edgeY
					? y === 0
						? x === 0
							? BOX.tl
							: BOX.tr
						: x === 0
							? BOX.bl
							: BOX.br
					: edgeY
						? BOX.h
						: edgeX
							? BOX.v
							: ' ';
			put(x0 + x, y0 + y, ch);
		}
	const text = (s: string, y: number, fg = 'white') =>
		[...s].forEach((ch, i) => put(x0 + Math.floor((w - s.length) / 2) + i, y0 + y, ch, fg));
	text(`[ ${page} ]`, 0, 'yellow');
	text('Demo terminal', 3);
	text('The real renderer runs in the Studio app.', 5, 'cyan');
	text('Enter: Next · Esc: Cancel', h - 4);
	text('< Next >    < Cancel >', h - 2, 'black');
	return { width: cols, height: rows, cells };
}

/** Sessions that answer Enter with Next and Esc with Cancel, and resize to any size. */
export function demoTerminal(): TuiApi {
	const sessions = new Map<string, { page: string; cols: number; rows: number }>();
	let n = 0;
	const frame = (id: string): TuiFrame => {
		const s = sessions.get(id)!;
		return { grid: demoGrid(s.page, s.cols, s.rows), focus: 'Next, button', state: {} };
	};
	return {
		tuiSessionOpen: async (_target, page, _state, size) => {
			const id = `demo-${++n}`;
			sessions.set(id, { page, cols: size?.cols ?? 80, rows: size?.rows ?? 24 });
			return { session: id, frame: frame(id) };
		},
		tuiSessionKey: async (session, key) => ({
			frame: frame(session),
			outcome:
				key.key === 'Enter'
					? { kind: 'activate', action: 'next' }
					: key.key === 'Escape'
						? { kind: 'cancel' }
						: { kind: 'none' },
		}),
		tuiSessionResize: async (session, cols, rows) => {
			const s = sessions.get(session);
			if (s) Object.assign(s, { cols, rows });
			return frame(session);
		},
		tuiSessionClose: async (session) => {
			sessions.delete(session);
		},
	};
}
