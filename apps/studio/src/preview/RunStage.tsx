// A step's screen that answers: Next and Back move through the flow, controls change, and meters can be played.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import type { ComponentInfo, Preview, PreviewStep } from '../lib/types';
import { cx } from '../ui/cx';
import ui from '../ui/ui.module.css';
import { hasMovingParts, type Scheme, PreviewTree } from '@liminal-hq/installer-renderer';
import type { Run } from './useRun';
import styles from './RunStage.module.css';

export const SPEEDS = [0.5, 1, 2, 4] as const;

/** Play, Pause, Reset and the speed of the copy animation. */
export function PlaybackBar({ run, step }: { run: Run; step: PreviewStep | undefined }) {
	const p = run.playback;
	const live = step?.layout_tree ? hasMovingParts(step.layout_tree) : false;
	return (
		<div className={styles.bar} role="group" aria-label={t('preview.playback')}>
			<button
				type="button"
				className={cx(ui.btn, ui.primary)}
				disabled={!live}
				aria-pressed={p.playing}
				onClick={() => (p.playing ? p.pause() : p.play())}
			>
				{p.playing ? t('preview.pause') : t('preview.play')}
			</button>
			<button type="button" className={ui.btn} disabled={!live} onClick={p.reset}>
				{t('preview.resetMeters')}
			</button>
			<label className={cx(ui.field, ui.inline)}>
				<span className={ui.fieldLabel}>{t('preview.speed')}</span>
				<select
					value={p.speed}
					aria-label={t('preview.speed')}
					onChange={(e) => p.setSpeed(Number(e.target.value))}
				>
					{SPEEDS.map((s) => (
						<option key={s} value={s}>
							{`${s}×`}
						</option>
					))}
				</select>
			</label>
			<button type="button" className={cx(ui.btn, ui.ghost)} onClick={run.resetChoices}>
				{t('preview.resetChoices')}
			</button>
		</div>
	);
}

/** The step's screen drawn from the run's state; null when the step has none. */
export function RunStage({
	pv,
	step,
	run,
	components,
	renderer,
	theme,
	fit,
	scheme,
}: {
	pv: Preview;
	step: PreviewStep;
	run: Run;
	components: ComponentInfo[];
	renderer: string;
	theme: string | null | undefined;
	fit?: 'width' | 'contain';
	scheme?: Scheme;
}) {
	if (!step.layout_tree) return null;
	return (
		<PreviewTree
			root={step.layout_tree}
			options={{
				renderer,
				theme: theme ?? pv.theme,
				vars: run.vars,
				state: run.state,
				components,
				live: run.live,
				fit,
				scheme,
				interact: { mode: 'run', onChange: run.onChange, onAction: run.onAction },
			}}
		/>
	);
}
