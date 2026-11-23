// The pages of Preferences: each is rows over the settings, applied at once.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useState, type ReactNode } from 'react';
import { ButtonRow } from '@liminal-hq/studio-chrome/SettingsShell/ButtonRow';
import { NumberRow, type NumberRowProps } from '@liminal-hq/studio-chrome/SettingsShell/NumberRow';
import { SegmentedRow } from '@liminal-hq/studio-chrome/SettingsShell/SegmentedRow';
import { SelectRow } from '@liminal-hq/studio-chrome/SettingsShell/SelectRow';
import { SettingsGroup } from '@liminal-hq/studio-chrome/SettingsShell/SettingsGroup';
import { SettingsRow } from '@liminal-hq/studio-chrome/SettingsShell/SettingsRow';
import { TextRow } from '@liminal-hq/studio-chrome/SettingsShell/TextRow';
import { ToggleRow } from '@liminal-hq/studio-chrome/SettingsShell/ToggleRow';
import { t, tn } from '../i18n/messages';
import type { AgentInfo, AppInfo, Host } from '../lib/host';
import {
	LIMITS,
	type Density,
	type MotionSetting,
	type SettingKey,
	type Settings,
	type ThemeId,
} from '../lib/settings';
import ui from '../ui/ui.module.css';
import { refusal, type Failures } from './usePreferences';

/** What a page needs: the settings in force, why a write was refused, and the ways to change them. */
export interface PageProps {
	settings: Settings;
	failures: Failures;
	update(patch: Partial<Settings>): Promise<boolean>;
	reset(): Promise<boolean>;
	host: Host;
	/** The full ids of the open project's targets, for the dry-run default. */
	targets: string[];
	/** Changes when Escape puts typed text back, so a number box drops its unapplied draft. */
	epoch: number;
}

/** The names the path boxes suggest. They are program names, not words to translate. */
const RUNNER_BINARY = 'liminal-engine';
const DOCKER_BINARY = 'docker';

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** A number box that applies when it is left or Enter is pressed, and drops what was typed on Escape. */
function Num({ epoch, ...props }: NumberRowProps & { epoch: number }) {
	return <NumberRow key={epoch} commitOn="commit" {...props} />;
}

/** One row for a number setting, with its bounds. */
function numberRow(
	p: PageProps,
	key: Exclude<SettingKey, 'version'>,
	label: string,
	description: string,
	unit?: string,
) {
	const bounds = (LIMITS as Record<string, readonly [number, number]>)[key]!;
	return (
		<Num
			epoch={p.epoch}
			label={label}
			description={description}
			value={p.settings[key] as number}
			min={bounds[0]}
			max={bounds[1]}
			step={1}
			{...(unit ? { unit } : {})}
			onChange={(v) => void p.update({ [key]: v })}
			{...refusal(p.failures, key)}
		/>
	);
}

function toggle(
	p: PageProps,
	key:
		| 'menuBar'
		| 'reopenLastProject'
		| 'confirmDiscard'
		| 'wrapLines'
		| 'lineNumbers'
		| 'formatOnSave'
		| 'previewAutoRun'
		| 'showSkippedSteps'
		| 'mcpEnabled',
	label: string,
	description: string,
) {
	return (
		<ToggleRow
			label={label}
			description={description}
			checked={p.settings[key]}
			onChange={(v) => void p.update({ [key]: v })}
			{...refusal(p.failures, key)}
		/>
	);
}

export function GeneralPage(p: PageProps) {
	const count = p.settings.recentProjects.length;
	return (
		<>
			<SettingsGroup title={t('prefs.group.window')}>
				{toggle(p, 'menuBar', t('prefs.menuBar.label'), t('prefs.menuBar.desc'))}
			</SettingsGroup>
			<SettingsGroup title={t('prefs.group.projects')}>
				{toggle(p, 'reopenLastProject', t('prefs.reopen.label'), t('prefs.reopen.desc'))}
				{toggle(
					p,
					'confirmDiscard',
					t('prefs.confirmDiscard.label'),
					t('prefs.confirmDiscard.desc'),
				)}
			</SettingsGroup>
			<SettingsGroup title={t('prefs.group.recent')}>
				{numberRow(p, 'recentLimit', t('prefs.recentLimit.label'), t('prefs.recentLimit.desc'))}
				<ButtonRow
					label={t('prefs.group.recent')}
					description={count === 0 ? t('prefs.recent.empty') : tn('prefs.recent.count', count)}
					actionLabel={t('prefs.recent.clear')}
					disabled={count === 0}
					onAction={() => void p.update({ recentProjects: [] })}
					{...refusal(p.failures, 'recentProjects')}
				/>
			</SettingsGroup>
		</>
	);
}

