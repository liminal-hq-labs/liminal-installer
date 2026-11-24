// The Targets tab: which targets are on the medium, their folders and where each SETUP.EXE came from.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../../i18n/messages';
import { checkDir, formatBytes } from '../../lib/media';
import { dirChange, targetChange, type Change } from '../../lib/mediaEdit';
import type { MediaInfo, MediaPlan, MediaTargetPlan } from '../../lib/types';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import ui from '../../ui/ui.module.css';
import styles from '../MediaScreen.module.css';
import { targetName } from './targets';
import { useDraft } from './useDraft';

/** Why a folder cannot be used, in words, or null. The other targets' folders are the ones it may not repeat. */
function dirProblem(dir: string, others: readonly string[]): string | null {
	const problem = checkDir(dir);
	if (problem?.kind === 'notOneFolder') return t('media.dir.notOne');
	if (problem) return t('media.dir.not83', { dir });
	return others.some((o) => o.toUpperCase() === dir.toUpperCase())
		? t('media.dir.duplicate')
		: null;
}

function FolderField({
	row,
	others,
	disabled,
	onCommit,
}: {
	row: MediaTargetPlan;
	others: string[];
	disabled: boolean;
	onCommit(dir: string): void;
}) {
	const draft = useDraft(
		row.dir,
		(d) => dirProblem(d, others),
		onCommit,
		(v) => v.toUpperCase(),
	);
	const errorId = `dir-error-${row.target}`;
	return (
		<div className={styles.fieldCell}>
			<input
				type="text"
				className={cx(styles.input, text.mono)}
				aria-label={t('media.targets.folderLabel', { target: targetName(row.target) })}
				placeholder={t('media.targets.root')}
				disabled={disabled}
				spellCheck={false}
				autoComplete="off"
				aria-describedby={draft.error ? errorId : undefined}
				{...draft.props}
			/>
			{draft.error ? (
				<span id={errorId} className={cx(text.small, text.err)} role="alert">
					{draft.error}
				</span>
			) : null}
		</div>
	);
}

export interface TargetsCardProps {
	info: MediaInfo;
	plan: MediaPlan;
	busy: boolean;
	onChange(change: Change): void;
}

export function TargetsCard({ info, plan, busy, onChange }: TargetsCardProps) {
	return (
		<section className={ui.card} aria-labelledby="targets-h">
			<h3 id="targets-h">{t('media.targets.title')}</h3>
			<p className={cx(text.muted, text.small)}>{t('media.targets.intro')}</p>
			<div className={styles.scroll}>
				<table className={styles.table}>
					<thead>
						<tr>
							<th scope="col">{t('media.col.target')}</th>
							<th scope="col">{t('media.col.folder')}</th>
							<th scope="col">{t('media.col.setup')}</th>
							<th scope="col" className={styles.num}>
								{t('media.details.onDisk')}
							</th>
						</tr>
					</thead>
					<tbody>
						{plan.targets.map((row) => {
							const name = targetName(row.target);
							const others = info.targets.filter((x) => x.target !== row.target).map((x) => x.dir);
							return (
								<tr key={row.target}>
									<th scope="row">
										<label className={styles.inlineCheck}>
											<input
												type="checkbox"
												checked={row.on_medium}
												disabled={busy}
												aria-label={t('media.targets.on', { target: name })}
												onChange={(e) =>
													onChange(targetChange(info, plan, row.target, name, e.target.checked))
												}
											/>
											{name}
										</label>
									</th>
									<td>
										<FolderField
											row={row}
											others={others}
											disabled={busy || !row.on_medium}
											onCommit={(dir) => onChange(dirChange(info, row.target, name, dir))}
										/>
									</td>
									<td>
										{row.runtime ? (
											<span className={text.small}>
												{t('media.targets.found', { path: row.runtime })}
											</span>
										) : (
											<span className={cx(text.small, text.err)}>
												{t('media.targets.estimated')}
											</span>
										)}
									</td>
									<td className={styles.num}>{formatBytes(row.setup_bytes)}</td>
								</tr>
							);
						})}
					</tbody>
				</table>
			</div>
			<p className={cx(text.dim, text.small)}>{t('media.dir.rule')}</p>
		</section>
	);
}
