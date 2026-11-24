// The Autorun tab: the AUTORUN.INF a disc carries and the target it opens.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../../i18n/messages';
import type { Change } from '../../lib/mediaEdit';
import type { MediaInfo, MediaPlan } from '../../lib/types';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import ui from '../../ui/ui.module.css';
import styles from '../MediaScreen.module.css';
import { targetName } from './targets';

export interface AutorunPreviewProps {
	info: MediaInfo;
	plan: MediaPlan;
	busy: boolean;
	onChange(change: Change): void;
}

/** The targets AUTORUN.INF could open: the 32-bit Windows ones on this medium. */
const opens = (info: MediaInfo) => info.targets.filter((x) => x.target.startsWith('windows'));

export function AutorunPreview({ info, plan, busy, onChange }: AutorunPreviewProps) {
	const choices = opens(info);
	const note =
		info.kind !== 'cdrom'
			? t('media.autorun.floppy')
			: choices.length === 0
				? t('media.autorun.noTarget')
				: null;
	return (
		<section className={ui.card} aria-labelledby="autorun-h">
			<h3 id="autorun-h">{t('media.autorun.title')}</h3>
			{note ? <p className={text.muted}>{note}</p> : null}
			<label className={styles.fieldBlock}>
				<span>{t('media.autorun.target')}</span>
				<select
					className={styles.input}
					value={info.autorun ?? ''}
					disabled={busy || note !== null}
					onChange={(e) =>
						onChange({
							edit: { autorun: e.target.value },
							label: t('history.mediaAutorun', { id: info.id }),
						})
					}
				>
					<option value="">{t('media.autorun.none')}</option>
					{choices.map((c) => (
						<option key={c.target} value={c.target}>
							{targetName(c.target)}
						</option>
					))}
				</select>
			</label>
			{plan.autorun_text ? (
				<>
					<p className={cx(text.small, text.muted)}>{t('media.autorun.text')}</p>
					<pre className={styles.readme} tabIndex={0} aria-label={t('media.autorun.title')}>
						{plan.autorun_text.replace(/\r/g, '')}
					</pre>
				</>
			) : null}
		</section>
	);
}
