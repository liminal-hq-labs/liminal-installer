// The disk map: a tree of what is on each disk, with a details pane for the row that is picked.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { t, type MessageId } from '../../i18n/messages';
import { formatBytes, siblings, visibleRows, volumeKey, type TreeRow } from '../../lib/media';
import type { EntryRole, MediaPlan } from '../../lib/types';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import ui from '../../ui/ui.module.css';
import styles from '../MediaScreen.module.css';
import { CapacityMeter } from './CapacityMeter';
import { DiskDetails } from './DiskDetails';

const ROLE_MESSAGE: Record<EntryRole, MessageId> = {
	folder: 'media.role.folder',
	setup: 'media.role.setup',
	own_pack: 'media.role.own_pack',
	common_pack: 'media.role.common_pack',
	readme: 'media.role.readme',
	autorun: 'media.role.autorun',
	extra: 'media.role.extra',
};

function rowName(row: TreeRow, plan: MediaPlan): string {
	if (row.kind === 'volume')
		return t('media.disk.root', {
			drive: plan.kind === 'cdrom' ? 'D:\\' : 'A:\\',
			label: row.volume.label,
		});
	return row.kind === 'pack' ? row.packEntry!.stored : row.entry!.name;
}

function rowRole(row: TreeRow): string {
	if (row.kind === 'volume') return t('media.role.volume');
	return row.kind === 'pack' ? t('media.role.packEntry') : t(ROLE_MESSAGE[row.entry!.role]);
}

/** The id of a tree row's element, for moving focus to it. */
const itemId = (key: string): string => `dm-${key.replace(/[^A-Za-z0-9_-]/g, '_')}`;

interface DiskTreeProps {
	plan: MediaPlan;
	rows: TreeRow[];
	focused: string;
	selected: string | null;
	expanded: ReadonlySet<string>;
	onFocus(key: string): void;
	onSelect(key: string): void;
	onExpand(key: string, open: boolean): void;
}

function DiskTree({
	plan,
	rows,
	focused,
	selected,
	expanded,
	onFocus,
	onSelect,
	onExpand,
}: DiskTreeProps) {
	const root = useRef<HTMLUListElement>(null);
	const number = rows[0]!.volume.number;
	const move = (key: string | undefined) => {
		if (!key) return;
		onFocus(key);
		root.current?.querySelector<HTMLElement>(`#${itemId(key)}`)?.focus();
	};
	const keys = (e: KeyboardEvent<HTMLElement>, row: TreeRow, i: number) => {
		const take = () => e.preventDefault();
		switch (e.key) {
			case 'ArrowDown':
				take();
				move(rows[i + 1]?.key);
				break;
			case 'ArrowUp':
				take();
				move(rows[i - 1]?.key);
				break;
			case 'Home':
				take();
				move(rows[0]?.key);
				break;
			case 'End':
				take();
				move(rows[rows.length - 1]?.key);
				break;
			case 'ArrowRight':
				take();
				if (row.expandable && !expanded.has(row.key)) onExpand(row.key, true);
				else if (row.expandable) move(rows[i + 1]?.key);
				break;
			case 'ArrowLeft':
				take();
				if (row.expandable && expanded.has(row.key)) onExpand(row.key, false);
				else move(row.parent ?? undefined);
				break;
			case 'Enter':
			case ' ':
				take();
				onSelect(row.key);
				break;
		}
	};
	return (
		<ul
			ref={root}
			role="tree"
			aria-label={t('media.map.label', { number })}
			className={styles.tree}
		>
			{rows.map((row, i) => {
				const open = expanded.has(row.key);
				return (
					<li
						key={row.key}
						id={itemId(row.key)}
						role="treeitem"
						aria-level={row.level}
						aria-setsize={siblings(rows, row)}
						aria-posinset={row.pos}
						aria-expanded={row.expandable ? open : undefined}
						aria-selected={selected === row.key}
						tabIndex={focused === row.key ? 0 : -1}
						className={cx(styles.node, selected === row.key && styles.nodeOn)}
						style={{ paddingInlineStart: `${(row.level - 1) * 16 + 8}px` }}
						data-key={row.key}
						onKeyDown={(e) => {
							// A key on a row is the row's; a parent row must not answer for its child's key.
							if (e.target === e.currentTarget) keys(e, row, i);
						}}
						onClick={(e) => {
							e.stopPropagation();
							onFocus(row.key);
							onSelect(row.key);
						}}
					>
						<span className={styles.twisty} aria-hidden="true">
							{row.expandable ? (open ? '▾' : '▸') : ''}
						</span>
						<span className={cx(text.mono, styles.nodeName)}>{rowName(row, plan)}</span>
						<span className={cx(text.small, text.muted, styles.nodeRole)}>{rowRole(row)}</span>
						<span className={cx(text.mono, text.small, styles.nodeSize)}>
							{formatBytes(row.size)}
						</span>
					</li>
				);
			})}
		</ul>
	);
}

