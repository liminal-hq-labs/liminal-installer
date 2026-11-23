// The Dry-run screen: the plan, the problems and each step's screen in order.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tn } from '../i18n/messages';
import { formatOp, screenshotSrc } from '../lib/format';
import { FullScreenBody, type FullScreenBodyProps } from '../preview/FullScreenBody';
import { PlaybackBar, RunStage } from '../preview/RunStage';
import { useRun } from '../preview/useRun';
import { useEffect, useState } from 'react';
import {
	stepFor,
	TerminalPreview,
	TerminalViewSwitch,
	type TerminalView,
} from '../preview/TerminalPreview';
import { lookFor } from '../preview/model';
import preview from '../preview/preview.module.css';
import { useChromeState, usePreview, useProject, useStudio } from '../app/StudioContext';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import styles from './PreviewScreen.module.css';

type OverlayBody = Omit<FullScreenBodyProps, 'fullscreen' | 'onToggleFullscreen' | 'onClose'>;

/**
 * The browser tab's full-screen preview: the page itself is the only window, so the preview fills the
 * viewport and asks the browser to go full screen too. Escape closes it. The desktop shell opens a
 * window of its own instead.
 */
function FullscreenOverlay({ body, onClose }: { body: OverlayBody; onClose: () => void }) {
	const [native, setNative] = useState(false);
	useEffect(() => {
		const onKey = (e: KeyboardEvent) => {
			if (e.key === 'Escape' && (e.target as HTMLElement | null)?.tagName !== 'SELECT') onClose();
		};
		const onChange = () => setNative(document.fullscreenElement !== null);
		window.addEventListener('keydown', onKey);
		document.addEventListener('fullscreenchange', onChange);
		return () => {
			window.removeEventListener('keydown', onKey);
			document.removeEventListener('fullscreenchange', onChange);
			if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined);
		};
	}, [onClose]);
	return (
		<div className={styles.overlay} role="dialog" aria-label={t('preview.closeFullScreen')}>
			<FullScreenBody
				{...body}
				fullscreen={native}
				onToggleFullscreen={() => {
					if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined);
					else void document.documentElement.requestFullscreen?.().catch(() => undefined);
				}}
				onClose={onClose}
			/>
		</div>
	);
}