const THEME_OPTIONS: { value: ThemeId; label: string }[] = [
	{ value: 'afterglow', label: t('prefs.theme.dark') },
	{ value: 'afterglow-light', label: t('prefs.theme.light') },
	{ value: 'system', label: t('prefs.theme.system') },
	{ value: 'afterglow-dark', label: t('prefs.theme.neutral') },
];

export function AppearancePage(p: PageProps) {
	return (
		<>
			<SettingsGroup title={t('prefs.group.theme')}>
				<SelectRow
					label={t('prefs.group.theme')}
					description={t('prefs.theme.desc')}
					value={p.settings.theme}
					options={THEME_OPTIONS}
					onChange={(v) => void p.update({ theme: v })}
					{...refusal(p.failures, 'theme')}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('prefs.group.layout')}>
				<SegmentedRow<Density>
					label={t('prefs.density.label')}
					description={t('prefs.density.desc')}
					value={p.settings.density}
					options={[
						{ value: 'comfortable', label: t('prefs.density.comfortable') },
						{ value: 'compact', label: t('prefs.density.compact') },
					]}
					onChange={(v) => void p.update({ density: v })}
					{...refusal(p.failures, 'density')}
				/>
				{numberRow(p, 'textSize', t('prefs.textSize.label'), t('prefs.textSize.desc'), '%')}
			</SettingsGroup>
			<SettingsGroup title={t('prefs.group.motion')}>
				<SegmentedRow<MotionSetting>
					label={t('prefs.motion.label')}
					description={t('prefs.motion.desc')}
					value={p.settings.reduceMotion}
					options={[
						{ value: 'system', label: t('prefs.theme.system') },
						{ value: 'on', label: t('prefs.motion.on') },
						{ value: 'off', label: t('prefs.motion.off') },
					]}
					onChange={(v) => void p.update({ reduceMotion: v })}
					{...refusal(p.failures, 'reduceMotion')}
				/>
			</SettingsGroup>
		</>
	);
}

export function EditorPage(p: PageProps) {
	return (
		<>
			<SettingsGroup title={t('prefs.group.editor')} description={t('prefs.group.editorDesc')}>
				{numberRow(p, 'editorFontSize', t('prefs.fontSize.label'), t('prefs.fontSize.desc'), 'px')}
				{numberRow(p, 'tabWidth', t('prefs.tabWidth.label'), t('prefs.tabWidth.desc'), 'columns')}
				{toggle(p, 'wrapLines', t('prefs.wrap.label'), t('prefs.wrap.desc'))}
				{toggle(p, 'lineNumbers', t('prefs.lineNumbers.label'), t('prefs.lineNumbers.desc'))}
				{toggle(p, 'formatOnSave', t('prefs.formatOnSave.label'), t('prefs.formatOnSave.desc'))}
			</SettingsGroup>
		</>
	);
}

export function PreviewPage(p: PageProps) {
	const current = p.settings.previewTarget;
	const known = current === '' || p.targets.includes(current);
	return (
		<SettingsGroup title={t('prefs.group.dryRun')}>
			<SelectRow
				label={t('prefs.previewTarget.label')}
				description={t('prefs.previewTarget.desc')}
				value={current}
				options={[
					{ value: '', label: t('prefs.previewTarget.first') },
					...p.targets.map((t) => ({ value: t, label: t })),
					...(known
						? []
						: [{ value: current, label: t('prefs.previewTarget.missing', { target: current }) }]),
				]}
				onChange={(v) => void p.update({ previewTarget: v })}
				{...refusal(p.failures, 'previewTarget')}
			/>
			{toggle(p, 'previewAutoRun', t('prefs.autoRun.label'), t('prefs.autoRun.desc'))}
			{toggle(p, 'showSkippedSteps', t('prefs.skipped.label'), t('prefs.skipped.desc'))}
		</SettingsGroup>
	);
}