/** The Disk map tab. A medium of one disk shows one tree; a set shows a tree for each disk, side by side. */
export function DiskMap({ plan }: { plan: MediaPlan }) {
	// A disk's root and the folders start open, so what is on the disk shows without a click.
	const [expanded, setExpanded] = useState<ReadonlySet<string>>(
		() =>
			new Set(
				plan.volumes.flatMap((v) => [
					volumeKey(v),
					...v.root.filter((e) => e.role === 'folder').map((e) => `${volumeKey(v)}/${e.name}`),
				]),
			),
	);
	// Each disk's tree has one tab stop of its own: the row last focused in it, or its root.
	const [focus, setFocus] = useState<Record<number, string>>({});
	const [selected, setSelected] = useState<string | null>(null);
	const trees = useMemo(
		() => plan.volumes.map((v) => visibleRows(v, expanded)),
		[plan.volumes, expanded],
	);
	const all = trees.flat();
	const picked = all.find((r) => r.key === selected) ?? null;
	const spans = plan.volumes.length > 1;
	const later = plan.components.filter((c) => c.volume > 1 && !c.default_on).map((c) => c.name);
	const expand = (key: string, open: boolean) =>
		setExpanded((s) => {
			const next = new Set(s);
			if (open) next.add(key);
			else next.delete(key);
			return next;
		});
	return (
		<div className={cx(styles.map, spans && styles.mapSpan)}>
			{spans && later.length > 0 ? (
				<p className={cx(text.muted, text.small)}>
					{t('media.span.note', { components: later.join(', ') })}
				</p>
			) : null}
			<div className={cx(styles.disks, spans && styles.disksSpan)}>
				{plan.volumes.map((v, i) => (
					<section key={v.number} className={ui.card} aria-labelledby={`dm-head-${v.number}`}>
						{spans ? (
							<>
								<h3 id={`dm-head-${v.number}`}>
									{t('media.disk.heading', { number: v.number, label: v.label })}
								</h3>
								<CapacityMeter kind={plan.kind} volume={v} reserve={plan.reserve} />
							</>
						) : (
							<h3 id={`dm-head-${v.number}`}>{t('media.tab.map')}</h3>
						)}
						{v.serial ? (
							<p className={cx(text.small, text.dim)}>
								{t('media.disk.serial', { serial: v.serial })}
							</p>
						) : null}
						<DiskTree
							plan={plan}
							rows={trees[i]!}
							focused={focus[v.number] ?? volumeKey(v)}
							selected={selected}
							expanded={expanded}
							onFocus={(key) => setFocus((f) => ({ ...f, [v.number]: key }))}
							onSelect={setSelected}
							onExpand={expand}
						/>
					</section>
				))}
			</div>
			<DiskDetails plan={plan} row={picked} />
		</div>
	);
}
