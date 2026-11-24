// The Sizes tab: what each component and each file takes, and what leaving a component off would change.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useState } from 'react';
import { t, type MessageId } from '../../i18n/messages';
import { formatBytes, sortedBy, type SortDir } from '../../lib/media';
import type { ComponentSize, FileSize, MediaPlan } from '../../lib/types';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import ui from '../../ui/ui.module.css';
import styles from '../MediaScreen.module.css';
import { toggleNote } from './toggleNote';
import { targetNames } from './targets';

const DASH = '—';

interface Column<T> {
	id: string;
	label: MessageId;
	value(row: T): string | number;
	numeric?: boolean;
}

interface Sort {
	column: string;
	dir: SortDir;
}

/** A column heading that sorts its table: a button inside a `th` that says which way it is sorted. */
function Head<T>({
	column,
	sort,
	onSort,
}: {
	column: Column<T>;
	sort: Sort | null;
	onSort(column: string): void;
}) {
	const on = sort?.column === column.id;
	return (
		<th scope="col" aria-sort={on ? sort.dir : 'none'} className={cx(column.numeric && styles.num)}>
			<button type="button" className={styles.sortBtn} onClick={() => onSort(column.id)}>
				{t(column.label)}
				<span aria-hidden="true" className={styles.sortMark}>
					{on ? (sort.dir === 'ascending' ? ' ▲' : ' ▼') : ''}
				</span>
			</button>
		</th>
	);
}

/** Clicking a heading sorts by it, ascending first and then the other way. */
function useSort(initial: Sort | null = null): [Sort | null, (column: string) => void] {
	const [sort, set] = useState<Sort | null>(initial);
	return [
		sort,
		(column) =>
			set((s) =>
				s?.column === column
					? { column, dir: s.dir === 'ascending' ? 'descending' : 'ascending' }
					: { column, dir: 'ascending' },
			),
	];
}

const componentColumns: Column<ComponentSize>[] = [
	{ id: 'name', label: 'media.col.component', value: (c) => c.name },
	{
		id: 'onMedium',
		label: 'media.col.onMedium',
		value: (c) => (c.required ? 0 : c.on_medium ? 1 : 2),
	},
	{ id: 'raw', label: 'media.col.raw', value: (c) => c.raw, numeric: true },
	{ id: 'packed', label: 'media.col.packed', value: (c) => c.packed, numeric: true },
	{ id: 'pack', label: 'media.col.pack', value: (c) => c.pack },
	{ id: 'targets', label: 'media.col.targets', value: (c) => targetNames(c.targets) },
	{ id: 'disk', label: 'media.col.disk', value: (c) => c.volume, numeric: true },
	{
		id: 'change',
		label: 'media.col.change',
		value: (c) => (c.required ? 0 : c.delta_if_toggled),
		numeric: true,
	},
];

const fileColumns: Column<FileSize>[] = [
	{ id: 'path', label: 'media.col.file', value: (f) => f.path },
	{ id: 'component', label: 'media.col.component', value: (f) => f.component },
	{ id: 'raw', label: 'media.col.raw', value: (f) => f.raw, numeric: true },
	{ id: 'packed', label: 'media.col.packed', value: (f) => f.packed, numeric: true },
	{ id: 'pack', label: 'media.col.pack', value: (f) => f.pack },
	{ id: 'targets', label: 'media.col.targets', value: (f) => targetNames(f.targets) },
	{ id: 'disk', label: 'media.col.disk', value: (f) => f.volume, numeric: true },
];

function onMediumText(c: ComponentSize): string {
	if (c.required) return t('media.onMedium.required');
	return t(c.on_medium ? 'media.onMedium.on' : 'media.onMedium.off');
}

export interface SizesTableProps {
	plan: MediaPlan;
	busy: boolean;
	/** Puts a component on the medium or leaves it off. */
	onToggle(component: ComponentSize, on: boolean): void;
}

