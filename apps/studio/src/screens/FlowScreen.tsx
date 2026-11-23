// The Flow screen: the step graph, its editor and the validation errors.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { type ReactNode } from 'react';
import { t } from '../i18n/messages';
import {
	NODE_H,
	NODE_W,
	STEP_TYPES,
	addStep,
	connect,
	disconnect,
	layoutGraph,
	removeStep,
	renameStep,
	updateStep,
	validateFlow,
} from '../lib/flowEdit';
import type { Flow, Step } from '../lib/types';
import { useProject, useStudio } from '../app/StudioContext';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import styles from './FlowScreen.module.css';

const trim = (s: string, max: number) => (s.length > max ? s.slice(0, max - 1) + '…' : s);

function Graph({ flow, errorSteps }: { flow: Flow; errorSteps: Set<string> }) {
	const studio = useStudio();
	const selectedStep = useProject((s) => s.selectedStep);
	const connectFrom = useProject((s) => s.connectFrom);
	const g = layoutGraph(flow);
	const pos = new Map(g.nodes.map((n) => [n.id, n]));

	const activate = (id: string) => {
		if (connectFrom && connectFrom !== id) {
			const from = flow.steps.find((x) => x.id === connectFrom);
			const next = connect(flow, connectFrom, id, from?.type === 'branch' ? -1 : undefined);
			studio.project.setState({ connectFrom: null });
			void studio.run(() =>
				studio.saveFlow(next, t('history.connect', { from: from?.id ?? '', to: id })),
			);
		} else studio.project.setState({ selectedStep: id, connectFrom: null });
	};

	return (
		<svg
			className={styles.graph}
			width={g.width}
			height={g.height + 20}
			viewBox={`0 0 ${g.width} ${g.height + 20}`}
			role="group"
			aria-label={t('flow.graphLabel', { id: flow.id })}
		>
			<defs>
				<marker
					id="arrow"
					viewBox="0 0 10 10"
					refX={9}
					refY={5}
					markerWidth={7}
					markerHeight={7}
					orient="auto-start-reverse"
				>
					<path d="M0 0L10 5L0 10z" fill="currentColor" />
				</marker>
			</defs>
			{g.edges.map((e, i) => {
				const a = pos.get(e.from)!;
				const b = pos.get(e.to)!;
				let d: string;
				if (e.back) {
					const x1 = a.x + NODE_W / 2;
					const y1 = a.y + NODE_H;
					const x2 = b.x + NODE_W / 2;
					const y2 = b.y + NODE_H;
					const dip = Math.max(y1, y2) + 24;
					d = `M${x1} ${y1} C${x1} ${dip} ${x2} ${dip} ${x2} ${y2}`;
				} else {
					const x1 = a.x + NODE_W;
					const y1 = a.y + NODE_H / 2;
					const x2 = b.x;
					const y2 = b.y + NODE_H / 2;
					const mx = (x1 + x2) / 2;
					d = `M${x1} ${y1} C${mx} ${y1} ${mx} ${y2} ${x2} ${y2}`;
				}
				const lx = e.back ? (a.x + b.x) / 2 + NODE_W / 2 : (a.x + NODE_W + b.x) / 2;
				const ly = e.back ? Math.max(a.y, b.y) + NODE_H + 22 : (a.y + b.y) / 2 + NODE_H / 2 - 6;
				return (
					<g key={i}>
						<path
							d={d}
							className={cx(styles.edge, e.label && styles.branch)}
							fill="none"
							markerEnd="url(#arrow)"
						/>
						{e.label ? (
							<text x={lx} y={ly} className={styles.edgeLabel} textAnchor="middle">
								{trim(e.label, 22)}
							</text>
						) : null}
					</g>
				);
			})}
			{flow.steps.map((step) => {
				const n = pos.get(step.id)!;
				return (
					<g
						key={step.id}
						className={cx(
							styles.node,
							styles[`t${step.type[0]!.toUpperCase()}${step.type.slice(1)}`],
							selectedStep === step.id && styles.selected,
							errorSteps.has(step.id) && styles.invalid,
							connectFrom && styles.connectable,
						)}
						transform={`translate(${n.x} ${n.y})`}
						tabIndex={0}
						role="button"
						aria-label={t('flow.nodeLabel', { type: step.type, id: step.id })}
						onClick={() => activate(step.id)}
						onKeyDown={(e) => {
							if (e.key === 'Enter' || e.key === ' ') {
								e.preventDefault();
								activate(step.id);
							}
						}}
					>
						<rect width={NODE_W} height={NODE_H} rx={8} />
						<text x={12} y={21} className={styles.nodeId}>
							{trim(step.id, 17)}
						</text>
						<text x={12} y={39} className={styles.nodeType}>
							{step.type + (step.when ? ` · ${t('flow.conditional')}` : '')}
						</text>
					</g>
				);
			})}
		</svg>
	);
}

