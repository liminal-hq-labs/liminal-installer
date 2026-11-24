// The Media screen: the disks and discs a project ships on, how full each is and what is on it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import type { Change } from '../lib/mediaEdit';
import { useMedia, useStudio } from '../app/StudioContext';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import styles from './MediaScreen.module.css';
import { MediaDetail } from './media/MediaDetail';
import { MediaList } from './media/MediaList';

export function MediaScreen() {
	const studio = useStudio();
	const items = useMedia((s) => s.items);
	const plans = useMedia((s) => s.plans);
	const selected = useMedia((s) => s.selected);
	const loaded = useMedia((s) => s.loaded);
	const loading = useMedia((s) => s.loading);
	const tab = useMedia((s) => s.tab);
	const busy = useMedia((s) => s.busy);
	const error = useMedia((s) => s.error);
	const current = items.find((m) => m.id === selected);

	const change = (id: string, c: Change) => void studio.media.edit(id, c.edit, c.label);
	const pickReadme = async (id: string) => {
		const file = await studio.api.pickReadme(studio.project.getState().summary?.root);
		if (file) await studio.media.edit(id, { readme: file }, t('history.mediaReadme', { id }));
	};
	const remove = (id: string) => void studio.run(() => studio.removeMedia(id));
	return (
		<div className={ui.screen}>
			<div className={ui.screenHead}>
				<div>
					<h1>{t('media.title')}</h1>
					<p className={text.muted}>{t('media.intro')}</p>
				</div>
				<div className={ui.toolbar}>
					<button
						type="button"
						className={ui.btn}
						disabled={busy || !loaded}
						onClick={() => void studio.media.add('floppy-1440')}
					>
						{t('media.add.floppy')}
					</button>
					<button
						type="button"
						className={ui.btn}
						disabled={busy || !loaded}
						onClick={() => void studio.media.add('cdrom')}
					>
						{t('media.add.cd')}
					</button>
				</div>
			</div>
			{error ? (
				<p className={text.err} role="alert">
					{error}
				</p>
			) : null}
			{!loaded ? (
				loading ? (
					<p className={text.muted}>{t('media.loading')}</p>
				) : null
			) : items.length === 0 ? (
				<section className={cx(ui.card, styles.empty)} aria-labelledby="media-empty-h">
					<h2 id="media-empty-h">{t('media.empty.title')}</h2>
					<p className={text.muted}>{t('media.empty.help')}</p>
				</section>
			) : (
				<div className={styles.body}>
					<MediaList
						items={items}
						plans={plans}
						selected={selected}
						onSelect={(id) => studio.media.select(id)}
						onRemove={remove}
					/>
					{current ? (
						<MediaDetail
							info={current}
							plan={plans[current.id]}
							tab={tab}
							busy={busy}
							onTab={(next) => studio.media.setTab(next)}
							onRemove={() => remove(current.id)}
							onChange={(c) => change(current.id, c)}
							onPickReadme={() => void studio.run(() => pickReadme(current.id))}
						/>
					) : (
						<p className={text.muted}>{t('media.detail.pick')}</p>
					)}
				</div>
			)}
		</div>
	);
}
