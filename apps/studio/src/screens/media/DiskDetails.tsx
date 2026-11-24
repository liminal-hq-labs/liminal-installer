// The details of the row picked in the disk map: sizes, the pack it is in, who uses it and what its switch would do.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Fragment, type ReactNode } from 'react';
import { t } from '../../i18n/messages';
import { formatBytes, type TreeRow } from '../../lib/media';
import type { ComponentSize, MediaPlan } from '../../lib/types';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import ui from '../../ui/ui.module.css';
import styles from '../MediaScreen.module.css';
import { toggleNote } from './toggleNote';
import { targetName, targetNames } from './targets';

const kib = (bytes: number): string => t('media.units.kib', { count: bytes / 1024 });

function Row({ term, children }: { term: string; children: ReactNode }) {
	return (
		<>
			<dt>{term}</dt>
			<dd>{children}</dd>
		</>
	);
}

/** What the row's component's switch would change, as the Sizes tab words it. */
function Switch({ component }: { component: ComponentSize | undefined }) {
	if (!component || component.required) return null;
	return (
		<Row term={t('media.details.component')}>
			{component.name}
			<br />
			<span className={cx(text.small, component.fits_if_toggled ? text.muted : text.err)}>
				{toggleNote(component)}
			</span>
		</Row>
	);
}

export function DiskDetails({ plan, row }: { plan: MediaPlan; row: TreeRow | null }) {
	const component = (id: string | undefined) => plan.components.find((c) => c.id === id);
	let body: ReactNode = <p className={text.muted}>{t('media.details.empty')}</p>;
	let title = t('media.details.title');
	if (row?.kind === 'volume') {
		title = row.volume.label;
		body = (
			<dl className={styles.facts}>
				<Row term={t('media.details.capacity')}>{formatBytes(row.volume.capacity)}</Row>
				<Row term={t('media.meter.used')}>{formatBytes(row.volume.used)}</Row>
				<Row term={t('media.meter.free')}>{formatBytes(row.volume.free)}</Row>
				{row.volume.serial ? <Row term={t('media.details.serial')}>{row.volume.serial}</Row> : null}
			</dl>
		);
	} else if (row?.kind === 'pack' && row.packEntry) {
		const p = row.packEntry;
		const pack = row.entry!.pack;
		title = p.name;
		body = (
			<dl className={styles.facts}>
				<Row term={t('media.details.stored')}>
					<span className={text.mono}>{p.stored}</span>
				</Row>
				<Row term={t('media.details.raw')}>{formatBytes(p.raw)}</Row>
				<Row term={t('media.details.packed')}>{formatBytes(p.packed)}</Row>
				<Row term={t('media.details.pack')}>
					{row.entry!.name}
					{pack ? ` · ${kib(pack.chunk)}` : ''}
				</Row>
				<Row term={t('media.details.targets')}>{targetNames(p.targets)}</Row>
				{p.dest.length > 0 ? (
					<Row term={t('media.details.installed')}>
						{p.dest.map((d) => (
							<Fragment key={d.target}>
								<span className={text.small}>{targetName(d.target)}</span>{' '}
								<span className={text.mono}>{d.path}</span>
								<br />
							</Fragment>
						))}
					</Row>
				) : null}
				<Switch component={component(p.component)} />
			</dl>
		);
	} else if (row?.entry) {
		const e = row.entry;
		title = e.name;
		const packed = e.entries.reduce((n, x) => n + x.packed, 0);
		body = (
			<dl className={styles.facts}>
				<Row term={t('media.details.raw')}>{formatBytes(e.raw)}</Row>
				<Row term={t('media.details.onDisk')}>{formatBytes(e.on_disk)}</Row>
				{e.pack ? (
					<>
						<Row term={t('media.details.pack')}>
							{t(e.pack.which === 'common' ? 'media.pack.common' : 'media.pack.own')}
						</Row>
						<Row term={t('media.details.chunk')}>{kib(e.pack.chunk)}</Row>
						{e.pack.set_id ? (
							<Row term={t('media.details.setId')}>
								<span className={text.mono}>{e.pack.set_id}</span>
							</Row>
						) : null}
						<Row term={t('media.details.entries')}>{e.entries.length}</Row>
						<Row term={t('media.details.overhead')}>{formatBytes(Math.max(0, e.raw - packed))}</Row>
					</>
				) : null}
				{e.targets.length > 0 ? (
					<Row term={t('media.details.targets')}>{targetNames(e.targets)}</Row>
				) : null}
				<Switch component={component(e.component)} />
			</dl>
		);
	}
	return (
		<aside className={cx(ui.card, styles.details)} aria-label={t('media.details.title')}>
			<h3 data-testid="details-title">{title}</h3>
			{body}
		</aside>
	);
}