export function AgentsPage(p: PageProps) {
	const { host, settings } = p;
	const [agent, setAgent] = useState<AgentInfo | null>(null);
	const [startError, setStartError] = useState<string | null>(null);
	const [token, setToken] = useState<string | null>(null);
	const [notice, setNotice] = useState<{ key: 'token' | 'copy' | 'regen'; text: string } | null>(
		null,
	);
	useEffect(() => {
		let live = true;
		setToken(null);
		void host.agent().then(
			(a) => live && setAgent(a),
			() => live && setAgent(null),
		);
		void host.agentError().then(
			(e) => live && setStartError(e),
			() => live && setStartError(null),
		);
		return () => {
			live = false;
		};
	}, [host, settings.mcpEnabled, settings.mcpPort]);
	const running = agent !== null;
	const fail = (key: 'token' | 'copy' | 'regen') => (e: unknown) =>
		setNotice({ key, text: message(e) });
	const error = (key: 'token' | 'copy' | 'regen') =>
		notice?.key === key ? { error: notice.text } : {};
	const fetchToken = async () => {
		const token = await host.agentToken();
		if (token === null) throw new Error(t('agent.serverOff'));
		return token;
	};
	return (
		<>
			<SettingsGroup
				title={t('prefs.group.agentServer')}
				description={t('prefs.group.agentServerDesc')}
			>
				{toggle(p, 'mcpEnabled', t('prefs.mcp.label'), t('prefs.mcp.desc'))}
				{numberRow(p, 'mcpPort', t('prefs.port.label'), t('prefs.port.desc'))}
				<SettingsRow
					label={t('prefs.status.label')}
					description={
						running
							? agent.url
							: settings.mcpEnabled
								? t('prefs.status.notRunning')
								: t('prefs.status.off')
					}
					{...(!running && settings.mcpEnabled && startError
						? { error: t('prefs.status.didNotStart', { why: startError }) }
						: {})}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('prefs.group.token')} description={t('prefs.group.tokenDesc')}>
				<SettingsRow
					label={t('prefs.token.label')}
					description={token ?? t('prefs.token.hidden')}
					{...error('token')}
				>
					<button
						type="button"
						className={ui.btn}
						disabled={!running}
						onClick={() => {
							if (token !== null) return setToken(null);
							setNotice(null);
							fetchToken().then(setToken, fail('token'));
						}}
					>
						{token !== null ? t('prefs.token.hide') : t('prefs.token.show')}
					</button>
				</SettingsRow>
				<ButtonRow
					label={t('prefs.copyToken.label')}
					description={
						notice?.key === 'copy' && notice.text === t('prefs.copied')
							? t('prefs.copyToken.copied')
							: t('prefs.copyToken.desc')
					}
					actionLabel={t('prefs.copyToken.action')}
					disabled={!running}
					onAction={() => {
						setNotice(null);
						fetchToken()
							.then((value) => p.host.copy(value))
							.then(() => setNotice({ key: 'copy', text: t('prefs.copied') }), fail('copy'));
					}}
					{...(notice?.key === 'copy' && notice.text !== t('prefs.copied')
						? { error: notice.text }
						: {})}
				/>
				<ButtonRow
					label={t('prefs.regen.label')}
					description={t('prefs.regen.desc')}
					actionLabel={t('prefs.regen.action')}
					danger
					disabled={!running}
					onAction={() => {
						setNotice(null);
						void (async () => {
							const answer = await host.confirm({
								kind: 'confirm',
								title: t('prefs.regen.label'),
								message: t('prefs.regen.message'),
								detail: t('prefs.regen.detail'),
								buttons: [
									{ id: 'regenerate', label: t('prefs.regen.action'), primary: true, danger: true },
									{ id: 'cancel', label: t('common.cancel') },
								],
							});
							if (answer !== 'regenerate') return;
							const fresh = await host.regenerateToken();
							setToken((shown) => (shown === null ? null : fresh));
						})().catch(fail('regen'));
					}}
					{...error('regen')}
				/>
			</SettingsGroup>
		</>
	);
}

