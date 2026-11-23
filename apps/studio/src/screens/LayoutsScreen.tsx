// The Layouts screen: a node tree editor, per-renderer overrides and a live preview.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useState } from 'react';
import { t } from '../i18n/messages';
import {
	NODE_KINDS,
	RENDERERS,
	addChild,
	describeNode,
	effectiveNode,
	flattenTree,
	getNode,
	hasOverride,
	moveNode,
	parsePropValue,
	propToText,
	removeNode,
	setBind,
	setKind,
	setProp,
	toggleOverride,
} from '../lib/layoutEdit';
import type { Layout } from '../lib/types';
import { LayoutPreview } from '../preview/PreviewTree';
import { lookFor } from '../preview/model';
import { useProject, useStudio } from '../app/StudioContext';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import styles from './LayoutsScreen.module.css';

const same = (a: number[], b: number[]) => a.length === b.length && a.every((v, i) => v === b[i]);

function Tree({ layout }: { layout: Layout }) {
	const studio = useStudio();
	const layoutPath = useProject((s) => s.layoutPath);
	const [addKind, setAddKind] = useState('text');
	const save = (next: Layout, label: string) =>
		void studio.run(() => studio.saveLayout(next, label));
	const node = getNode(layout.root, layoutPath);
	const select = (path: number[]) => studio.project.setState({ layoutPath: path });
	return (
		<div>
			<ul className={styles.tree} role="tree" aria-label={t('layouts.nodes')}>
				{flattenTree(layout.root).map(({ path, depth, node: n }) => (
					<li
						key={path.join('.')}
						role="treeitem"
						aria-selected={same(path, layoutPath)}
						className={same(path, layoutPath) ? styles.selected : undefined}
						style={{ paddingInlineStart: 8 + depth * 16 }}
						tabIndex={0}
						onClick={() => select(path)}
						onKeyDown={(e) => {
							if (e.key === 'Enter') select(path);
						}}
					>
						<span className={styles.kind}>{describeNode(n)}</span>
						{Object.keys(n.overrides ?? {}).map((r) => (
							<span key={r} className={ui.chip} title={t('layouts.hasOverride', { renderer: r })}>
								{r}
							</span>
						))}
					</li>
				))}
			</ul>
			<div className={ui.toolbar}>
				<select
					aria-label={t('layouts.kindToAdd')}
					value={addKind}
					onChange={(e) => setAddKind(e.target.value)}
				>
					{NODE_KINDS.map((k) => (
						<option key={k} value={k}>
							{k}
						</option>
					))}
				</select>
				<button
					type="button"
					className={ui.btn}
					disabled={!node}
					onClick={() => {
						const r = addChild(layout, layoutPath, addKind);
						select(r.path);
						save(r.layout, t('history.addNode', { kind: addKind, id: layout.id }));
					}}
				>
					{t('layouts.addChild')}
				</button>
				<button
					type="button"
					className={cx(ui.btn, ui.ghost)}
					disabled={layoutPath.length === 0}
					aria-label={t('layouts.moveUp')}
					onClick={() => {
						const r = moveNode(layout, layoutPath, -1);
						select(r.path);
						save(r.layout, t('history.moveNode', { id: layout.id }));
					}}
				>
					↑
				</button>
				<button
					type="button"
					className={cx(ui.btn, ui.ghost)}
					disabled={layoutPath.length === 0}
					aria-label={t('layouts.moveDown')}
					onClick={() => {
						const r = moveNode(layout, layoutPath, 1);
						select(r.path);
						save(r.layout, t('history.moveNode', { id: layout.id }));
					}}
				>
					↓
				</button>
				<button
					type="button"
					className={cx(ui.btn, ui.danger)}
					disabled={layoutPath.length === 0}
					onClick={() => {
						const p = layoutPath;
						select(p.slice(0, -1));
						save(removeNode(layout, p), t('history.removeNode', { id: layout.id }));
					}}
				>
					{t('layouts.remove')}
				</button>
			</div>
		</div>
	);
}

