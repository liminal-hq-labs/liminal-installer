// The full-screen preview window: what it is opened with and how big it opens.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { STAGE, type Look } from './model';
import type { Scheme } from './model';
import type { TerminalView } from './TerminalPreview';

/** The `?window=` kind, and the label the shell gives the window. */
export const PREVIEW_WINDOW = 'preview';

/** Height of the title bar and the stepper bar around the stage, in pixels. */
export const WINDOW_CHROME_HEIGHT = 92;
/** The narrowest the window opens: the stepper bar needs this much. */
export const WINDOW_MIN_WIDTH = 720;

/** What the window opens with: the target and the step on show, how a terminal target is drawn and the colour scheme. */
export interface PreviewQuery {
	target: string;
	step: number;
	view: TerminalView;
	scheme: Scheme;
}

/** The query string the window opens with. A default view and scheme are left out. */
export function previewWindowQuery(
	target: string,
	step: number,
	view: TerminalView = 'rich',
	scheme: Scheme = 'system',
): string {
	const params = new URLSearchParams({ target, step: String(step) });
	if (view !== 'rich') params.set('view', view);
	if (scheme !== 'system') params.set('scheme', scheme);
	return params.toString();
}

export function parsePreviewQuery(search: string): PreviewQuery | null {
	const params = new URLSearchParams(search);
	const target = params.get('target');
	if (!target) return null;
	const step = Number(params.get('step') ?? 0);
	const scheme = params.get('scheme');
	return {
		target,
		step: Number.isInteger(step) && step >= 0 ? step : 0,
		view: params.get('view') === 'terminal' ? 'terminal' : 'rich',
		scheme: scheme === 'light' || scheme === 'dark' ? scheme : 'system',
	};
}

/** The text-mode screen the full-screen window draws: 80 by 25 cells. */
export const TERMINAL_COLS = 80;
export const TERMINAL_ROWS = 25;

/**
 * How much to draw a terminal of `cols` by `rows` cells, each `cellW` by `cellH` pixels, in a box.
 * The zoom is the largest whole number that fits, so the cells stay sharp; a box too small for
 * even that scales down to fit instead. The aspect ratio is always kept.
 */
export function terminalFit(
	box: { width: number; height: number },
	cols: number,
	rows: number,
	cellW: number,
	cellH: number,
): { zoom: number; width: number; height: number } {
	const natural = { w: cols * cellW, h: rows * cellH };
	const fit = Math.min(box.width / natural.w, box.height / natural.h);
	const zoom = !(fit > 0) ? 1 : fit >= 1 ? Math.floor(fit) : fit;
	return { zoom, width: Math.round(natural.w * zoom), height: Math.round(natural.h * zoom) };
}

export interface WindowFit {
	width: number;
	height: number;
	/** How much the stage is scaled down from its natural size to fit the screen. */
	scale: number;
}

/**
 * The inner size to open the window at: the look's natural stage size plus the window's own bars,
 * scaled down, with the aspect ratio kept, until it fits the screen with a margin.
 */
export function fitWindow(
	look: Look,
	screen: { width: number; height: number },
	margin = 0.92,
): WindowFit {
	const { w, h } = STAGE[look];
	const scale = Math.min(
		1,
		(screen.width * margin) / w,
		(screen.height * margin - WINDOW_CHROME_HEIGHT) / h,
	);
	return {
		scale,
		width: Math.max(WINDOW_MIN_WIDTH, Math.round(w * scale)),
		height: Math.round(h * scale + WINDOW_CHROME_HEIGHT),
	};
}
