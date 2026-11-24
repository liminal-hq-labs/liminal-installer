// The picked medium: its heading, how full it is, what is wrong with it and the tabs that show what is on it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tn } from '../../i18n/messages';
import type { MediaInfo, MediaPlan } from '../../lib/types';
import { kindMessage, type MediaTab } from '../../stores/media';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import ui from '../../ui/ui.module.css';
import styles from '../MediaScreen.module.css';
import { CapacityMeter } from './CapacityMeter';
import { DiskMap } from './DiskMap';
import { MediaGlyph } from './MediaGlyph';
import { SizesTable } from './SizesTable';
import { panelId, tabId, TabBar, type TabSpec } from './TabBar';

const TABS: TabSpec[] = [
	{ id: 'map', label: 'media.tab.map' },
	{ id: 'sizes', label: 'media.tab.sizes' },
];

export interface MediaDetailProps {
	info: MediaInfo;
	plan: MediaPlan | undefined;
	tab: MediaTab;
	busy: boolean;
	onTab(tab: MediaTab): void;
	onRemove(): void;
}

export function MediaDetail({ info, plan, tab, busy, onTab, onRemove }: MediaDetailProps) {
	const only = plan && plan.volumes.length === 1 ? plan.volumes[0]! : null;
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
					{only ? (
						<div className={ui.card}>
							<CapacityMeter
								kind={plan.kind}
								volume={only}
								reserve={plan.reserve}
								{...(plan.kind === 'floppy-1440' ? { withoutSharing: plan.without_sharing } : {})}
							/>
						</div>
					) : null}
					{plan.warnings.length > 0 ? (
						<div className={ui.card}>
							<h3>{t('media.detail.warnings')}</h3>
							<ul className={ui.problems}>
								{plan.warnings.map((w, i) => (
									<li key={i} className={w.severity === 'error' ? ui.error : undefined}>
										<span className={cx(text.mono, text.small)}>{w.code}</span> {w.message}
									</li>
								))}
							</ul>
						</div>
					) : null}
					<TabBar tabs={TABS} current={tab} onSelect={onTab} />
					<div role="tabpanel" id={panelId(tab)} aria-labelledby={tabId(tab)}>
						{tab === 'sizes' ? <SizesTable plan={plan} /> : <DiskMap key={info.id} plan={plan} />}
					</div>
				</>
			) : null}
		</section>
	);
}
