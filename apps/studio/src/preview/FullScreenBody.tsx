// The full-screen preview: a stepper bar over the screen at the largest size that fits, shared by the child window and the in-page view.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import type { TuiApi } from '../lib/api';
import { stepFrom } from './sim';
import type { ComponentInfo, Preview, TargetInfo } from '../lib/types';
import { cx } from '../ui/cx';
import ui from '../ui/ui.module.css';
import { SPEEDS, RunStage } from './RunStage';
import type { Run } from './useRun';
import { hasMovingParts, hasSchemes, lookFor, type Scheme } from './model';
import { SchemeSwitch } from './SchemeSwitch';
import { stepFor, TerminalPreview, TerminalViewSwitch, type TerminalView } from './TerminalPreview';
import styles from './FullScreenBody.module.css';

export interface FullScreenBodyProps {
	pv: Preview | null;
	run: Run;
	components: ComponentInfo[];
	targets: TargetInfo[];
	target: string;
	index: number;
	setIndex: (i: number) => void;
	onTarget: (target: string) => void;
	/** Whether the window is truly full screen, and the toggle for it. */
	fullscreen: boolean;
	onToggleFullscreen: () => void;
	/** Shown by the in-page view, which has no title bar to close it. */
	onClose?: () => void;
	/** The target's renderer while the preview is still loading. */
	loading?: boolean;
	/** How a terminal target is drawn, and the way to change it. Only `tui` targets offer the real terminal. */
	view: TerminalView;
	onView: (view: TerminalView) => void;
	/** Runs the terminal sessions; without it the real terminal is not offered. */
	api?: TuiApi | null;
	/** Light, dark or the system's colours, for a look that has both, and the way to change it. */
	scheme: Scheme;
	onScheme: (scheme: Scheme) => void;
}

export function FullScreenBody(props: FullScreenBodyProps) {
	const { pv, run, components, targets, target, index, setIndex } = props;
	const steps = pv?.steps ?? [];
	const step = steps[index];
	const info = targets.find((x) => x.full_id === target);
	const renderer = pv?.renderer ?? info?.renderer ?? 'canvas';
	const moving = step?.layout_tree ? hasMovingParts(step.layout_tree) : false;
	const p = run.playback;
	const style = step?.layout_tree?.props?.style;
	const schemes = hasSchemes(
		lookFor(renderer, pv?.theme ?? info?.theme, typeof style === 'string' ? style : undefined),
	);
	const terminal = renderer.startsWith('tui') && Boolean(props.api);
	const real = terminal && props.view === 'terminal' && pv && step?.layout && !step.skipped;
	return (
		<div className={styles.body} data-testid="fullscreen-preview">
			<div className={styles.bar} role="toolbar" aria-label={t('preview.stepper')}>
				<button
					type="button"
					className={ui.btn}
					disabled={stepFrom(steps, index, -1) === index}
					onClick={() => setIndex(stepFrom(steps, index, -1))}
				>
					{t('preview.back')}
				</button>
				<select
					aria-label={t('preview.steps')}
					value={index}
					onChange={(e) => setIndex(Number(e.target.value))}
				>
					{steps.map((st, i) => (
						<option key={`${st.id}-${i}`} value={i} disabled={st.skipped}>
							{st.skipped ? `${st.id} · ${t('preview.skippedForTarget')}` : st.id}
						</option>
					))}
				</select>
				<button
					type="button"
					className={ui.btn}
					disabled={stepFrom(steps, index, 1) === index}
					onClick={() => setIndex(stepFrom(steps, index, 1))}
				>
					{t('preview.next')}
				</button>
				<select
					aria-label={t('preview.target')}
					value={target}
					onChange={(e) => props.onTarget(e.target.value)}
				>
					{targets.map((x) => (
						<option key={x.full_id} value={x.full_id}>
							{`${x.id} · ${x.renderer}`}
						</option>
					))}
				</select>
				{terminal ? <TerminalViewSwitch value={props.view} onChange={props.onView} /> : null}
				{schemes ? <SchemeSwitch value={props.scheme} onChange={props.onScheme} /> : null}
				<span className={styles.spacer} />
				<button
					type="button"
					className={ui.btn}
					disabled={!moving || Boolean(real)}
					aria-pressed={p.playing}
					onClick={() => (p.playing ? p.pause() : p.play())}
				>
					{p.playing ? t('preview.pause') : t('preview.play')}
				</button>
				<select
					aria-label={t('preview.speed')}
					value={p.speed}
					onChange={(e) => p.setSpeed(Number(e.target.value))}
				>
					{SPEEDS.map((s) => (
						<option key={s} value={s}>
							{`${s}×`}
						</option>
					))}
				</select>
				<button
					type="button"
					className={cx(ui.btn, props.fullscreen && ui.primary)}
					aria-pressed={props.fullscreen}
					onClick={props.onToggleFullscreen}
				>
					{props.fullscreen ? t('preview.exitFullScreen') : t('preview.fullScreen')}
				</button>
				{props.onClose ? (
					<button type="button" className={ui.btn} onClick={props.onClose}>
						{t('common.close')}
					</button>
				) : null}
			</div>
			<div className={styles.stage}>
				{real && props.api ? (
					<TerminalPreview
						fill
						autoFocus
						api={props.api}
						target={pv.target}
						page={step.id}
						state={step.state}
						label={t('terminal.label', { id: step.id })}
						onOutcome={(outcome) => {
							const to = stepFor(outcome);
							if (to) run.onAction(to);
						}}
					/>
				) : pv && step?.layout_tree ? (
					<RunStage
						pv={pv}
						step={step}
						run={run}
						components={components}
						renderer={renderer}
						theme={pv.theme ?? info?.theme}
						scheme={props.scheme}
						fit="contain"
					/>
				) : (
					<p className={styles.empty}>
						{props.loading ? t('common.loading') : step ? t('preview.noScreen') : ''}
					</p>
				)}
			</div>
		</div>
	);
}
