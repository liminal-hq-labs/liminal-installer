// The full-screen preview window: what it is opened with and how big it opens.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { STAGE, type Look } from './model';

/** The `?window=` kind, and the label the shell gives the window. */
export const PREVIEW_WINDOW = 'preview';

/** Height of the title bar and the stepper bar around the stage, in pixels. */
export const WINDOW_CHROME_HEIGHT = 92;
/** The narrowest the window opens: the stepper bar needs this much. */
export const WINDOW_MIN_WIDTH = 720;

/** The query string the window opens with: the target and the step on show. */
export function previewWindowQuery(target: string, step: number): string {
	return new URLSearchParams({ target, step: String(step) }).toString();
}

export function parsePreviewQuery(search: string): { target: string; step: number } | null {
	const params = new URLSearchParams(search);
	const target = params.get('target');
	if (!target) return null;
	const step = Number(params.get('step') ?? 0);
	return { target, step: Number.isInteger(step) && step >= 0 ? step : 0 };
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
