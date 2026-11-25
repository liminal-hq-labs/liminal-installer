// The Project screen: targets, components, themes, flows and checks at a glance.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef } from 'react';
import { t, tn } from '../i18n/messages';
import { errorCount, targetStatus } from '../lib/format';
import { useProject, usePreview, useStudio } from '../app/StudioContext';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import './ProjectScreen.module.css';

const STATUS_LABEL = {
	ok: 'project.status.ok',
	warning: 'project.status.warning',
	error: 'project.status.error',
} as const;

const PILL = { ok: ui.pillOk, warning: ui.pillWarning, error: ui.pillError } as const;

function Stat({ label, value, note }: { label: string; value: string | number; note: string }) {
	return (
		<div className={cx(ui.card, ui.stat)}>
			<div className={cx(text.dim, text.small)}>{label}</div>
			<div className={ui.statValue}>{String(value)}</div>
			<div className={cx(text.muted, text.small)}>{note}</div>
		</div>
	);
}

export function ProjectScreen() {
	const studio = useStudio();
	const summary = useProject((s) => s.summary)!;
	const problems = useProject((s) => s.problems);
	const mcp = useProject((s) => s.mcp);
	const checksRequest = useProject((s) => s.checksRequest);
	const previewTarget = usePreview((s) => s.previewTarget);
	const checks = useRef<HTMLElement>(null);
	const seen = useRef(checksRequest);

	// A request from the status bar brings the checks list into view and gives it focus.
	useEffect(() => {
		if (seen.current === checksRequest) return;
		seen.current = checksRequest;
		checks.current?.scrollIntoView?.({ block: 'start' });
		checks.current?.focus?.({ preventScroll: true });
	}, [checksRequest]);

	const required = summary.components.filter((c) => c.required).length;
	const errors = errorCount(problems);
	const dryRun = () =>
		void studio.run(async () => {
			await studio.runPreview(previewTarget);
			studio.go('preview');
		});

	return (
		<div className={ui.screen}>
			<div className={ui.screenHead}>
				<div>
					<h1>{summary.name}</h1>
					<p className={text.muted}>
						{t('project.identity', {
							id: summary.id,
							publisher: summary.publisher,
							version: summary.version,
						})}
					</p>
				</div>
				<button type="button" className={cx(ui.btn, ui.primary)} onClick={dryRun}>
					{t('project.dryRunTarget')}
				</button>
			</div>
			<div className={ui.stats}>
				<Stat
					label={t('project.targets')}
					value={summary.targets.length}
					note={[...new Set(summary.targets.map((x) => x.id))].join(' · ')}
				/>
				<Stat
					label={t('project.themes')}
					value={summary.themes.length}
					note={summary.themes.map((x) => x.name).join(' · ')}
				/>
				<Stat
					label={t('project.components')}
					value={summary.components.length}
					note={t('project.componentsNote', {
						required,
						optional: summary.components.length - required,
					})}
				/>
				<Stat
					label={t('project.checks')}
					value={errors === 0 ? t('project.checksPass') : tn('problems.errors', errors)}
					note={tn('project.findings', problems.length)}
				/>
			</div>
			<section className={ui.card} aria-labelledby="targets-h">
				<h2 id="targets-h">{t('project.targets')}</h2>
				<table>
					<thead>
						<tr>
							{(
								[
									'project.col.target',
									'project.col.renderer',
									'project.col.theme',
									'project.col.outputs',
									'project.col.components',
									'project.col.status',
								] as const
							).map((c) => (
								<th key={c} scope="col">
									{t(c)}
								</th>
							))}
						</tr>
					</thead>
					<tbody>
						{summary.targets.map((target) => {
							const status = targetStatus(problems, target.full_id);
							return (
								<tr key={target.full_id}>
									<td>{target.id}</td>
									<td className={cx(text.mono, text.violet)}>{target.renderer}</td>
									<td>{target.theme ?? '—'}</td>
									<td className={text.muted}>{target.outputs.join(' · ')}</td>
									<td className={text.muted}>{target.components.join(', ')}</td>
									<td>
										<span className={cx(ui.pill, PILL[status])}>{t(STATUS_LABEL[status])}</span>
									</td>
								</tr>
							);
						})}
					</tbody>
				</table>
			</section>
			<div className={ui.columns}>
				<section className={ui.card} aria-labelledby="comp-h">
					<h2 id="comp-h">{t('project.components')}</h2>
					<ul className={ui.plain}>
						{summary.components.map((c) => (
							<li key={c.id}>
								<strong>{c.name}</strong>
								<span className={text.dim}>
									{` ${c.id}${c.required ? ` · ${t('project.required')}` : ''}${
										c.default ? '' : ` · ${t('project.startsUnticked')}`
									}${
										c.targets.length
											? ` · ${t('project.onlyOn', { targets: c.targets.join(', ') })}`
											: ''
									}`}
								</span>
							</li>
						))}
					</ul>
				</section>
				<section className={ui.card} aria-labelledby="theme-h">
					<h2 id="theme-h">{t('project.themes')}</h2>
					<ul className={ui.plain}>
						{summary.themes.map((theme) => (
							<li key={theme.id}>
								<strong>{theme.name}</strong>
								<span className={cx(text.dim, text.mono)}> {theme.renderers.join(' ')}</span>
							</li>
						))}
					</ul>
				</section>
				<section className={ui.card} aria-labelledby="flows-h">
					<h2 id="flows-h">{t('project.flows')}</h2>
					<ul className={ui.plain}>
						{summary.flows.map((f) => (
							<li key={f.id}>
								<a
									href="#"
									onClick={(e) => {
										e.preventDefault();
										studio.project.setState({ flowId: f.id, flow: null });
										studio.go('flow');
									}}
								>
									{f.id}
								</a>
								<span className={f.errors ? text.err : text.dim}>
									{` ${tn('project.steps', f.steps)}${f.errors ? ` · ${tn('problems.errors', f.errors)}` : ''}`}
								</span>
							</li>
						))}
					</ul>
				</section>
			</div>
			<section ref={checks} className={ui.card} id="checks" tabIndex={-1} aria-labelledby="check-h">
				<h2 id="check-h">{t('project.checks')}</h2>
				{problems.length === 0 ? (
					<p className={text.muted}>{t('project.nothingToReport')}</p>
				) : (
					<ul className={ui.problems}>
						{problems.map((p, i) => (
							<li key={i} className={p.severity === 'error' ? ui.error : undefined}>
								<span className={cx(text.mono, text.small)}>{p.scope}</span> {p.message}
							</li>
						))}
					</ul>
				)}
			</section>
			<section className={ui.card} aria-labelledby="agent-h">
				<h2 id="agent-h">{t('project.agents')}</h2>
				{mcp ? (
					<div>
						<p className={text.muted}>{t('project.agentsIntro')}</p>
						<p>
							{t('project.endpoint')} <code>{mcp.url}</code>
						</p>
						<p className={cx(text.dim, text.small)}>{t('project.tokenNote')}</p>
					</div>
				) : (
					<p className={text.muted}>{t('project.mcpOff')}</p>
				)}
			</section>
		</div>
	);
}
