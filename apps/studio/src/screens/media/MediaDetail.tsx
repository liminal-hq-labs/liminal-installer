// The picked medium: its heading, how full it is, what is wrong with it and the tabs that show what is on it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tn, type MessageId } from '../../i18n/messages';
import { formatBytes } from '../../lib/media';
import { toggleChange, warningTab, type Change } from '../../lib/mediaEdit';
import type { MediaInfo, MediaPlan, MediaWarningCode } from '../../lib/types';
import { kindMessage, type MediaTab } from '../../stores/media';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import ui from '../../ui/ui.module.css';
import styles from '../MediaScreen.module.css';
import { CapacityMeter } from './CapacityMeter';
import { DiskMap } from './DiskMap';
import { MediaGlyph } from './MediaGlyph';
import { AutorunPreview } from './AutorunPreview';
import { LabelCard } from './LabelCard';
import { SizesTable } from './SizesTable';
import { SpaceCard } from './SpaceCard';
import { TargetsCard } from './TargetsCard';
import { panelId, tabId, TabBar, type TabSpec } from './TabBar';

const TABS: TabSpec[] = [
	{ id: 'map', label: 'media.tab.map' },
	{ id: 'sizes', label: 'media.tab.sizes' },
	{ id: 'targets', label: 'media.tab.targets' },
	{ id: 'label', label: 'media.tab.label' },
	{ id: 'autorun', label: 'media.tab.autorun' },
];

/** The short name of each kind of warning, in bold before its message. */
const WARNING_TITLE: Record<MediaWarningCode, MessageId> = {
	does_not_fit: 'media.warn.does_not_fit',
	below_reserve: 'media.warn.below_reserve',
	default_on_later_disk: 'media.warn.default_on_later_disk',
	win16_window: 'media.warn.win16_window',
	label: 'media.warn.label',
	dir_not_8_3: 'media.warn.dir_not_8_3',
	autorun_target: 'media.warn.autorun_target',
	runtime_estimated: 'media.warn.runtime_estimated',
	runtime_lacks: 'media.warn.runtime_lacks',
	name_83: 'media.warn.name_83',
};

export interface MediaDetailProps {
	info: MediaInfo;
	plan: MediaPlan | undefined;
	tab: MediaTab;
	busy: boolean;
	onTab(tab: MediaTab): void;
	onRemove(): void;
	/** Writes a change as one undoable edit. */
	onChange(change: Change): void;
	/** Asks for a file to use as the README. */
	onPickReadme(): void;
}

export function MediaDetail({
	info,
	plan,
	tab,
	busy,
	onTab,
	onRemove,
	onChange,
	onPickReadme,
}: MediaDetailProps) {
	/** Opens the tab that fixes a warning and moves focus to it. */
	const open = (next: MediaTab) => {
		onTab(next);
		queueMicrotask(() => document.getElementById(tabId(next))?.focus());
	};
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
								{plan.warnings.map((w, i) => {
									const link = warningTab(w.code);
									const tabSpec = TABS.find((x) => x.id === link);
									return (
										<li key={i} className={w.severity === 'error' ? ui.error : undefined}>
											<strong>{t(WARNING_TITLE[w.code])}</strong> {w.message}{' '}
											{link && tabSpec ? (
												<button type="button" className={ui.linklike} onClick={() => open(link)}>
													{t('media.warn.open', { tab: t(tabSpec.label) })}
												</button>
											) : null}
										</li>
									);
								})}
							</ul>
						</div>
					) : null}
					<TabBar tabs={TABS} current={tab} onSelect={onTab} />
					<div role="tabpanel" id={panelId(tab)} aria-labelledby={tabId(tab)}>
						{tab === 'sizes' ? (
							<div className={styles.sizes}>
								<p role="status" className={cx(text.small, text.muted)}>
									{t('media.sizes.freeNow', {
										free: formatBytes(plan.free),
										label: plan.label,
									})}
								</p>
								<SpaceCard info={info} busy={busy} onChange={onChange} />
								<SizesTable
									plan={plan}
									busy={busy}
									onToggle={(c, on) => onChange(toggleChange(info, c, on))}
								/>
							</div>
						) : tab === 'targets' ? (
							<TargetsCard info={info} plan={plan} busy={busy} onChange={onChange} />
						) : tab === 'label' ? (
							<LabelCard info={info} plan={plan} onChange={onChange} onPickReadme={onPickReadme} />
						) : tab === 'autorun' ? (
							<AutorunPreview info={info} plan={plan} busy={busy} onChange={onChange} />
						) : (
							<DiskMap key={info.id} plan={plan} />
						)}
					</div>
				</>
			) : null}
		</section>
	);
}
