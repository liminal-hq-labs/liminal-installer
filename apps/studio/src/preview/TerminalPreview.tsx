// The real terminal screen of a `tui` or `tui-retro` page, drawn from the renderer's cell grid and played with the keyboard.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';
import { t } from '../i18n/messages';
import type { TuiApi } from '../lib/api';
import type { CellGrid, TuiKey, TuiOutcome } from '../lib/types';
import { terminalFit } from './fullScreen';
import { paintGrid, DEFAULT_BG } from './terminalPaint';
import styles from './terminal.module.css';
import { useTuiSession, type TuiSessionOptions } from './useTuiSession';

/** Cell size in CSS pixels: one to two, like a text-mode font. */
const CELL_W = 10;
const CELL_H = 20;
const FONT_FAMILY = "VT323, ui-monospace, 'DejaVu Sans Mono', Menlo, Consolas, monospace";

const NAMED_KEYS = new Set([
	'Tab',
	'Enter',
	'Escape',
	'Backspace',
	'Delete',
	'ArrowUp',
	'ArrowDown',
	'ArrowLeft',
	'ArrowRight',
	'Home',
	'End',
	'PageUp',
	'PageDown',
]);

/** The key the terminal gets for a browser key event, or null when it is not for the terminal. */
export function tuiKeyFrom(e: {
	key: string;
	shiftKey: boolean;
	ctrlKey: boolean;
	metaKey: boolean;
	altKey: boolean;
}): TuiKey | null {
	if (e.metaKey || e.altKey) return null;
	if (NAMED_KEYS.has(e.key)) {
		return { key: e.key, shift: e.shiftKey, ctrl: e.ctrlKey, alt: false };
	}
	// One character, such as a letter, a digit or a space. Control combinations stay with the app.
	if ([...e.key].length === 1 && !e.ctrlKey) {
		return { key: e.key, shift: false, ctrl: false, alt: false };
	}
	return null;
}

/** What a chosen button asks the dry run to do. */
export type TerminalStep = 'next' | 'back' | 'cancel';

/** Maps what a key did to a step through the flow: Back goes back, Cancel and Esc restart, any other button goes on. */
export function stepFor(outcome: TuiOutcome): TerminalStep | null {
	if (outcome.kind === 'cancel') return 'cancel';
	if (outcome.kind !== 'activate') return null;
	if (outcome.action === 'back') return 'back';
	if (outcome.action === 'cancel') return 'cancel';
	return 'next';
}

/** How many CSS pixels a cell is drawn at, so a window can show the text at a whole-number zoom. */
function Canvas({
	grid,
	zoom = 1,
	size,
}: {
	grid: CellGrid;
	zoom?: number;
	size?: { width: number; height: number };
}) {
	const ref = useRef<HTMLCanvasElement>(null);
	const [fontsReady, setFontsReady] = useState(0);
	useEffect(() => {
		const fonts = (document as { fonts?: FontFaceSet }).fonts;
		if (!fonts) return;
		let live = true;
		void fonts
			.load(`20px ${FONT_FAMILY.split(',')[0]}`)
			.then(() => live && setFontsReady((n) => n + 1))
			.catch(() => undefined);
		return () => {
			live = false;
		};
	}, []);
	useEffect(() => {
		const canvas = ref.current;
		const ctx = canvas?.getContext('2d');
		if (!canvas || !ctx) return;
		const scale = Math.max(1, Math.round((window.devicePixelRatio || 1) * Math.max(1, zoom)));
		const cellW = CELL_W * scale;
		const cellH = CELL_H * scale;
		canvas.width = grid.width * cellW;
		canvas.height = grid.height * cellH;
		ctx.fillStyle = DEFAULT_BG;
		ctx.fillRect(0, 0, canvas.width, canvas.height);
		paintGrid(ctx, grid, { cellW, cellH, font: `${Math.round(cellH * 1.1)}px ${FONT_FAMILY}` });
	}, [grid, fontsReady, zoom]);
	return (
		<canvas
			ref={ref}
			className={styles.canvas}
			aria-hidden="true"
			style={
				size
					? { width: size.width, height: size.height }
					: { aspectRatio: `${grid.width * CELL_W} / ${grid.height * CELL_H}` }
			}
		/>
	);
}