function Field({ label, children }: { label: string; children: ReactNode }) {
	return (
		<label className={ui.field}>
			<span className={ui.fieldLabel}>{label}</span>
			{children}
		</label>
	);
}

/** A select that reports its value on change. `value` of undefined shows the blank choice. */
function Choice({
	value,
	choices,
	onChange,
	blank,
}: {
	value: string | undefined;
	choices: string[];
	onChange: (v: string) => void;
	blank: string;
}) {
	return (
		<select value={value ?? ''} onChange={(e) => onChange(e.target.value)}>
			<option value="">{blank}</option>
			{choices.map((c) => (
				<option key={c} value={c}>
					{c}
				</option>
			))}
		</select>
	);
}

/** A text box that saves when it is left, as a change event does. */
function TextBox({
	value,
	onChange,
	mono = true,
	label,
}: {
	value: string | undefined;
	onChange: (v: string) => void;
	mono?: boolean;
	label?: string;
}) {
	return (
		<input
			// A new value from outside (another step, an undo) replaces what was typed.
			key={value ?? ''}
			type="text"
			defaultValue={value ?? ''}
			className={mono ? text.mono : undefined}
			aria-label={label}
			onBlur={(e) => {
				if (e.target.value !== (value ?? '')) onChange(e.target.value.trim());
			}}
			onKeyDown={(e) => {
				if (e.key === 'Enter') e.currentTarget.blur();
			}}
		/>
	);
}

function StepEditor({ flow, step }: { flow: Flow; step: Step }) {
	const studio = useStudio();
	const layouts = useProject((s) => s.summary!.layouts);
	const save = (next: Flow, label: string) => void studio.run(() => studio.saveFlow(next, label));
	const ids = flow.steps.map((x) => x.id);
	const none = t('flow.none');
	return (
		<div>
			<h3>{t('flow.step')}</h3>
			<Field label={t('flow.id')}>
				<TextBox
					value={step.id}
					onChange={(v) => {
						studio.project.setState({ selectedStep: v });
						save(renameStep(flow, step.id, v), t('history.renameStep', { from: step.id, to: v }));
					}}
				/>
			</Field>
			<Field label={t('flow.type')}>
				<select
					value={step.type}
					onChange={(e) =>
						save(
							updateStep(flow, step.id, { type: e.target.value as Step['type'] }),
							t('history.changeType', { id: step.id, type: e.target.value }),
						)
					}
				>
					{STEP_TYPES.map((c) => (
						<option key={c} value={c}>
							{c}
						</option>
					))}
				</select>
			</Field>
			{step.type !== 'action' && step.type !== 'branch' ? (
				<Field label={t('flow.layout')}>
					<Choice
						value={step.layout}
						choices={layouts}
						blank={none}
						onChange={(v) =>
							save(
								updateStep(flow, step.id, { layout: v }),
								t('history.setLayout', { id: step.id }),
							)
						}
					/>
				</Field>
			) : null}
			<Field label={t('flow.skipUnless')}>
				<TextBox
					value={step.when}
					onChange={(v) =>
						save(updateStep(flow, step.id, { when: v }), t('history.setCondition', { id: step.id }))
					}
				/>
			</Field>
			<Field label={t('flow.onEnter')}>
				<TextBox
					value={step.on_enter}
					onChange={(v) =>
						save(updateStep(flow, step.id, { on_enter: v }), t('history.setHook', { id: step.id }))
					}
				/>
			</Field>
			{step.type !== 'finish' ? (
				<Field label={step.type === 'branch' ? t('flow.otherwiseGoTo') : t('flow.next')}>
					<Choice
						value={step.next}
						choices={ids.filter((i) => i !== step.id)}
						blank={none}
						onChange={(v) =>
							save(
								v ? connect(flow, step.id, v) : disconnect(flow, step.id),
								t('history.connectStep', { id: step.id }),
							)
						}
					/>
				</Field>
			) : null}
			{step.type === 'branch' || (step.branches?.length ?? 0) > 0 ? (
				<div>
					<h3>{t('flow.branches')}</h3>
					{(step.branches ?? []).map((b, i) => (
						<div key={i} className={ui.branchRow}>
							<TextBox
								value={b.when}
								onChange={(v) => {
									const f = structuredClone(flow);
									f.steps.find((x) => x.id === step.id)!.branches![i]!.when = v;
									save(f, t('history.editBranch', { id: step.id }));
								}}
							/>
							<Choice
								value={b.next}
								choices={ids}
								blank={t('flow.choose')}
								onChange={(v) =>
									save(connect(flow, step.id, v, i), t('history.connectBranch', { id: step.id }))
								}
							/>
							<button
								type="button"
								className={cx(ui.btn, ui.ghost, ui.small)}
								aria-label={t('flow.removeBranch')}
								onClick={() =>
									save(disconnect(flow, step.id, i), t('history.removeBranch', { id: step.id }))
								}
							>
								×
							</button>
						</div>
					))}
					<button
						type="button"
						className={cx(ui.btn, ui.ghost)}
						onClick={() =>
							save(
								connect(flow, step.id, ids.find((i) => i !== step.id) ?? step.id, -1),
								t('history.addBranch', { id: step.id }),
							)
						}
					>
						{t('flow.addBranch')}
					</button>
				</div>
			) : null}
			<div className={ui.rowActions}>
				<button
					type="button"
					className={ui.btn}
					onClick={() => studio.project.setState({ connectFrom: step.id })}
				>
					{t('flow.connectTo')}
				</button>
				<button
					type="button"
					className={cx(ui.btn, ui.danger)}
					onClick={() => {
						studio.project.setState({ selectedStep: null });
						save(removeStep(flow, step.id), t('history.removeStep', { id: step.id }));
					}}
				>
					{t('flow.removeStep')}
				</button>
			</div>
		</div>
	);
}

