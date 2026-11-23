// The Tauri screen: the import of tauri.conf.json, what drifted, and the install and update settings built on it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tn, type MessageId } from '../i18n/messages';
import { sortProblems } from '../lib/tauri';
import type { TauriImportResult, TauriView } from '../lib/types';
import { useProject, useStudio, useTauri } from '../app/StudioContext';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import styles from './TauriScreen.module.css';
import { IdentityCard, UpdatesCard, WindowsCard, Webview2Card } from './TauriSettings';

const DRIFT_KIND: Record<'changed' | 'added' | 'removed', MessageId> = {
	changed: 'tauri.drift.changed',
	added: 'tauri.drift.added',
	removed: 'tauri.drift.removed',
};

function ImportSummary({ result }: { result: TauriImportResult }) {
	const nothing =
		result.changes.length === 0 &&
		result.filled.length === 0 &&
		result.kept.length === 0 &&
		result.warnings.length === 0;
	return (
		<div aria-live="polite" data-testid="import-summary">
			<h3>{t(result.first_import ? 'tauri.import.firstDone' : 'tauri.import.done')}</h3>
			{nothing ? <p className={text.muted}>{t('tauri.import.nothing')}</p> : null}
			<ul className={ui.plain}>
				{result.changes.length > 0 ? (
					<li>{tn('tauri.import.changes', result.changes.length)}</li>
				) : null}
				{result.filled.length > 0 ? (
					<li>{t('tauri.import.filled', { fields: result.filled.join(', ') })}</li>
				) : null}
				{result.kept.length > 0 ? (
					<li>{t('tauri.import.kept', { fields: result.kept.join(', ') })}</li>
				) : null}
				{result.components.length > 0 ? (
					<li>{t('tauri.import.components', { components: result.components.join(', ') })}</li>
				) : null}
			</ul>
			{result.warnings.length > 0 ? (
				<>
					<h4>{tn('tauri.import.warnings', result.warnings.length)}</h4>
					<ul className={ui.problems}>
						{result.warnings.map((w, i) => (
							<li key={i}>{w}</li>
						))}
					</ul>
				</>
			) : null}
		</div>
	);
}

function ImportCard(props: {
	view: TauriView;
	busy: boolean;
	lastImport: TauriImportResult | null;
	onPick: () => void;
	onReimport: () => void;
}) {
	const { view, busy, lastImport } = props;
	return (
		<section className={ui.card} aria-labelledby="tauri-import-h">
			<h2 id="tauri-import-h">{t('tauri.import.title')}</h2>
			<p>
				{t('tauri.import.config')}{' '}
				{view.config ? (
					<code>{view.config}</code>
				) : (
					<span className={text.dim}>{t('tauri.import.noConfig')}</span>
				)}
			</p>
			<div className={styles.status}>
				{view.config ? (
					<span className={cx(ui.pill, view.config_found ? ui.pillOk : ui.pillError)}>
						{t(view.config_found ? 'tauri.import.found' : 'tauri.import.notFound')}
					</span>
				) : null}
				<span className={cx(ui.pill, view.imported ? ui.pillOk : ui.pillWarning)}>
					{t(view.imported ? 'tauri.import.imported' : 'tauri.import.notImported')}
				</span>
			</div>
			<div className={ui.toolbar}>
				<button
					type="button"
					className={cx(ui.btn, ui.primary)}
					disabled={busy}
					onClick={props.onPick}
				>
					{t('tauri.import.pick')}
				</button>
				<button
					type="button"
					className={ui.btn}
					disabled={busy || !view.config}
					onClick={props.onReimport}
				>
					{t('tauri.import.again')}
				</button>
			</div>
			{lastImport ? <ImportSummary result={lastImport} /> : null}
		</section>
	);
}

