// The reserve a medium keeps free, and what happens when its files do not fit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../../i18n/messages';
import type { Change } from '../../lib/mediaEdit';
import type { MediaInfo, Overflow } from '../../lib/types';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import ui from '../../ui/ui.module.css';
import styles from '../MediaScreen.module.css';
import { useDraft } from './useDraft';

export interface SpaceCardProps {
	info: MediaInfo;
	busy: boolean;
	onChange(change: Change): void;
}

const KIB = 1024;

export function SpaceCard({ info, busy, onChange }: SpaceCardProps) {
	const reserve = useDraft(
		String(Math.round(info.reserve / KIB)),
		(v) => (/^\d{1,6}$/.test(v) ? null : t('media.space.reserveBad')),
		(v) =>
			onChange({
				edit: { reserve: Number(v) * KIB },
				label: t('history.mediaReserve', { id: info.id }),
			}),
	);
	const cd = info.kind === 'cdrom';
	return (
		<section className={ui.card} aria-labelledby="space-h">
			<h3 id="space-h">{t('media.space.title')}</h3>
			<div className={styles.fieldBlock}>
				<label htmlFor="media-reserve">{t('media.space.reserve')}</label>
				<input
					id="media-reserve"
					type="text"
					inputMode="numeric"
					className={cx(styles.input, text.mono, styles.narrow)}
					aria-describedby="media-reserve-help"
					disabled={busy}
					{...reserve.props}
				/>
				<p id="media-reserve-help" className={cx(text.small, reserve.error ? text.err : text.dim)}>
					{reserve.error ?? t('media.space.reserveHelp')}
				</p>
			</div>
			<div className={styles.fieldBlock}>
				<label htmlFor="media-overflow">{t('media.space.overflow')}</label>
				<select
					id="media-overflow"
					className={styles.input}
					value={cd ? 'error' : info.overflow}
					disabled={busy || cd}
					onChange={(e) =>
						onChange({
							edit: { overflow: e.target.value as Overflow },
							label: t('history.mediaOverflow', { id: info.id }),
						})
					}
				>
					<option value="error">{t('media.space.error')}</option>
					<option value="span">{t('media.space.span')}</option>
				</select>
				{cd ? <p className={cx(text.small, text.dim)}>{t('media.space.cdNote')}</p> : null}
			</div>
		</section>
	);
}
