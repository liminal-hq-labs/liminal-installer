// The list of the project's media: one row for each, with how it stands and how much room is left.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { KeyboardEvent } from 'react';
import { t, tn, type MessageId } from '../../i18n/messages';
import { fitOf, formatBytes, type Fit } from '../../lib/media';
import type { MediaInfo, MediaPlan } from '../../lib/types';
import { kindMessage } from '../../stores/media';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import ui from '../../ui/ui.module.css';
import styles from '../MediaScreen.module.css';
import { MediaGlyph } from './MediaGlyph';

const FIT_MESSAGE: Record<Fit, MessageId> = {
	ok: 'media.fit.ok',
	warning: 'media.fit.warning',
	error: 'media.fit.error',
};
const FIT_PILL: Record<Fit, string> = {
	ok: ui.pillOk!,
	warning: ui.pillWarning!,
	error: ui.pillError!,
};

export interface MediaListProps {
	items: MediaInfo[];
	plans: Record<string, MediaPlan>;
	selected: string | null;
	onSelect(id: string): void;
	/** Delete was pressed on a row: ask to remove it. */
	onRemove(id: string): void;
}

export function MediaList({ items, plans, selected, onSelect, onRemove }: MediaListProps) {
	const keys = (e: KeyboardEvent<HTMLElement>) => {
		const at = items.findIndex((m) => m.id === selected);
		const go = (i: number) => {
			const next = items[Math.max(0, Math.min(items.length - 1, i))];
			if (!next) return;
			e.preventDefault();
			onSelect(next.id);
			// The roving tab stop follows the selection, so focus goes to the row that now has it.
			queueMicrotask(() => document.getElementById(rowId(next.id))?.focus());
		};
		if (e.key === 'ArrowDown') go(at + 1);
		else if (e.key === 'ArrowUp') go(at - 1);
		else if (e.key === 'Home') go(0);
		else if (e.key === 'End') go(items.length - 1);
		else if (e.key === 'Delete' && selected) {
			e.preventDefault();
			onRemove(selected);
		}
	};
	return (
		<div
			role="listbox"
			aria-label={t('media.list.label')}
			className={styles.list}
			onKeyDown={keys}
			data-testid="media-list"
		>
			{items.map((m) => {
				const plan = plans[m.id];
				const fit = plan ? fitOf(plan) : 'ok';
				const on = m.id === selected;
				return (
					<div
						key={m.id}
						id={rowId(m.id)}
						role="option"
						aria-selected={on}
						tabIndex={on || (!selected && items[0] === m) ? 0 : -1}
						className={cx(styles.row, on && styles.rowOn)}
						onClick={() => onSelect(m.id)}
					>
						<span className={styles.rowGlyph}>
							<MediaGlyph kind={m.kind} />
						</span>
						<span className={styles.rowMain}>
							<span className={styles.rowTitle}>
								<strong>{m.id}</strong>
								<span className={cx(text.mono, text.dim)}>{m.label}</span>
							</span>
							<span className={cx(text.small, text.muted)}>{t(kindMessage(m.kind))}</span>
							{plan ? (
								<span className={cx(text.small, text.muted)}>
									{t('media.list.free', { free: formatBytes(plan.free) })}
									{plan.volumes.length > 1
										? ` · ${tn('media.list.disks', plan.volumes.length)}`
										: ''}
								</span>
							) : null}
							{plan && plan.warnings.length > 0 ? (
								<span className={cx(text.small, styles.rowWarn)}>
									{tn('media.list.warnings', plan.warnings.length)}
								</span>
							) : null}
						</span>
						<span className={cx(ui.pill, FIT_PILL[fit])}>{t(FIT_MESSAGE[fit])}</span>
					</div>
				);
			})}
		</div>
	);
}

/** The element id of a list row. */
export const rowId = (id: string): string => `media-row-${id}`;
