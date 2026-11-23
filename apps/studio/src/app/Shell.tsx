// The app shell: the sidebar, the status bar, the history popover and toast around the active screen.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Outlet } from '@tanstack/react-router';
import type { ReactNode } from 'react';
import type { Screen } from '../chrome/menuModel';
import { t, type MessageId } from '../i18n/messages';
import { errorCount, historyRows, problemSummary } from '../lib/format';
import { SCREEN_TITLE } from '../stores/studio';
import { cx } from '../ui/cx';
import { ICON_PATHS } from '../ui/iconPaths';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import { useProject, useStudio, useToast } from './StudioContext';
import { useShellOptions } from './ShellOptions';
import { useScreen } from './screens';
import styles from './Shell.module.css';

const NAV: { route: Screen; icon: string }[] = [
	{ route: 'project', icon: ICON_PATHS.folder },
	{ route: 'flow', icon: ICON_PATHS.flow },
	{ route: 'layouts', icon: ICON_PATHS.layouts },
	{ route: 'script', icon: ICON_PATHS.code },
	{ route: 'preview', icon: ICON_PATHS.eye },
];

function NavIcon({ d }: { d: string }) {
	return (
		<svg
			viewBox="0 0 16 16"
			width="16"
			height="16"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.5"
			strokeLinejoin="round"
			aria-hidden="true"
		>
			<path d={d} />
		</svg>
	);
}

function AgentCard() {
	const mcp = useProject((s) => s.mcp);
	const mcpError = useProject((s) => s.mcpError);
	const failed = !mcp && mcpError;
	const label: MessageId = mcp ? 'nav.agentOn' : failed ? 'nav.agentFailed' : 'nav.agentOff';
	return (
		<div className={cx(styles.agentCard, failed && styles.failed)} data-testid="agent-card">
			<span className={cx(ui.dot, mcp && ui.on)} />
			{t(label)}
			{mcp ? <code>{mcp.url}</code> : null}
			{failed ? (
				<span className={cx(text.err, text.small)} role="alert">
					{mcpError}
				</span>
			) : null}
			{failed ? <span className={cx(text.dim, text.small)}>{t('nav.agentChangePort')}</span> : null}
		</div>
	);
}

function HistoryPopover() {
	const entries = useProject((s) => s.history.entries);
	const rows = historyRows(entries);
	return (
		<div className={styles.popover} role="dialog" aria-label={t('shell.historyLabel')}>
			{rows.length === 0 ? (
				<p className={text.dim}>{t('shell.noChanges')}</p>
			) : (
				<ol>
					{rows.map((r, i) => (
						<li key={i} className={r.undone ? styles.undone : undefined}>
							<span>{r.label}</span>
							<span className={text.dim}>{` · ${r.source} · ${r.doc}`}</span>
						</li>
					))}
				</ol>
			)}
		</div>
	);
}

function StatusBar() {
	const studio = useStudio();
	const summary = useProject((s) => s.summary);
	const problems = useProject((s) => s.problems);
	const entries = useProject((s) => s.history.entries);
	const showHistory = useProject((s) => s.showHistory);
	const errors = errorCount(problems);
	const warnings = problems.length - errors;
	const applied = entries.filter((e) => e.applied).length;
	return (
		<footer className={styles.statusbar}>
			<span className={styles.statusbarPath} data-testid="status-path">
				{summary?.root ?? t('shell.noProjectOpen')}
			</span>
			<span className={styles.statusbarEnd} data-testid="status-end">
				{problems.length === 0 ? (
					<span>{t('shell.noProblems')}</span>
				) : (
					<button
						type="button"
						className={cx(styles.statusbarBtn, errors > 0 ? styles.err : styles.warn)}
						id="status-problems"
						title={t('shell.showChecks')}
						onClick={() => studio.showChecks()}
					>
						{problemSummary(errors, warnings)}
					</button>
				)}
				<button
					type="button"
					className={styles.statusbarBtn}
					aria-expanded={showHistory}
					onClick={() => studio.project.setState((s) => ({ showHistory: !s.showHistory }))}
				>
					{t('shell.history', { count: applied })}
				</button>
			</span>
		</footer>
	);
}

function Toast() {
	const toast = useToast((s) => s.toast);
	if (!toast) return null;
	return (
		<div
			className={cx(styles.toast, toast.error && styles.error)}
			role="status"
			data-testid="toast"
			data-error={toast.error}
		>
			{toast.text}
		</div>
	);
}

/** The page's own header, for a browser tab that has no title bar. */
function Topbar() {
	const studio = useStudio();
	const summary = useProject((s) => s.summary);
	const canUndo = useProject((s) => s.history.can_undo);
	const canRedo = useProject((s) => s.history.can_redo);
	const act = (id: 'edit.undo' | 'edit.redo') => void studio.app().dispatch(id);
	return (
		<header className={styles.topbar}>
			<span className={styles.brand}>{t('shell.title')}</span>
			<span className={styles.projectName}>{summary?.name ?? t('shell.noProject')}</span>
			<span className={styles.topbarActions}>
				<button
					type="button"
					className={cx(ui.btn, ui.ghost)}
					disabled={!canUndo}
					title={t('shell.undo')}
					onClick={() => act('edit.undo')}
				>
					{t('shell.undo')}
				</button>
				<button
					type="button"
					className={cx(ui.btn, ui.ghost)}
					disabled={!canRedo}
					title={t('shell.redo')}
					onClick={() => act('edit.redo')}
				>
					{t('shell.redo')}
				</button>
				<button
					type="button"
					className={cx(ui.btn, ui.ghost)}
					onClick={() => void studio.run(() => studio.openProject())}
				>
					{t('shell.open')}
				</button>
			</span>
		</header>
	);
}

export function Shell(): ReactNode {
	const studio = useStudio();
	const { header } = useShellOptions();
	const summary = useProject((s) => s.summary);
	const showHistory = useProject((s) => s.showHistory);
	const screen = useScreen();
	return (
		<div className={cx(styles.shell, header && styles.withHeader)}>
			{header ? <Topbar /> : null}
			{showHistory ? <HistoryPopover /> : null}
			<nav className={styles.sidebar} aria-label={t('shell.sidebarLabel')}>
				<div className={styles.navLabel}>{t('nav.project')}</div>
				{NAV.map((n) => (
					<a
						key={n.route}
						href="#"
						className={screen === n.route ? styles.active : undefined}
						aria-current={screen === n.route ? 'page' : undefined}
						onClick={(e) => {
							e.preventDefault();
							studio.go(n.route);
						}}
					>
						<NavIcon d={n.icon} />
						{t(SCREEN_TITLE[n.route])}
					</a>
				))}
				<AgentCard />
			</nav>
			<main className={styles.main}>
				{summary ? (
					<Outlet />
				) : (
					<section className={styles.empty}>
						<h1>{t('shell.openProjectHeading')}</h1>
						<p>{t('shell.openProjectHelp')}</p>
						<button
							type="button"
							className={cx(ui.btn, ui.primary)}
							onClick={() => void studio.run(() => studio.openProject())}
						>
							{t('shell.openProjectButton')}
						</button>
					</section>
				)}
			</main>
			<StatusBar />
			<Toast />
		</div>
	);
}
