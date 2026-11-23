// The Script screen: a Lua editor with completion and diagnostics, the hook and capability lists, and the test console.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useState } from 'react';
import { countFindings } from '../editor/diagnostics';
import { HOOK_PARAMS, hookSignature } from '../editor/luaDocs';
import { t, tn, type MessageId } from '../i18n/messages';
import type { ScriptDiagnostic } from '../lib/types';
import { isStale, type ScriptState } from '../stores/script';
import { useChromeState, useProject, useScript, useStudio } from '../app/StudioContext';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import { ScriptEditorView } from './ScriptEditorView';
import styles from './ScriptScreen.module.css';

const CAPABILITY_NOTES: Record<string, MessageId> = {
	'li.env': 'script.cap.env',
	'li.fs': 'script.cap.fs',
	'li.ui': 'script.cap.ui',
	'li.plan': 'script.cap.plan',
	'li.net': 'script.cap.net',
	'li.retro.sound': 'script.cap.sound',
	'li.retro.video': 'script.cap.video',
};

const clock = (d: Date) =>
	[d.getHours(), d.getMinutes(), d.getSeconds()].map((n) => String(n).padStart(2, '0')).join(':');

/** The line beside the Test run button. */
export function testStatus(
	s: Pick<ScriptState, 'testRun' | 'scriptDraft' | 'scriptTarget'>,
): string {
	const run = s.testRun;
	if (!run) return '';
	if (run.status === 'running') return t('script.running');
	const when = clock(run.at ?? new Date());
	const base =
		run.status === 'error'
			? t('script.couldNotRunAt', { when })
			: t('script.testedAt', {
					when,
					ms: run.ms ?? 0,
					source: run.unsaved ? t('script.unsavedText') : t('script.savedFile'),
				});
	return isStale(s) ? `${base} · ${t('script.outOfDate')}` : base;
}

/** "2 errors, 1 warning" for the editor's footer, or "No problems". */
function problemsText(found: ScriptDiagnostic[]): string {
	const { errors, warnings } = countFindings(found);
	if (errors === 0 && warnings === 0) return t('editor.status.clean');
	return [
		errors ? tn('problems.errors', errors) : '',
		warnings ? tn('problems.warnings', warnings) : '',
	]
		.filter(Boolean)
		.join(', ');
}

function Console() {
	const run = useScript((s) => s.testRun);
	const draft = useScript((s) => s.scriptDraft);
	const target = useScript((s) => s.scriptTarget);
	const stale = isStale({ testRun: run, scriptDraft: draft, scriptTarget: target });
	const where = run
		? run.target
			? t('script.onTarget', { target: run.target })
			: t('script.onSample')
		: '';
	const body: React.ReactNode[] = [];
	if (!run) {
		body.push(
			<p key="none" className={cx(text.dim, text.small)}>
				{t('script.notRun')}
			</p>,
		);
	} else if (run.status === 'running') {
		body.push(
			<p key="running" className={text.dim}>
				{t('script.running')}
			</p>,
		);
	} else if (run.status === 'error') {
		body.push(
			<p key="error" className={styles.err} role="alert">
				✗ {run.error ?? t('script.testCouldNotRun')}
			</p>,
		);
	} else if (run.report) {
		const r = run.report;
		const tests = r.tests ?? [];
		tests.forEach((test, i) => {
			body.push(
				<div key={`t${i}`} className={cx(styles.line, test.ok ? styles.ok : styles.err)}>
					<span className={styles.mark}>{test.ok ? '✓' : '✗'}</span>
					{` ${test.hook}(${HOOK_PARAMS[test.hook] ?? 'page'}) — ${test.summary}`}
				</div>,
			);
			test.lines.forEach((line, j) =>
				body.push(
					<div key={`t${i}l${j}`} className={cx(styles.line, styles.sub, text.dim)}>
						{line}
					</div>,
				),
			);
		});
		if (tests.length === 0)
			r.output.forEach((line, i) =>
				body.push(
					<div key={`o${i}`} className={styles.line}>
						{line}
					</div>,
				),
			);
		const hookErrors = new Set(tests.filter((x) => !x.ok).map((x) => x.summary));
		r.diagnostics.forEach((d, i) => {
			if (d.severity === 'error' && [...hookErrors].some((m) => d.message.endsWith(m))) return;
			body.push(
				<div
					key={`d${i}`}
					className={cx(styles.line, d.severity === 'error' ? styles.err : styles.warn)}
				>
					<span className={styles.mark}>{d.severity === 'error' ? '✗' : '!'}</span>
					{` ${d.message}`}
				</div>,
			);
		});
		const passed = tests.filter((x) => x.ok).length;
		const failed = tests.length - passed;
		const errors = r.diagnostics.filter((d) => d.severity === 'error').length;
		body.push(
			<div key="summary" className={cx(styles.summary, r.ok ? styles.ok : styles.err)}>
				{r.ok
					? t('script.passed', { passed })
					: t('script.failedPassed', { failed: failed || errors, passed })}
			</div>,
		);
	}
	return (
		<section
			id="script-console"
			className={cx(styles.console, stale && styles.stale)}
			aria-label={t('script.testConsole')}
		>
			<div>
				<span className={cx(text.dim, text.small)}>
					{where ? t('script.consoleWhere', { where }) : t('script.console')}
				</span>
			</div>
			<p id="script-stale" className={cx(styles.warn, text.small)} hidden={!stale}>
				{t('script.staleNote')}
			</p>
			<div className={styles.consoleBody} aria-live="polite">
				{body}
			</div>
		</section>
	);
}

