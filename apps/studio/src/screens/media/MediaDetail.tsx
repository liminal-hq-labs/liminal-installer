// The picked medium: its heading, how full it is and what is wrong with it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tn } from '../../i18n/messages';
import { formatBytes } from '../../lib/media';
import type { MediaInfo, MediaPlan } from '../../lib/types';
import { kindMessage } from '../../stores/media';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import ui from '../../ui/ui.module.css';
import styles from '../MediaScreen.module.css';
import { MediaGlyph } from './MediaGlyph';

export interface MediaDetailProps {
	info: MediaInfo;
	plan: MediaPlan | undefined;
	busy: boolean;
	onRemove(): void;
}

export function MediaDetail({ info, plan, busy, onRemove }: MediaDetailProps) {
	return (
		<section className={styles.detail} aria-labelledby="media-detail-h">
			<div className={styles.detailHead}>
				<span className={styles.headGlyph}>
					<MediaGlyph kind={info.kind} size={32} />
				</span>
				<div className={styles.headText}>
					<h2 id="media-detail-h">{info.id}</h2>
					<p className={cx(text.muted, text.small)}>
						{t('media.detail.summary', {
							kind: t(kindMessage(info.kind)),
							label: info.label,
							targets: tn('media.detail.targets', info.targets.length),
						})}
					</p>
				</div>
				<button type="button" className={cx(ui.btn, ui.danger)} disabled={busy} onClick={onRemove}>
					{t('media.remove.button')}
				</button>
			</div>
			{plan ? (
				<>
					<div className={cx(ui.card, styles.capacity)} aria-label={t('media.detail.capacity')}>
						<dl className={styles.totals}>
							<div>
								<dt>{t('media.detail.total')}</dt>
								<dd>{formatBytes(plan.capacity)}</dd>
							</div>
							<div>
								<dt>{t('media.detail.used')}</dt>
								<dd>{formatBytes(plan.used)}</dd>
							</div>
							<div>
								<dt>{t('media.detail.free')}</dt>
								<dd>{formatBytes(plan.free)}</dd>
							</div>
						</dl>
					</div>
					<div className={ui.card}>
						<h3>{t('media.detail.warnings')}</h3>
						{plan.warnings.length === 0 ? (
							<p className={text.muted}>{t('media.detail.noWarnings')}</p>
						) : (
							<ul className={ui.problems}>
								{plan.warnings.map((w, i) => (
									<li key={i} className={w.severity === 'error' ? ui.error : undefined}>
										<span className={cx(text.mono, text.small)}>{w.code}</span> {w.message}
									</li>
								))}
							</ul>
						)}
					</div>
				</>
			) : null}
		</section>
	);
}
