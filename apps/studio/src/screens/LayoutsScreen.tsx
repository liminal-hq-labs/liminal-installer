// The Layouts screen: the whole composed screen of a page (theme stage and page), a node tree editor and a live preview.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useMemo, useState } from 'react';
import { t, tn } from '../i18n/messages';
import {
	NODE_KINDS,
	RENDERERS,
	describeNode,
	effectiveNode,
	getNode,
	hasOverride,
	insertNodes,
	moveNode,
	parsePropValue,
	propToText,
	removeNode,
	setBind,
	setKind,
	setProp,
	toggleOverride,
} from '../lib/layoutEdit';
import { GROUPS, METER_SOURCES, PALETTE, meterLabel, paletteItem } from '../lib/palette';
import {
	attachStage,
	detachStage,
	isStageId,
	parseRef,
	refKey,
	screenRows,
	type Ref,
	type Scope,
	type StepInfo,
	LayoutPreview,
	hasSchemes,
	lookFor,
	type Scheme,
} from '@liminal-hq/installer-renderer';
import type { Layout, LayoutNode } from '../lib/types';
import { TerminalPreview, TerminalViewSwitch, type TerminalView } from '../preview/TerminalPreview';
import { SchemeSwitch } from '../preview/SchemeSwitch';
import { usePreview, useProject, useStudio } from '../app/StudioContext';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import styles from './LayoutsScreen.module.css';

const same = (a: number[], b: number[]) => a.length === b.length && a.every((v, i) => v === b[i]);

/** The layouts a screen is made of: the open layout, and the stage of its theme when it has one. */
function useScreenParts() {
	const layout = useProject((s) => s.layout);
	const stageLayout = useProject((s) => s.stageLayout);
	const layoutId = useProject((s) => s.layoutId);
	const scope = useProject((s) => s.layoutScope);
	const flow = useProject((s) => s.flow);
	const openIsStage = isStageId(layoutId);
	const stage = openIsStage ? layout : stageLayout;
	const page = openIsStage ? null : layout;
	const name = layoutId.includes('/') ? layoutId.split('/')[1]! : layoutId;
	const step: StepInfo = useMemo(() => {
		const s = flow?.flow.steps.find((x) => x.layout === name);
		return { id: s?.id ?? name, kind: s?.type ?? 'page' };
	}, [flow, name]);
	// What an edit applies to: the stage when a stage node is selected, else the page.
	const active = scope === 'stage' && stage ? stage : (page ?? stage);
	const activeScope: Scope = active === stage && stage ? 'stage' : 'page';
	return { layout, stage, page, step, openIsStage, active, activeScope };
}