function DriftCard({
	view,
	busy,
	onReimport,
}: {
	view: TauriView;
	busy: boolean;
	onReimport: () => void;
}) {
	return (
		<section className={ui.card} aria-labelledby="tauri-drift-h">
			<div className={styles.cardHead}>
				<h2 id="tauri-drift-h">{t('tauri.drift.title')}</h2>
				<span className={cx(ui.pill, view.stale ? ui.pillWarning : ui.pillOk)}>
					{t(view.stale ? 'tauri.drift.stale' : 'tauri.drift.current')}
				</span>
			</div>
			{!view.imported ? (
				<p className={text.muted}>{t('tauri.drift.notImported')}</p>
			) : !view.stale ? (
				<p className={text.muted}>{t('tauri.drift.currentHelp')}</p>
			) : (
				<>
					<p className={text.muted}>{t('tauri.drift.staleHelp')}</p>
					<ul className={ui.plain} aria-label={t('tauri.drift.files')}>
						{view.drift.map((d) => (
							<li key={d.file}>
								<code>{d.file}</code> <span className={text.dim}>{t(DRIFT_KIND[d.kind])}</span>
							</li>
						))}
					</ul>
					{view.pending.length > 0 ? (
						<table>
							<thead>
								<tr>
									<th scope="col">{t('tauri.drift.col.field')}</th>
									<th scope="col">{t('tauri.drift.col.from')}</th>
									<th scope="col">{t('tauri.drift.col.to')}</th>
								</tr>
							</thead>
							<tbody>
								{view.pending.map((c) => (
									<tr key={c.field}>
										<th scope="row" className={text.mono}>
											{c.field}
										</th>
										<td className={text.mono}>{c.from ?? '—'}</td>
										<td className={text.mono}>{c.to ?? '—'}</td>
									</tr>
								))}
							</tbody>
						</table>
					) : (
						<p className={text.dim}>{t('tauri.drift.noValueChanges')}</p>
					)}
					<div className={ui.toolbar}>
						<button type="button" className={ui.btn} disabled={busy} onClick={onReimport}>
							{t('tauri.import.again')}
						</button>
					</div>
				</>
			)}
		</section>
	);
}

function ProblemsCard({ view }: { view: TauriView }) {
	const problems = sortProblems(view.problems);
	return (
		<section className={ui.card} aria-labelledby="tauri-problems-h">
			<h2 id="tauri-problems-h">{t('tauri.problems.title')}</h2>
			{problems.length === 0 ? (
				<p className={text.muted}>{t('tauri.problems.none')}</p>
			) : (
				<ul className={ui.problems}>
					{problems.map((p, i) => (
						<li key={i} className={p.severity === 'error' ? ui.error : undefined}>
							<strong className={styles.severity}>
								{t(p.severity === 'error' ? 'tauri.problems.error' : 'tauri.problems.warning')}
							</strong>
							<span className={cx(text.mono, text.small)}>{p.scope}</span> {p.message}
						</li>
					))}
				</ul>
			)}
		</section>
	);
}

export function TauriScreen() {
	const studio = useStudio();
	const summary = useProject((s) => s.summary)!;
	const view = useTauri((s) => s.view);
	const loading = useTauri((s) => s.loading);
	const busy = useTauri((s) => s.busy);
	const error = useTauri((s) => s.error);
	const lastImport = useTauri((s) => s.lastImport);

	const pick = () =>
		void studio.run(async () => {
			const path = await studio.api.pickTauriConfig(summary.root);
			if (path) await studio.tauri.importConfig(path);
		});
	const reimport = () => void studio.tauri.importConfig(null);
	const apply = (change: Parameters<typeof studio.tauri.setSettings>[0], label?: string) =>
		void studio.tauri.setSettings(change, label);

	return (
		<div className={ui.screen}>
			<div className={ui.screenHead}>
				<div>
					<h1>{t('tauri.title')}</h1>
					<p className={text.muted}>{t('tauri.intro')}</p>
				</div>
			</div>
			{error ? (
				<p className={text.err} role="alert">
					{error}
				</p>
			) : null}
			{!view ? (
				<p className={text.muted}>{loading ? t('tauri.loading') : t('tauri.unavailable')}</p>
			) : !view.configured ? (
				<section className={cx(ui.card, styles.empty)} aria-labelledby="tauri-empty-h">
					<h2 id="tauri-empty-h">{t('tauri.empty.title')}</h2>
					<p className={text.muted}>{t('tauri.empty.help')}</p>
					<button type="button" className={cx(ui.btn, ui.primary)} disabled={busy} onClick={pick}>
						{t('tauri.import.pick')}
					</button>
					{lastImport ? <ImportSummary result={lastImport} /> : null}
				</section>
			) : (
				<>
					<ImportCard
						view={view}
						busy={busy}
						lastImport={lastImport}
						onPick={pick}
						onReimport={reimport}
					/>
					<DriftCard view={view} busy={busy} onReimport={reimport} />
					<IdentityCard view={view} busy={busy} apply={apply} />
					<WindowsCard view={view} busy={busy} apply={apply} />
					<Webview2Card view={view} busy={busy} apply={apply} />
					<UpdatesCard view={view} busy={busy} apply={apply} />
					<ProblemsCard view={view} />
				</>
			)}
		</div>
	);
}