function Inspector({ layout }: { layout: Layout }) {
	const studio = useStudio();
	const layoutPath = useProject((s) => s.layoutPath);
	const r = useProject((s) => s.overrideRenderer);
	const [newKey, setNewKey] = useState('');
	const base = getNode(layout.root, layoutPath);
	if (!base) return <p className={text.muted}>{t('layouts.selectNode')}</p>;
	const overridden = r ? hasOverride(base, r) : false;
	const node = effectiveNode(base, overridden ? r : null);
	const editR = overridden ? r : null;
	const save = (next: Layout, label: string) =>
		void studio.run(() => studio.saveLayout(next, label));
	const props = Object.entries(node.props ?? {});
	return (
		<div>
			<h3>{t('layouts.node')}</h3>
			<label className={ui.field}>
				<span className={ui.fieldLabel}>{t('layouts.editFor')}</span>
				<select
					value={r ?? ''}
					onChange={(e) => studio.project.setState({ overrideRenderer: e.target.value || null })}
				>
					<option value="">{t('layouts.allRenderers')}</option>
					{RENDERERS.map((x) => (
						<option key={x} value={x}>
							{x}
						</option>
					))}
				</select>
			</label>
			{r ? (
				<label className={ui.check}>
					<input
						type="checkbox"
						checked={overridden}
						onChange={(e) =>
							save(
								toggleOverride(layout, layoutPath, r, e.target.checked),
								e.target.checked
									? t('history.addOverride', { renderer: r, id: layout.id })
									: t('history.removeOverride', { renderer: r, id: layout.id }),
							)
						}
					/>
					{t('layouts.overrideFor', { renderer: r })}
				</label>
			) : null}
			<label className={ui.field}>
				<span className={ui.fieldLabel}>{t('layouts.kind')}</span>
				<select
					value={node.kind}
					onChange={(e) =>
						save(
							setKind(layout, layoutPath, e.target.value, editR),
							t('history.changeKind', { id: layout.id }),
						)
					}
				>
					{NODE_KINDS.map((k) => (
						<option key={k} value={k}>
							{k}
						</option>
					))}
				</select>
			</label>
			<label className={ui.field}>
				<span className={ui.fieldLabel}>{t('layouts.bind')}</span>
				<input
					key={`${layoutPath.join('.')}:${node.bind ?? ''}`}
					type="text"
					className={text.mono}
					defaultValue={node.bind ?? ''}
					onBlur={(e) => {
						if (e.target.value !== (node.bind ?? ''))
							save(
								setBind(layout, layoutPath, e.target.value.trim(), editR),
								t('history.bindNode', { id: layout.id }),
							);
					}}
					onKeyDown={(e) => {
						if (e.key === 'Enter') e.currentTarget.blur();
					}}
				/>
			</label>
			<h3>{t('layouts.properties')}</h3>
			{props.map(([k, v]) => (
				<div key={k} className={ui.propRow}>
					<label className={cx(ui.propKey, text.mono, text.small)} htmlFor={`prop-${k}`}>
						{k}
					</label>
					<input
						id={`prop-${k}`}
						key={`${layoutPath.join('.')}:${k}:${propToText(v)}`}
						type="text"
						className={text.mono}
						defaultValue={propToText(v)}
						onBlur={(e) => {
							if (e.target.value !== propToText(v))
								save(
									setProp(layout, layoutPath, k, parsePropValue(e.target.value, v), editR),
									t('history.editProp', { prop: k, id: layout.id }),
								);
						}}
						onKeyDown={(e) => {
							if (e.key === 'Enter') e.currentTarget.blur();
						}}
					/>
					<button
						type="button"
						className={cx(ui.btn, ui.ghost, ui.small)}
						aria-label={t('layouts.removeProp', { prop: k })}
						onClick={() =>
							save(
								setProp(layout, layoutPath, k, undefined, editR),
								t('history.removeProp', { prop: k, id: layout.id }),
							)
						}
					>
						×
					</button>
				</div>
			))}
			<div className={ui.propRow}>
				<input
					type="text"
					className={text.mono}
					placeholder={t('layouts.newProp')}
					aria-label={t('layouts.newPropName')}
					value={newKey}
					onChange={(e) => setNewKey(e.target.value)}
				/>
				<button
					type="button"
					className={ui.btn}
					onClick={() => {
						const key = newKey.trim();
						if (key) {
							save(
								setProp(layout, layoutPath, key, 'value', editR),
								t('history.addProp', { prop: key, id: layout.id }),
							);
							setNewKey('');
						}
					}}
				>
					{t('layouts.add')}
				</button>
			</div>
		</div>
	);
}