export function PreviewScreen() {
	const studio = useStudio();
	const summary = useProject((s) => s.summary)!;
	const { settings } = useChromeState();
	const previewTarget = usePreview((s) => s.previewTarget);
	const pv = usePreview((s) => s.preview);
	const previewStep = usePreview((s) => s.previewStep);
	const targets = summary.targets;
	const shown = pv?.steps[previewStep];
	const [view, setView] = useState<TerminalView>('rich');
	const setStep = (i: number) => studio.preview.setState({ previewStep: i });
	const run = useRun(pv, summary.components, previewStep, setStep);
	const fullscreen = usePreview((s) => s.fullscreen);
	const closeFullscreen = () => studio.preview.setState({ fullscreen: false });
	const go = (delta: -1 | 1) => run.onAction(delta < 0 ? 'back' : 'next');
	const targetInfo = targets.find((x) => x.full_id === previewTarget);
	const stale = Boolean(pv && pv.target !== previewTarget);
	const skippedCount = pv?.steps.filter((x) => x.skipped).length ?? 0;
	const renderer = pv?.renderer ?? targetInfo?.renderer ?? 'canvas';
	const isTui = renderer.startsWith('tui');
	const real = isTui && view === 'terminal' && pv && shown?.layout && !shown.skipped;

	return (
		<div className={ui.screen}>
			<div className={ui.screenHead}>
				<div>
					<h1>{t('preview.title')}</h1>
					<p className={text.muted}>{t('preview.intro')}</p>
				</div>
			</div>
			<div className={ui.toolbar}>
				<select
					aria-label={t('preview.target')}
					value={previewTarget}
					onChange={(e) => {
						const target = e.target.value;
						studio.preview.setState({ previewTarget: target });
						void studio.run(() => studio.runPreview(target));
					}}
				>
					{targets.map((x) => (
						<option key={x.full_id} value={x.full_id}>
							{`${x.id} · ${x.renderer}`}
						</option>
					))}
				</select>
				<button
					type="button"
					className={cx(ui.btn, ui.primary)}
					onClick={() => void studio.run(() => studio.runPreview(previewTarget))}
				>
					{t('preview.run')}
				</button>
				{isTui ? <TerminalViewSwitch value={view} onChange={setView} /> : null}
				{pv ? (
					<span className={stale ? cx(ui.chip, styles.stale) : cx(text.dim, text.small)}>
						{stale
							? t('preview.stale', { target: pv.target })
							: t('preview.showing', {
									target: pv.target,
									theme: pv.theme ? ` · ${t('preview.theme', { theme: pv.theme })}` : '',
									renderer: pv.renderer ?? '',
									look: lookFor(renderer, pv.theme),
								})}
					</span>
				) : null}
			</div>
			{!pv ? (
				<p className={text.muted}>{t('preview.runToSee')}</p>
			) : (
				<div className={styles.previewGrid}>
					<section className={ui.card} aria-label={t('preview.steps')}>
						<h2>{t('preview.steps')}</h2>
						<ol className={styles.steps}>
							{pv.steps.map((st, i) =>
								st.skipped && !settings.showSkippedSteps ? null : (
									<li
										key={`${st.id}-${i}`}
										className={cx(i === previewStep && styles.active, st.skipped && styles.skipped)}
									>
										<button
											type="button"
											className={ui.linklike}
											disabled={st.skipped}
											onClick={() => setStep(i)}
										>
											{st.id}
										</button>
										<span className={cx(text.dim, text.small)}>
											{` ${st.kind}${st.skipped ? ` · ${t('preview.skippedForTarget')}` : ''}`}
										</span>
									</li>
								),
							)}
						</ol>
					</section>
					<section className={ui.card} aria-label={t('preview.screen')}>
						<div className={ui.toolbar}>
							<button type="button" className={ui.btn} onClick={() => go(-1)}>
								{t('preview.back')}
							</button>
							<strong>{shown?.id ?? ''}</strong>
							<button type="button" className={ui.btn} onClick={() => go(1)}>
								{t('preview.next')}
							</button>
							<button
								type="button"
								className={ui.btn}
								onClick={() =>
									void studio.run(() => studio.app().dispatch('view.fullScreenPreview'))
								}
							>
								{t('preview.fullScreen')}
							</button>
						</div>
						{real ? (
							<TerminalPreview
								api={studio.api}
								target={pv.target}
								page={shown.id}
								state={shown.state}
								label={t('terminal.label', { id: shown.id })}
								onOutcome={(o) => {
									const step = stepFor(o);
									if (step === 'cancel') setStep(0);
									else if (step) go(step === 'back' ? -1 : 1);
								}}
							/>
						) : pv && shown?.layout_tree ? (
							<>
								<div
									className={preview.host}
									aria-label={t('preview.screenOfStep', { id: shown.id })}
								>
									<RunStage
										pv={pv}
										step={shown}
										run={run}
										components={summary.components}
										renderer={renderer}
										theme={pv.theme ?? targetInfo?.theme}
									/>
								</div>
								<PlaybackBar run={run} step={shown} />
							</>
						) : (
							<>
								<div className={preview.host} aria-label={t('preview.screen')} />
								<p className={text.muted}>{shown ? t('preview.noScreen') : ''}</p>
							</>
						)}
						{skippedCount > 0 ? (
							<p className={cx(text.dim, text.small)}>{tn('preview.skippedNote', skippedCount)}</p>
						) : null}
						{shown?.screenshot ? (
							<details className={styles.textVersion}>
								<summary>{t('preview.textVersion')}</summary>
								<img
									className={styles.shot}
									src={screenshotSrc(shown.screenshot)}
									alt={t('preview.textVersionOfStep', { id: shown.id })}
								/>
							</details>
						) : null}
					</section>
					<section className={cx(ui.card, styles.plan)} aria-label={t('preview.plan')}>
						<h2>{t('preview.planOps', { count: pv.plan.ops.length })}</h2>
						<p className={cx(text.dim, text.small)} data-testid="plan-summary">
							{run.vars['plan.summary']}
						</p>
						<p className={cx(text.dim, text.small)} data-testid="space-summary">
							{t('preview.spaceRequired', {
								size: run.vars['install.size'] ?? '',
								after: run.vars['system.free_space_after'] ?? '',
							})}
						</p>
						{run.derived.fits ? null : (
							<p className={text.err} role="alert">
								{t('preview.notEnoughSpace')}
							</p>
						)}
						<h3>{t('preview.selectedComponents', { count: run.selected.length })}</h3>
						<ul className={styles.ops}>
							{summary.components
								.filter((c) => run.selected.includes(c.id))
								.map((c) => (
									<li key={c.id}>
										<code className={styles.opName}>{c.id}</code>{' '}
										<span className={cx(text.mono, text.small)}>{c.files}</span>
									</li>
								))}
						</ul>
						<ol className={styles.ops}>
							{pv.plan.ops.map((op, i) => {
								const f = formatOp(op);
								return (
									<li key={i}>
										<code className={styles.opName}>{f.name}</code>{' '}
										<span className={cx(text.mono, text.small)}>{f.detail}</span>
									</li>
								);
							})}
						</ol>
						<h2>{t('preview.problemsCount', { count: pv.problems.length })}</h2>
						{pv.problems.length === 0 ? (
							<p className={text.ok}>{t('preview.none')}</p>
						) : (
							<ul className={ui.problems}>
								{pv.problems.map((p, i) => (
									<li key={i} className={p.severity === 'error' ? ui.error : undefined}>
										<span className={cx(text.mono, text.small)}>{p.scope}</span> {p.message}
									</li>
								))}
							</ul>
						)}
					</section>
				</div>
			)}
			{fullscreen ? (
				<FullscreenOverlay
					onClose={closeFullscreen}
					body={{
						pv,
						run,
						components: summary.components,
						targets,
						target: previewTarget,
						index: previewStep,
						setIndex: setStep,
						onTarget: (target) => {
							studio.preview.setState({ previewTarget: target });
							void studio.run(() => studio.runPreview(target));
						},
					}}
				/>
			) : null}
		</div>
	);
}