export function FlowScreen() {
	const studio = useStudio();
	const summary = useProject((s) => s.summary)!;
	const view = useProject((s) => s.flow);
	const flowId = useProject((s) => s.flowId);
	const selectedStep = useProject((s) => s.selectedStep);
	const connectFrom = useProject((s) => s.connectFrom);
	if (!view) return <p className={text.muted}>{t('common.loading')}</p>;
	const flow = view.flow;
	// Show the backend's problems, plus any the local rules find that it has not seen yet.
	const problems = view.errors.length ? view.errors : validateFlow(flow);
	const errorSteps = new Set(problems.flatMap((p) => (p.step ? [p.step] : [])));
	const selected = flow.steps.find((x) => x.id === selectedStep);
	const add = (type: Step['type']) => {
		const { flow: next, id } = addStep(flow, type, selectedStep ?? undefined);
		studio.project.setState({ selectedStep: id });
		void studio.run(() => studio.saveFlow(next, t('history.addStep', { type, id })));
	};
	return (
		<div className={ui.screen}>
			<div className={ui.screenHead}>
				<div>
					<h1>{t('flow.title')}</h1>
					<p className={text.muted}>{t('flow.intro')}</p>
				</div>
			</div>
			<div className={ui.tabs} role="tablist">
				{summary.flows.map((f) => (
					<button
						key={f.id}
						type="button"
						role="tab"
						aria-selected={f.id === flowId}
						className={cx(ui.tab, f.id === flowId && ui.active)}
						onClick={() =>
							void studio.run(async () => {
								studio.project.setState({ selectedStep: null });
								await studio.loadFlow(f.id);
							})
						}
					>
						{f.id}
						{f.errors ? <span className={ui.badge}>{String(f.errors)}</span> : null}
					</button>
				))}
			</div>
			<div className={ui.toolbar}>
				<span className={cx(text.dim, text.small)}>{t('flow.addAfterSelection')}</span>
				{STEP_TYPES.map((type) => (
					<button key={type} type="button" className={ui.btn} onClick={() => add(type)}>
						+ {type}
					</button>
				))}
				{connectFrom ? (
					<span className={ui.hint}>
						{t('flow.pickTarget', { id: connectFrom })}
						<button
							type="button"
							className={cx(ui.btn, ui.ghost, ui.small)}
							onClick={() => studio.project.setState({ connectFrom: null })}
						>
							{t('common.cancel')}
						</button>
					</span>
				) : null}
			</div>
			<div className={styles.flowLayout}>
				<div className={cx(styles.graphWrap, ui.card)}>
					<Graph flow={flow} errorSteps={errorSteps} />
				</div>
				<aside className={ui.card}>
					{selected ? (
						<StepEditor flow={flow} step={selected} />
					) : (
						<p className={text.muted}>{t('flow.selectToEdit')}</p>
					)}
					<h3>
						{problems.length === 0
							? t('flow.valid')
							: t('flow.problems', { count: problems.length })}
					</h3>
					{problems.length === 0 ? (
						<p className={text.ok}>{t('flow.passes')}</p>
					) : (
						<ul className={ui.problems}>
							{problems.map((p, i) => (
								<li key={i} className={p.severity === 'error' ? ui.error : undefined}>
									{p.step ? (
										<>
											<a
												href="#"
												onClick={(e) => {
													e.preventDefault();
													studio.project.setState({ selectedStep: p.step! });
												}}
											>
												{p.step}
											</a>{' '}
										</>
									) : null}
									{p.message}
								</li>
							))}
						</ul>
					)}
				</aside>
			</div>
		</div>
	);
}