/** A text row for a path, with a button that asks for it in a dialog. */
function PathRows({
	p,
	name,
	label,
	description,
	kind,
	title,
	placeholder,
}: {
	p: PageProps;
	name: 'projectsFolder' | 'runnerPath' | 'dockerPath';
	label: string;
	description: ReactNode;
	kind: 'folder' | 'file';
	title: string;
	placeholder?: string;
}) {
	const [browseError, setBrowseError] = useState<string | undefined>();
	const value = p.settings[name];
	return (
		<>
			<TextRow
				key={p.epoch}
				label={label}
				description={description}
				value={value}
				size={34}
				{...(placeholder ? { placeholder } : {})}
				onCommit={(v) => void p.update({ [name]: v.trim() })}
				{...refusal(p.failures, name)}
			/>
			<ButtonRow
				label={t('prefs.browse.for', { name: label.toLowerCase() })}
				actionLabel={t('prefs.browse.action')}
				onAction={() => {
					setBrowseError(undefined);
					p.host
						.browse(kind, title, value || undefined)
						.then((picked) => (picked ? p.update({ [name]: picked }) : undefined))
						.catch((e) => setBrowseError(message(e)));
				}}
				{...(browseError ? { error: browseError } : {})}
			/>
		</>
	);
}

export function ProjectsPage(p: PageProps) {
	return (
		<>
			<SettingsGroup title={t('prefs.group.folders')}>
				<PathRows
					p={p}
					name="projectsFolder"
					label={t('prefs.projectsFolder.label')}
					kind="folder"
					title={t('prefs.projectsFolder.title')}
					placeholder={t('prefs.projectsFolder.placeholder')}
					description={t('prefs.projectsFolder.desc')}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('prefs.group.tools')}>
				<PathRows
					p={p}
					name="runnerPath"
					label={t('prefs.runner.label')}
					kind="file"
					title={t('prefs.runner.title')}
					placeholder={RUNNER_BINARY}
					description={t('prefs.runner.desc')}
				/>
				<PathRows
					p={p}
					name="dockerPath"
					label={t('prefs.docker.label')}
					kind="file"
					title={t('prefs.docker.title')}
					placeholder={DOCKER_BINARY}
					description={t('prefs.docker.desc')}
				/>
			</SettingsGroup>
		</>
	);
}

export function AdvancedPage(p: PageProps) {
	const { host } = p;
	const [info, setInfo] = useState<AppInfo | null>(null);
	const [notice, setNotice] = useState<{ key: string; text: string } | null>(null);
	useEffect(() => {
		let live = true;
		void host.appInfo().then(
			(i) => live && setInfo(i),
			() => undefined,
		);
		return () => {
			live = false;
		};
	}, [host]);
	const attempt = (key: string, work: () => Promise<unknown>) => {
		setNotice(null);
		work().catch((e) => setNotice({ key, text: message(e) }));
	};
	const error = (key: string) => (notice?.key === key ? { error: notice.text } : {});
	return (
		<>
			<SettingsGroup title={t('prefs.group.files')}>
				<ButtonRow
					label={t('prefs.settingsFile.label')}
					description={info?.settingsFile ?? t('prefs.settingsFile.desc')}
					actionLabel={t('prefs.settingsFile.action')}
					onAction={() => attempt('settings', () => host.reveal('settings'))}
					{...error('settings')}
				/>
				<ButtonRow
					label={t('prefs.logs.label')}
					description={info?.logDir ?? t('prefs.logs.desc')}
					actionLabel={t('prefs.logs.action')}
					onAction={() => attempt('logs', () => host.reveal('logs'))}
					{...error('logs')}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('prefs.group.reset')}>
				<ButtonRow
					label={t('prefs.reset.label')}
					description={t('prefs.reset.desc')}
					actionLabel={t('prefs.reset.action')}
					danger
					onAction={() =>
						attempt('reset', async () => {
							const answer = await host.confirm({
								kind: 'confirm',
								title: t('prefs.reset.title'),
								message: t('prefs.reset.message'),
								detail: t('prefs.reset.detail'),
								buttons: [
									{ id: 'reset', label: t('prefs.reset.confirm'), primary: true, danger: true },
									{ id: 'cancel', label: t('common.cancel') },
								],
							});
							if (answer === 'reset') await p.reset();
						})
					}
					{...error('reset')}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('prefs.group.about')}>
				<SettingsRow
					label={t('prefs.version.label')}
					description={info ? t('prefs.version.value', { version: info.version }) : t('app.name')}
				/>
			</SettingsGroup>
		</>
	);
}
