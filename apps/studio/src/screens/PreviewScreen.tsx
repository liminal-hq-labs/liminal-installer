// The Dry-run screen: the plan, the problems and each step's screen in order.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tn } from '../i18n/messages';
import { formatOp, screenshotSrc } from '../lib/format';
import { PreviewTree } from '../preview/PreviewTree';
import { lookFor } from '../preview/model';
import preview from '../preview/preview.module.css';
import { useChromeState, usePreview, useProject, useStudio } from '../app/StudioContext';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import styles from './PreviewScreen.module.css';

export function PreviewScreen() {
	const studio = useStudio();
	const summary = useProject((s) => s.summary)!;
	const { settings } = useChromeState();
	const previewTarget = usePreview((s) => s.previewTarget);
	const pv = usePreview((s) => s.preview);
	const previewStep = usePreview((s) => s.previewStep);
	const targets = summary.targets;
	const shown = pv?.steps[previewStep];
	const setStep = (i: number) => studio.preview.setState({ previewStep: i });
	const go = (delta: number) => {
		if (!pv) return;
		let i = previewStep;
		do i += delta;
		while (pv.steps[i]?.skipped);
		if (i >= 0 && i < pv.steps.length) setStep(i);
	};
	const targetInfo = targets.find((x) => x.full_id === previewTarget);
	const stale = Boolean(pv && pv.target !== previewTarget);
	const skippedCount = pv?.steps.filter((x) => x.skipped).length ?? 0;
	const renderer = pv?.renderer ?? targetInfo?.renderer ?? 'canvas';

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
						</div>
						{shown?.layout_tree ? (
							<div
								className={preview.host}
								aria-label={t('preview.screenOfStep', { id: shown.id })}
							>
								<PreviewTree
									root={shown.layout_tree}
									options={{
										renderer,
										theme: pv.theme ?? targetInfo?.theme,
										vars: pv.vars,
										state: shown.state,
										components: summary.components,
									}}
								/>
							</div>
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
		</div>
	);
}