export interface TerminalPreviewProps extends TuiSessionOptions {
	api: TuiApi;
	/** Accessible name of the terminal, such as `Terminal screen of welcome`. */
	label: string;
	/**
	 * Fill the parent: the terminal is drawn at the largest whole-number zoom that fits, centred, and
	 * the hint says how to leave it. For the full-screen window.
	 */
	fill?: boolean;
	/** Focus the terminal when it opens, so the keyboard plays it at once. */
	autoFocus?: boolean;
}

/**
 * The real terminal screen. The renderer runs in the Studio's backend, so what is drawn is the
 * cell grid it sends; the keyboard goes to it, and the control that is focused in the screen is
 * the one the keys act on. F6 leaves the terminal, since Tab belongs to the screen.
 */
export function TerminalPreview({
	api,
	label,
	fill = false,
	autoFocus = false,
	...options
}: TerminalPreviewProps) {
	const { frame, error, press } = useTuiSession(
		api,
		fill ? { cols: 80, rows: 25, ...options } : options,
	);
	const host = useRef<HTMLDivElement>(null);
	const area = useRef<HTMLDivElement>(null);
	const [box, setBox] = useState<{ width: number; height: number } | null>(null);
	useLayoutEffect(() => {
		const el = area.current;
		if (!fill || !el) return;
		const measure = () => setBox({ width: el.clientWidth, height: el.clientHeight });
		measure();
		if (typeof ResizeObserver === 'undefined') return;
		const observer = new ResizeObserver(measure);
		observer.observe(el);
		return () => observer.disconnect();
	}, [fill]);
	const grid = frame?.grid;
	const fit =
		fill && box && grid ? terminalFit(box, grid.width, grid.height, CELL_W, CELL_H) : null;
	const cols = grid?.width ?? 0;
	useEffect(() => {
		if (autoFocus && cols > 0) host.current?.focus();
	}, [autoFocus, cols]);
	const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
		if (e.key === 'F6') {
			host.current?.blur();
			return;
		}
		const key = tuiKeyFrom(e);
		if (!key) return;
		e.preventDefault();
		press(key);
	};
	return (
		<div className={fill ? `${styles.wrap} ${styles.fill}` : styles.wrap}>
			{error ? (
				<p role="alert" className={styles.error}>
					{error}
				</p>
			) : null}
			<div ref={area} className={fill ? styles.area : styles.areaNatural}>
				<div
					ref={host}
					className={fill ? `${styles.terminal} ${styles.terminalFill}` : styles.terminal}
					style={fit ? { width: fit.width, height: fit.height } : undefined}
					role="group"
					tabIndex={0}
					aria-label={label}
					onKeyDown={onKeyDown}
				>
					{frame ? (
						<>
							<Canvas grid={frame.grid} zoom={fit?.zoom} size={fit ?? undefined} />
							<pre className={styles.sr}>{gridText(frame.grid)}</pre>
							<p className={styles.sr} aria-live="polite">
								{frame.focus}
							</p>
						</>
					) : error ? null : (
						<p className={styles.opening}>{t('terminal.opening')}</p>
					)}
				</div>
			</div>
			<p className={styles.hint}>{t(fill ? 'terminal.hintFullScreen' : 'terminal.hint')}</p>
		</div>
	);
}

/** The grid as plain rows, trailing spaces trimmed. */
export function gridText(grid: CellGrid): string {
	const rows: string[] = [];
	for (let y = 0; y < grid.height; y++) {
		rows.push(
			grid.cells
				.slice(y * grid.width, (y + 1) * grid.width)
				.map((c) => c.ch)
				.join('')
				.trimEnd(),
		);
	}
	return rows.join('\n');
}

export type TerminalView = 'rich' | 'terminal';

/** Picks between the rich screen and the real terminal. Only `tui` targets offer the second. */
export function TerminalViewSwitch({
	value,
	onChange,
}: {
	value: TerminalView;
	onChange: (view: TerminalView) => void;
}) {
	return (
		<label className={styles.viewSwitch}>
			<span>{t('terminal.view')}</span>
			<select value={value} onChange={(e) => onChange(e.target.value as TerminalView)}>
				<option value="rich">{t('terminal.viewRich')}</option>
				<option value="terminal">{t('terminal.viewReal')}</option>
			</select>
		</label>
	);
}