function LayoutPicker() {
	const studio = useStudio();
	const ids = useProject((s) => s.summary!.layouts);
	const layoutId = useProject((s) => s.layoutId);
	const groups = [...new Set(ids.map((id) => (id.includes('/') ? id.split('/')[0]! : '')))].sort(
		(a, b) => (a === '' ? -1 : b === '' ? 1 : a.localeCompare(b)),
	);
	return (
		<label className={cx(ui.field, ui.inline)}>
			<span className={ui.fieldLabel}>{t('layouts.layout')}</span>
			<select
				aria-label={t('layouts.layout')}
				value={layoutId}
				onChange={(e) => void studio.run(() => studio.loadLayout(e.target.value))}
			>
				{groups.map((g) => (
					<optgroup
						key={g}
						label={g === '' ? t('layouts.sharedGroup') : t('layouts.themeGroup', { theme: g })}
					>
						{ids
							.filter((id) => (id.includes('/') ? id.split('/')[0] : '') === g)
							.map((id) => (
								<option key={id} value={id}>
									{id.includes('/') ? id.split('/')[1]! : id}
								</option>
							))}
					</optgroup>
				))}
			</select>
		</label>
	);
}

export function LayoutsScreen() {
	const studio = useStudio();
	const summary = useProject((s) => s.summary)!;
	const layout = useProject((s) => s.layout);
	const layoutId = useProject((s) => s.layoutId);
	const layoutRenderer = useProject((s) => s.layoutRenderer);
	// A theme's own layout is previewed by a renderer that theme supports.
	const themeId = layoutId.includes('/') ? layoutId.split('/')[0]! : null;
	const theme = summary.themes.find((x) => x.id === themeId) ?? null;
	const renderer =
		theme && !theme.renderers.includes(layoutRenderer)
			? (theme.renderers[0] ?? layoutRenderer)
			: layoutRenderer;
	useEffect(() => {
		if (renderer !== layoutRenderer) studio.project.setState({ layoutRenderer: renderer });
	}, [renderer, layoutRenderer, studio]);
	if (!layout) return <p className={text.muted}>{t('common.loading')}</p>;
	return (
		<div className={ui.screen}>
			<div className={ui.screenHead}>
				<div>
					<h1>{t('layouts.title')}</h1>
					<p className={text.muted}>{t('layouts.intro')}</p>
				</div>
			</div>
			<LayoutPicker />
			<div className={styles.layoutGrid}>
				<section className={ui.card} aria-label={t('layouts.nodeTree')}>
					<Tree layout={layout} />
				</section>
				<section className={ui.card} aria-label={t('layouts.nodeProperties')}>
					<Inspector layout={layout} />
				</section>
				<section className={ui.card} aria-label={t('layouts.livePreview')}>
					<div className={ui.toolbar}>
						<label className={cx(ui.field, ui.inline)}>
							<span className={ui.fieldLabel}>{t('layouts.previewAs')}</span>
							<select
								value={renderer}
								onChange={(e) => studio.project.setState({ layoutRenderer: e.target.value })}
							>
								{RENDERERS.map((r) => (
									<option key={r} value={r}>
										{r}
									</option>
								))}
							</select>
						</label>
						<span className={ui.chip}>
							{t('layouts.look', {
								look: lookFor(renderer, themeId, layout.root.props?.style as string | undefined),
							})}
						</span>
					</div>
					<div className={styles.host}>
						<LayoutPreview
							layout={layout}
							renderer={renderer}
							options={{ theme: themeId, components: summary.components }}
						/>
					</div>
				</section>
			</div>
		</div>
	);
}