function Tree() {
	const studio = useStudio();
	const layoutPath = useProject((s) => s.layoutPath);
	const { stage, page, step, openIsStage, active, activeScope } = useScreenParts();
	const [addId, setAddId] = useState('text');
	const item = paletteItem(addId) ?? PALETTE[0]!;
	const [scopeChoice, setScopeChoice] = useState<Scope | null>(null);
	const canStage = Boolean(stage) && !openIsStage;
	const target: Scope =
		scopeChoice && (scopeChoice === 'page' || canStage)
			? scopeChoice
			: item.scope === 'stage' && canStage
				? 'stage'
				: openIsStage
					? 'stage'
					: 'page';
	const save = (next: Layout, label: string) =>
		void studio.run(() => studio.saveLayout(next, label));
	const select = (ref: Ref) =>
		studio.project.setState({ layoutScope: ref.scope, layoutPath: ref.path });
	const node = active ? getNode(active.root, layoutPath) : undefined;
	const rows = screenRows(stage?.root ?? null, page?.root ?? null, step);
	const targetLayout = target === 'stage' ? stage : (page ?? stage);
	return (
		<div>
			<ul className={styles.tree} role="tree" aria-label={t('layouts.nodes')}>
				{rows.map(({ ref, depth, node: n }) => {
					const selected = ref.scope === activeScope && same(ref.path, layoutPath);
					const shared = ref.scope === 'stage' && !openIsStage && canStage;
					return (
						<li
							key={refKey(ref)}
							role="treeitem"
							aria-selected={selected}
							className={cx(selected && styles.selected, shared && styles.shared)}
							style={{ paddingInlineStart: 8 + depth * 16 }}
							tabIndex={0}
							data-scope={ref.scope}
							onClick={() => select(ref)}
							onKeyDown={(e) => {
								if (e.key === 'Enter') select(ref);
							}}
						>
							<span className={styles.kind}>{describeNode(n)}</span>
							{shared ? (
								<span className={cx(ui.chip, styles.scopeChip)} title={t('layouts.scopeStageHint')}>
									{t('layouts.scopeStage')}
								</span>
							) : null}
							{Object.keys(n.overrides ?? {}).map((r) => (
								<span key={r} className={ui.chip} title={t('layouts.hasOverride', { renderer: r })}>
									{r}
								</span>
							))}
						</li>
					);
				})}
			</ul>
			<div className={styles.palette}>
				<label className={ui.field}>
					<span className={ui.fieldLabel}>{t('layouts.kindToAdd')}</span>
					<select
						aria-label={t('layouts.kindToAdd')}
						value={addId}
						onChange={(e) => {
							setAddId(e.target.value);
							setScopeChoice(null);
						}}
					>
						{GROUPS.map((g) => (
							<optgroup key={g.id} label={g.label()}>
								{PALETTE.filter((p) => p.group === g.id).map((p) => (
									<option key={p.id} value={p.id}>
										{p.label()}
									</option>
								))}
							</optgroup>
						))}
					</select>
				</label>
				<p className={cx(text.dim, text.small)} data-testid="palette-hint">
					{item.hint()}
				</p>
				{canStage ? (
					<div role="radiogroup" aria-label={t('layouts.addTo')} className={styles.scopePick}>
						{(['page', 'stage'] as const).map((sc) => (
							<label key={sc} className={ui.check}>
								<input
									type="radio"
									name="add-scope"
									checked={target === sc}
									onChange={() => setScopeChoice(sc)}
								/>
								{sc === 'page' ? t('layouts.addToPage') : t('layouts.addToStage')}
							</label>
						))}
					</div>
				) : null}
			</div>
			<div className={ui.toolbar}>
				<button
					type="button"
					className={ui.btn}
					disabled={!targetLayout}
					onClick={() => {
						if (!targetLayout) return;
						// Inside the selection when it is in the layout being added to, else at that layout's root.
						const at = targetLayout === active ? layoutPath : [];
						const r = insertNodes(targetLayout, at, item.build());
						select({ scope: targetLayout === stage ? 'stage' : 'page', path: r.path });
						save(
							r.layout,
							t('history.addNode', { kind: item.label().toLowerCase(), id: targetLayout.id }),
						);
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
						if (!active) return;
						const r = moveNode(active, layoutPath, -1);
						select({ scope: activeScope, path: r.path });
						save(r.layout, t('history.moveNode', { id: active.id }));
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
						if (!active) return;
						const r = moveNode(active, layoutPath, 1);
						select({ scope: activeScope, path: r.path });
						save(r.layout, t('history.moveNode', { id: active.id }));
					}}
				>
					↓
				</button>
				<button
					type="button"
					className={cx(ui.btn, ui.danger)}
					disabled={layoutPath.length === 0 || node?.kind === 'slot'}
					onClick={() => {
						if (!active) return;
						const p = layoutPath;
						select({ scope: activeScope, path: p.slice(0, -1) });
						save(removeNode(active, p), t('history.removeNode', { id: active.id }));
					}}
				>
					{t('layouts.remove')}
				</button>
			</div>
		</div>
	);
}