/** The switch of an optional component, and a word for a required one. */
function OnMedium({
	c,
	busy,
	onToggle,
}: {
	c: ComponentSize;
	busy: boolean;
	onToggle(on: boolean): void;
}) {
	if (c.required) return <>{t('media.onMedium.required')}</>;
	return (
		<button
			type="button"
			role="switch"
			aria-checked={c.on_medium}
			aria-label={t('media.switch.label', { component: c.name })}
			aria-describedby={`switch-note-${c.id}`}
			className={cx(styles.switch, c.on_medium && styles.switchOn)}
			disabled={busy}
			onClick={() => onToggle(!c.on_medium)}
		>
			<span aria-hidden="true">{c.on_medium ? '■' : '□'}</span> {onMediumText(c)}
		</button>
	);
}

export function SizesTable({ plan, busy, onToggle }: SizesTableProps) {
	const [cSort, sortComponents] = useSort();
	const [fSort, sortFiles] = useSort();
	const cCols = componentColumns;
	const components = cSort
		? sortedBy(plan.components, cCols.find((c) => c.id === cSort.column)!.value, cSort.dir)
		: plan.components;
	const files = fSort
		? sortedBy(plan.files, fileColumns.find((c) => c.id === fSort.column)!.value, fSort.dir)
		: plan.files;
	const nameOf = (id: string) => plan.components.find((c) => c.id === id)?.name ?? id;
	const disk = (n: number) => (n === 0 ? DASH : String(n));
	return (
		<div className={styles.sizes}>
			<section className={ui.card} aria-labelledby="sizes-components-h">
				<h3 id="sizes-components-h">{t('media.sizes.components')}</h3>
				<div className={styles.scroll}>
					<table className={styles.table}>
						<thead>
							<tr>
								{cCols.map((c) => (
									<Head key={c.id} column={c} sort={cSort} onSort={sortComponents} />
								))}
							</tr>
						</thead>
						<tbody>
							{components.map((c) => (
								<tr key={c.id}>
									<th scope="row">{c.name}</th>
									<td>
										<OnMedium c={c} busy={busy} onToggle={(on) => onToggle(c, on)} />
									</td>
									<td className={styles.num}>{c.on_medium ? formatBytes(c.raw) : DASH}</td>
									<td className={styles.num}>{c.on_medium ? formatBytes(c.packed) : DASH}</td>
									<td>{t(c.pack === 'common' ? 'media.pack.common' : 'media.pack.own')}</td>
									<td>{targetNames(c.targets)}</td>
									<td className={styles.num}>{disk(c.volume)}</td>
									<td
										id={`switch-note-${c.id}`}
										className={cx(styles.num, !c.fits_if_toggled && text.err)}
									>
										{c.required ? DASH : toggleNote(c)}
									</td>
								</tr>
							))}
						</tbody>
					</table>
				</div>
			</section>
			<section className={ui.card} aria-labelledby="sizes-files-h">
				<h3 id="sizes-files-h">{t('media.sizes.files')}</h3>
				<div className={styles.scroll}>
					<table className={styles.table}>
						<thead>
							<tr>
								{fileColumns.map((c) => (
									<Head key={c.id} column={c} sort={fSort} onSort={sortFiles} />
								))}
							</tr>
						</thead>
						<tbody>
							{files.map((f, i) => (
								<tr key={`${f.path}:${f.targets.join()}:${i}`}>
									<th scope="row" className={text.mono}>
										{f.path}
									</th>
									<td>{nameOf(f.component)}</td>
									<td className={styles.num}>{formatBytes(f.raw)}</td>
									<td className={styles.num}>{formatBytes(f.packed)}</td>
									<td>{t(f.pack === 'common' ? 'media.pack.common' : 'media.pack.own')}</td>
									<td>{targetNames(f.targets)}</td>
									<td className={styles.num}>{disk(f.volume)}</td>
								</tr>
							))}
						</tbody>
					</table>
				</div>
			</section>
		</div>
	);
}
