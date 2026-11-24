// How full a disk is: a meter of VT323 block cells and the numbers it stands for.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../../i18n/messages';
import {
	formatBytes,
	meterCellCount,
	meterCells,
	meterParts,
	type MeterParts,
} from '../../lib/media';
import type { MediaKind, VolumePlan } from '../../lib/types';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import styles from '../MediaScreen.module.css';

/** The block that draws each part, in the order they run along the bar. Patterns as well as colour. */
const BLOCKS = { own: '█', common: '▓', reserve: '▒', free: '░' } as const;
const ORDER = ['own', 'common', 'reserve', 'free'] as const;
const PART_CLASS = {
	own: styles.cellOwn,
	common: styles.cellCommon,
	reserve: styles.cellReserve,
	free: styles.cellFree,
} as const;

/** A row of block cells, one span for each so that a narrow window wraps the bar rather than cutting it. Drawn for the eye only. */
function Cells({ parts, count, over }: { parts: MeterParts; count: number; over: boolean }) {
	const cells = meterCells(parts, count);
	return (
		<span className={cx(styles.cells, over && styles.cellsOver)} aria-hidden="true">
			{ORDER.flatMap((k) =>
				Array.from({ length: cells[k] }, (_, i) => (
					<span key={`${k}${i}`} className={PART_CLASS[k]}>
						{BLOCKS[k]}
					</span>
				)),
			)}
		</span>
	);
}

export interface CapacityMeterProps {
	kind: MediaKind;
	volume: VolumePlan;
	reserve: number;
	/** What the medium would need if no file were shared, for the second bar. Left out on a disk of a set. */
	withoutSharing?: number;
	/** Draw the numbers under the bar. A disk of a set draws them beside its heading instead. */
	totals?: boolean;
}

export function CapacityMeter({
	kind,
	volume,
	reserve,
	withoutSharing,
	totals = true,
}: CapacityMeterProps) {
	const parts = meterParts(volume, reserve);
	const count = meterCellCount(kind, volume.capacity);
	const over = volume.free < 0;
	const met = volume.free >= reserve;
	const spoken = over
		? t('media.meter.ariaOver', {
				used: formatBytes(volume.used),
				capacity: formatBytes(volume.capacity),
				over: formatBytes(-volume.free),
			})
		: t('media.meter.aria', {
				used: formatBytes(volume.used),
				capacity: formatBytes(volume.capacity),
				free: formatBytes(volume.free),
				reserve: t(met ? 'media.meter.reserveMet' : 'media.meter.reserveShort'),
			});
	return (
		<div className={styles.meter}>
			<div
				role="meter"
				aria-label={t('media.meter.label', { label: volume.label })}
				aria-valuemin={0}
				aria-valuemax={volume.capacity}
				aria-valuenow={Math.min(volume.used, volume.capacity)}
				aria-valuetext={spoken}
				className={styles.bar}
				data-testid="capacity-meter"
			>
				<Cells parts={parts} count={count} over={over} />
			</div>
			{totals ? (
				<dl className={styles.totals}>
					<div>
						<dt>
							<span className={cx(styles.key, styles.cellOwn)} aria-hidden="true">
								{BLOCKS.own}
							</span>
							{t('media.meter.used')}
						</dt>
						<dd>{formatBytes(volume.used)}</dd>
					</div>
					{volume.common > 0 ? (
						<div>
							<dt>
								<span className={cx(styles.key, styles.cellCommon)} aria-hidden="true">
									{BLOCKS.common}
								</span>
								{t('media.meter.common')}
							</dt>
							<dd>{formatBytes(volume.common)}</dd>
						</div>
					) : null}
					<div>
						<dt>
							<span className={cx(styles.key, styles.cellFree)} aria-hidden="true">
								{BLOCKS.free}
							</span>
							{t(over ? 'media.meter.over' : 'media.meter.free')}
						</dt>
						<dd className={over ? text.err : undefined}>{formatBytes(Math.abs(volume.free))}</dd>
					</div>
					{reserve > 0 ? (
						<div>
							<dt>
								<span className={cx(styles.key, styles.cellReserve)} aria-hidden="true">
									{BLOCKS.reserve}
								</span>
								{t('media.meter.reserve')}
							</dt>
							<dd className={met ? text.ok : text.err}>
								{formatBytes(reserve)}
								{' · '}
								{t(met ? 'media.meter.reserveMet' : 'media.meter.reserveShort')}
							</dd>
						</div>
					) : null}
				</dl>
			) : null}
			{withoutSharing !== undefined && withoutSharing > volume.used ? (
				<WithoutSharing needs={withoutSharing} volume={volume} kind={kind} />
			) : null}
		</div>
	);
}

/** The second, thin bar: the same disk if every target carried its own copy of each shared file. */
function WithoutSharing({
	needs,
	volume,
	kind,
}: {
	needs: number;
	volume: VolumePlan;
	kind: MediaKind;
}) {
	const count = meterCellCount(kind, volume.capacity);
	const over = needs - volume.capacity;
	const parts: MeterParts = {
		own: Math.min(needs, volume.capacity),
		common: 0,
		reserve: 0,
		free: Math.max(0, volume.capacity - needs),
		over: Math.max(0, over),
	};
	return (
		<div className={styles.without}>
			<div className={styles.thin}>
				<Cells parts={parts} count={count} over={over > 0} />
			</div>
			<p className={cx(text.small, over > 0 ? text.err : text.muted)}>
				{over > 0
					? t('media.meter.withoutSharingOver', {
							needs: formatBytes(needs),
							over: formatBytes(over),
						})
					: t('media.meter.withoutSharing', {
							needs: formatBytes(needs),
							spare: formatBytes(-over),
						})}
			</p>
		</div>
	);
}