function Inspector() {
	const studio = useStudio();
	const layoutPath = useProject((s) => s.layoutPath);
	const summary = useProject((s) => s.summary)!;
	const layoutId = useProject((s) => s.layoutId);
	const { page, stage, step, openIsStage, active, activeScope } = useScreenParts();
	const layout = active;
	const r = useProject((s) => s.overrideRenderer);
	const [newKey, setNewKey] = useState('');
	if (!layout) return <p className={text.muted}>{t('layouts.selectNode')}</p>;
	const base = getNode(layout.root, layoutPath);
	if (!base) return <p className={text.muted}>{t('layouts.selectNode')}</p>;
	const theme = layoutId.includes('/') ? layoutId.split('/')[0]! : '';
	const pagesAffected = summary.layouts.filter(
		(id) => id.startsWith(`${theme}/`) && !isStageId(id),
	).length;
	const hasStage = Boolean(stage) && !openIsStage;
	const ownBackdrop = page?.root.kind === 'backdrop' && hasStage;
	const overridden = r ? hasOverride(base, r) : false;
	const node = effectiveNode(base, overridden ? r : null);
	const editR = overridden ? r : null;
	const save = (next: Layout, label: string) =>
		void studio.run(() => studio.saveLayout(next, label));
	const props = Object.entries(node.props ?? {});
	return (
		<div>
			{hasStage && activeScope === 'stage' ? (
				<div className={styles.notice} role="note" data-testid="stage-notice">
					<strong>{t('layouts.scopeStageHint')}</strong>
					<span>{tn('layouts.stageNotice', pagesAffected, { theme })}</span>
					{page ? (
						<button
							type="button"
							className={ui.btn}
							onClick={() =>
								save(detachStage(stage!, page, step), t('history.detachStage', { id: page.id }))
							}
						>
							{t('layouts.detach')}
						</button>
					) : null}
				</div>
			) : null}
			{hasStage && activeScope === 'page' ? (
				<p className={cx(text.dim, text.small)}>{t('layouts.scopePage')}</p>
			) : null}
			{ownBackdrop && page ? (
				<div className={styles.notice} role="note">
					<span>{t('layouts.ownBackdrop')}</span>
					<button
						type="button"
						className={ui.btn}
						onClick={() => save(attachStage(page), t('history.attachStage', { id: page.id }))}
					>
						{t('layouts.useStage')}
					</button>
				</div>
			) : null}
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
			{node.kind === 'meter' ? (
				<MeterFields
					node={node}
					onChange={(next, key) => save(next(layout, layoutPath, editR), key)}
					id={layout.id}
				/>
			) : null}
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

/** The settings of a status meter: what it measures, its label, the free-space warning level and where it is drawn. */
function MeterFields({
	node,
	onChange,
	id,
}: {
	node: LayoutNode;
	onChange: (
		next: (layout: Layout, path: number[], r: string | null) => Layout,
		label: string,
	) => void;
	id: string;
}) {
	const p = node.props ?? {};
	const source = typeof p.source === 'string' ? p.source : 'total';
	const set = (key: string, value: unknown, extra?: [string, unknown]) =>
		onChange(
			(l, path, r) => {
				const a = setProp(l, path, key, value, r);
				return extra ? setProp(a, path, extra[0], extra[1], r) : a;
			},
			t('history.editProp', { prop: key, id }),
		);
	return (
		<fieldset className={styles.meterFields}>
			<legend>{t('layouts.meterSettings')}</legend>
			<label className={ui.field}>
				<span className={ui.fieldLabel}>{t('layouts.meterSource')}</span>
				<select value={source} onChange={(e) => set('source', e.target.value)}>
					{METER_SOURCES.map((x) => (
						<option key={x} value={x}>
							{meterLabel(x)}
						</option>
					))}
				</select>
			</label>
			<label className={ui.field}>
				<span className={ui.fieldLabel}>{t('layouts.meterLabel')}</span>
				<input
					key={String(p.label ?? '')}
					type="text"
					defaultValue={String(p.label ?? '')}
					onBlur={(e) => e.target.value !== p.label && set('label', e.target.value)}
				/>
			</label>
			{source === 'free_space' ? (
				<label className={ui.field}>
					<span className={ui.fieldLabel}>{t('layouts.meterWarn')}</span>
					<input
						key={String(p.low_below ?? '')}
						type="number"
						min={0}
						max={1}
						step={0.01}
						defaultValue={typeof p.low_below === 'number' ? p.low_below : 0.05}
						onBlur={(e) => {
							const v = Number(e.target.value);
							if (Number.isFinite(v) && v !== p.low_below) set('low_below', v);
						}}
					/>
				</label>
			) : null}
			<label className={ui.field}>
				<span className={ui.fieldLabel}>{t('layouts.meterPlace')}</span>
				<select
					value={p.place === 'dialog' ? 'dialog' : 'backdrop'}
					onChange={(e) =>
						e.target.value === 'dialog'
							? set('place', 'dialog', ['region', undefined])
							: set('place', 'backdrop', ['region', 'status'])
					}
				>
					<option value="backdrop">{t('layouts.placeBackdrop')}</option>
					<option value="dialog">{t('layouts.placeDialog')}</option>
				</select>
			</label>
		</fieldset>
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
	const { stage, step } = useScreenParts();
	const selected = useProject((s) => s.layoutPath);
	const scope = useProject((s) => s.layoutScope);
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
	const [view, setView] = useState<TerminalView>('rich');
	const scheme = usePreview((s) => s.previewScheme);
	const setScheme = (next: Scheme) => studio.preview.setState({ previewScheme: next });
	// The real terminal runs on a target that uses this renderer, and reopens after an edit to the
	// layout or its stage.
	const tuiTarget = summary.targets.find((x) => x.renderer === renderer)?.full_id;
	const real = renderer.startsWith('tui') && view === 'terminal' && tuiTarget;
	const editKey = useMemo(() => JSON.stringify([layout, stage]), [layout, stage]);
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
					<Tree />
				</section>
				<section className={ui.card} aria-label={t('layouts.nodeProperties')}>
					<Inspector />
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
						{renderer.startsWith('tui') && tuiTarget ? (
							<TerminalViewSwitch value={view} onChange={setView} />
						) : null}
						{hasSchemes(
							lookFor(renderer, themeId, layout.root.props?.style as string | undefined),
						) ? (
							<SchemeSwitch value={scheme} onChange={setScheme} />
						) : null}
						<span className={ui.chip}>
							{t('layouts.look', {
								look: lookFor(renderer, themeId, layout.root.props?.style as string | undefined),
							})}
						</span>
					</div>
					<div className={styles.host}>
						{real ? (
							<TerminalPreview
								api={studio.api}
								target={real}
								page={layoutId}
								refreshKey={editKey}
								label={t('terminal.label', { id: layoutId })}
							/>
						) : (
							<LayoutPreview
								layout={layout}
								stage={isStageId(layoutId) ? null : stage}
								step={step}
								renderer={renderer}
								options={{
									theme: themeId,
									scheme,
									components: summary.components,
									interact: {
										mode: 'edit',
										selected: refKey({
											scope: isStageId(layoutId) ? 'stage' : scope,
											path: selected,
										}),
										onSelect: (key) => {
											const ref = parseRef(key);
											if (ref)
												studio.project.setState({ layoutScope: ref.scope, layoutPath: ref.path });
										},
									},
								}}
							/>
						)}
					</div>
				</section>
			</div>
		</div>
	);
}