export function ScriptScreen() {
	const studio = useStudio();
	const summary = useProject((s) => s.summary)!;
	const scriptPath = useScript((s) => s.scriptPath);
	const draft = useScript((s) => s.scriptDraft);
	const dirty = useScript((s) => s.scriptDirty);
	const target = useScript((s) => s.scriptTarget);
	const run = useScript((s) => s.testRun);
	const report = useScript((s) => s.report);
	const { settings } = useChromeState();
	const [position, setPosition] = useState({ line: 1, column: 1 });
	const [found, setFound] = useState<ScriptDiagnostic[]>([]);
	const [tabLeaves, setTabLeaves] = useState(false);
	const busy = run?.status === 'running';
	const targets = summary.targets.map((x) => x.full_id);

	const newScript = () => {
		const name = window.prompt(t('script.newPrompt'), 'my-script');
		if (name && /^[A-Za-z0-9_-]+$/.test(name))
			void studio.run(async () => {
				const path = `scripts/${name}.lua`;
				await studio.api.writeScript(
					path,
					`-- ${name}\n--\n-- capabilities: env\n\nfunction detect(page)\nend\n`,
					t('history.createScript', { path }),
				);
				await studio.selectScript(path);
			});
	};

	return (
		<div className={ui.screen}>
			<div className={ui.screenHead}>
				<div>
					<h1>{t('script.title')}</h1>
					<p className={text.muted}>{t('script.intro')}</p>
				</div>
			</div>
			<div className={ui.toolbar}>
				<select
					aria-label={t('script.script')}
					value={scriptPath}
					onChange={(e) => void studio.run(() => studio.selectScript(e.target.value))}
				>
					{summary.scripts.length === 0 ? (
						<option>{t('script.noScripts')}</option>
					) : (
						summary.scripts.map((p) => (
							<option key={p} value={p}>
								{p}
							</option>
						))
					)}
				</select>
				<button type="button" className={ui.btn} onClick={newScript}>
					{t('script.new')}
				</button>
				<button
					type="button"
					className={cx(ui.btn, ui.primary)}
					disabled={!scriptPath}
					title={t('script.saveTitle')}
					onClick={() => void studio.run(() => studio.saveScript())}
				>
					{t('common.save')}
				</button>
				<button
					type="button"
					className={ui.btn}
					disabled={!scriptPath}
					title={t('script.formatTitle')}
					onClick={() => studio.formatDraft()}
				>
					{t('script.format')}
				</button>
				<label className={cx(text.dim, text.small, styles.targetPick)}>
					{t('script.testOn')}{' '}
					<select
						aria-label={t('script.testTarget')}
						disabled={!scriptPath}
						value={target}
						onChange={(e) => studio.script.setState({ scriptTarget: e.target.value })}
					>
						<option value="">{t('script.sampleMachine')}</option>
						{targets.map((id) => (
							<option key={id} value={id}>
								{id}
							</option>
						))}
					</select>
				</label>
				<button
					type="button"
					className={ui.btn}
					disabled={!scriptPath || busy}
					title={t('script.testRunTitle')}
					onClick={() => void studio.run(() => studio.testScript())}
				>
					{busy ? t('script.running') : t('script.testRun')}
				</button>
				<span id="script-test-status" className={cx(text.dim, text.small)} role="status">
					{testStatus({ testRun: run, scriptDraft: draft, scriptTarget: target })}
				</span>
				<span id="script-dirty" className={cx(text.dim, text.small)}>
					{dirty ? t('script.unsavedChanges') : t('script.saved')}
				</span>
			</div>
			<div className={styles.scriptGrid}>
				<div className={styles.scriptMain}>
					{scriptPath ? (
						<ScriptEditorView
							key={scriptPath}
							studio={studio}
							draft={draft}
							settings={{
								fontSize: settings.editorFontSize,
								tabWidth: settings.tabWidth,
								wrap: settings.wrapLines,
								lineNumbers: settings.lineNumbers,
							}}
							label={t('editor.label', { path: scriptPath })}
							onCursor={(line, column) => setPosition({ line, column })}
							onDiagnostics={setFound}
							onTabFocus={setTabLeaves}
						/>
					) : (
						<p className={text.muted}>{t('script.none')}</p>
					)}
					{scriptPath ? (
						<div className={cx(styles.editorFoot, text.dim, text.small)}>
							<span id="script-pos">{t('editor.status.position', position)}</span>
							<span>{t('editor.status.lua')}</span>
							<span
								id="script-problems"
								className={found.some((d) => d.severity === 'error') ? styles.err : undefined}
							>
								{problemsText(found)}
							</span>
							<span id="script-tab-hint">{tabLeaves ? t('editor.tabFocus') : ''}</span>
						</div>
					) : null}
					{scriptPath ? <Console /> : null}
				</div>
				<aside className={ui.card}>
					<h3>{t('script.hooks')}</h3>
					<ul className={ui.plain}>
						{(report?.hooks ?? []).map((k) => (
							<li key={k.name}>
								<span className={cx(ui.dot, k.present && ui.on)} />
								<code>{hookSignature(k.name)}</code>
								<span className={cx(text.dim, text.small)}> {k.when}</span>
							</li>
						))}
					</ul>
					<h3>{t('script.capabilities')}</h3>
					{(report?.capabilities.length ?? 0) === 0 ? (
						<p className={cx(text.dim, text.small)}>{t('script.noneRequested')}</p>
					) : (
						<ul className={ui.plain}>
							{report!.capabilities.map((c) => (
								<li key={c}>
									<code>{c}</code>
									<div className={cx(text.dim, text.small)}>
										{CAPABILITY_NOTES[c] ? t(CAPABILITY_NOTES[c]) : t('script.unknownModule')}
									</div>
								</li>
							))}
						</ul>
					)}
				</aside>
			</div>
		</div>
	);
}
