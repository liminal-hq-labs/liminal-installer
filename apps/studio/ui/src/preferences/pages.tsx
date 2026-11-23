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
import type { AgentInfo, AppInfo, Host } from '../lib/host';
import { LIMITS, type Density, type MotionSetting, type SettingKey, type Settings, type ThemeId } from '../lib/settings';
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

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** A number box that applies when it is left or Enter is pressed, and drops what was typed on Escape. */
function Num({ epoch, ...props }: NumberRowProps & { epoch: number }) {
	return <NumberRow key={epoch} commitOn="commit" {...props} />;
}

/** One row for a number setting, with its bounds. */
function numberRow(p: PageProps, key: Exclude<SettingKey, 'version'>, label: string, description: string, unit?: string) {
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

function toggle(p: PageProps, key: 'menuBar' | 'reopenLastProject' | 'confirmDiscard' | 'wrapLines' | 'lineNumbers' | 'formatOnSave' | 'previewAutoRun' | 'showSkippedSteps' | 'mcpEnabled', label: string, description: string) {
	return <ToggleRow label={label} description={description} checked={p.settings[key]} onChange={(v) => void p.update({ [key]: v })} {...refusal(p.failures, key)} />;
}

export function GeneralPage(p: PageProps) {
	const count = p.settings.recentProjects.length;
	return (
		<>
			<SettingsGroup title="Window">
				{toggle(p, 'menuBar', 'Show menu bar', 'Show File, Edit, View, Project and Help under the title bar. The app menu stays on the wordmark.')}
			</SettingsGroup>
			<SettingsGroup title="Projects">
				{toggle(p, 'reopenLastProject', 'Reopen the last project on start', 'Open the project that was open when the Studio closed, unless one is given on the command line.')}
				{toggle(p, 'confirmDiscard', 'Confirm before discarding unsaved script edits', 'Ask before Reload, Open project or closing the window throws away edits to a script.')}
			</SettingsGroup>
			<SettingsGroup title="Recent projects">
				{numberRow(p, 'recentLimit', 'Projects to remember', 'How many projects File > Open recent lists, from 5 to 20.')}
				<ButtonRow
					label="Recent projects"
					description={count === 0 ? 'The list is empty.' : `${count} ${count === 1 ? 'project' : 'projects'} in File > Open recent.`}
					actionLabel="Clear recent projects"
					disabled={count === 0}
					onAction={() => void p.update({ recentProjects: [] })}
					{...refusal(p.failures, 'recentProjects')}
				/>
			</SettingsGroup>
		</>
	);
}

const THEME_OPTIONS: { value: ThemeId; label: string }[] = [
	{ value: 'afterglow', label: 'Afterglow dark' },
	{ value: 'afterglow-light', label: 'Afterglow light' },
	{ value: 'system', label: 'Follow system' },
	{ value: 'afterglow-dark', label: 'Afterglow dark, neutral' },
];

export function AppearancePage(p: PageProps) {
	return (
		<>
			<SettingsGroup title="Theme">
				<SelectRow
					label="Theme"
					description="Applies to every Studio window at once. Follow system switches between Afterglow dark and light with the desktop."
					value={p.settings.theme}
					options={THEME_OPTIONS}
					onChange={(v) => void p.update({ theme: v })}
					{...refusal(p.failures, 'theme')}
				/>
			</SettingsGroup>
			<SettingsGroup title="Layout">
				<SegmentedRow<Density>
					label="Interface density"
					description="Compact tightens spacing in lists, tables and toolbars."
					value={p.settings.density}
					options={[
						{ value: 'comfortable', label: 'Comfortable' },
						{ value: 'compact', label: 'Compact' },
					]}
					onChange={(v) => void p.update({ density: v })}
					{...refusal(p.failures, 'density')}
				/>
				{numberRow(p, 'textSize', 'Interface text size', 'Scales the text of every window, from 90 to 130 percent.', '%')}
			</SettingsGroup>
			<SettingsGroup title="Motion">
				<SegmentedRow<MotionSetting>
					label="Reduce motion"
					description="Turn off transitions and animation. Follow system uses the desktop's setting."
					value={p.settings.reduceMotion}
					options={[
						{ value: 'system', label: 'Follow system' },
						{ value: 'on', label: 'On' },
						{ value: 'off', label: 'Off' },
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
			<SettingsGroup title="Script editor" description="These apply to the Scripts screen.">
				{numberRow(p, 'editorFontSize', 'Font size', 'The size of the code, from 10 to 24 pixels.', 'px')}
				{numberRow(p, 'tabWidth', 'Tab width', 'How many columns a tab character spans, from 1 to 8.', 'columns')}
				{toggle(p, 'wrapLines', 'Wrap lines', 'Break long lines at the edge of the editor instead of scrolling sideways.')}
				{toggle(p, 'lineNumbers', 'Show line numbers', 'Number the lines beside the code.')}
				{toggle(p, 'formatOnSave', 'Format on save', 'Strip trailing spaces, use one line feed per line and end the file with a single newline when saving.')}
			</SettingsGroup>
		</>
	);
}

export function PreviewPage(p: PageProps) {
	const current = p.settings.previewTarget;
	const known = current === '' || p.targets.includes(current);
	return (
		<SettingsGroup title="Dry run">
			<SelectRow
				label="Default target"
				description="The target the Dry run screen starts on. It is shown only when the project has it."
				value={current}
				options={[
					{ value: '', label: 'First target of the project' },
					...p.targets.map((t) => ({ value: t, label: t })),
					...(known ? [] : [{ value: current, label: `${current} (not in this project)` }]),
				]}
				onChange={(v) => void p.update({ previewTarget: v })}
				{...refusal(p.failures, 'previewTarget')}
			/>
			{toggle(p, 'previewAutoRun', 'Run the preview again when a flow or layout changes', 'Keeps the Dry run screen current as you or an agent edit. Off, a change clears the preview.')}
			{toggle(p, 'showSkippedSteps', 'Show skipped steps', 'List the steps that do not apply to the target, dimmed, in the step list.')}
		</SettingsGroup>
	);
}

export function AgentsPage(p: PageProps) {
	const { host, settings } = p;
	const [agent, setAgent] = useState<AgentInfo | null>(null);
	const [token, setToken] = useState<string | null>(null);
	const [notice, setNotice] = useState<{ key: 'token' | 'copy' | 'regen'; text: string } | null>(null);
	useEffect(() => {
		let live = true;
		setToken(null);
		void host.agent().then((a) => live && setAgent(a), () => live && setAgent(null));
		return () => {
			live = false;
		};
	}, [host, settings.mcpEnabled, settings.mcpPort]);
	const running = agent !== null;
	const fail = (key: 'token' | 'copy' | 'regen') => (e: unknown) => setNotice({ key, text: message(e) });
	const error = (key: 'token' | 'copy' | 'regen') => (notice?.key === key ? { error: notice.text } : {});
	const fetchToken = async () => {
		const t = await host.agentToken();
		if (t === null) throw new Error('The agent server is off.');
		return t;
	};
	return (
		<>
			<SettingsGroup title="Agent server" description="An agent can read the project, edit flows, layouts and scripts, and run checks and dry runs. Every change it makes goes through the same undo history as yours, so Undo reverses it.">
				{toggle(p, 'mcpEnabled', 'Enable the MCP server', 'Serve the Studio to agents on this computer only (127.0.0.1). Off by default in a release build.')}
				{numberRow(p, 'mcpPort', 'Port', '0 picks a free port each time the server starts. A port that is taken is refused.')}
				<SettingsRow label="Status" description={running ? agent.url : settings.mcpEnabled ? 'The server is not running.' : 'The server is off.'} />
			</SettingsGroup>
			<SettingsGroup title="Access token" description="Agents send the token as a bearer token. It is not written to logs or to the settings file, and it changes each time the Studio starts.">
				<SettingsRow label="Token" description={token ?? 'Hidden until you ask to see it.'} {...error('token')}>
					<button
						type="button"
						className="btn"
						disabled={!running}
						onClick={() => {
							if (token !== null) return setToken(null);
							setNotice(null);
							fetchToken().then(setToken, fail('token'));
						}}
					>
						{token !== null ? 'Hide token' : 'Show token'}
					</button>
				</SettingsRow>
				<ButtonRow
					label="Copy token"
					description={notice?.key === 'copy' && notice.text === 'Copied.' ? 'Copied to the clipboard.' : 'Put the token on the clipboard.'}
					actionLabel="Copy"
					disabled={!running}
					onAction={() => {
						setNotice(null);
						fetchToken()
							.then((t) => p.host.copy(t))
							.then(() => setNotice({ key: 'copy', text: 'Copied.' }), fail('copy'));
					}}
					{...(notice?.key === 'copy' && notice.text !== 'Copied.' ? { error: notice.text } : {})}
				/>
				<ButtonRow
					label="Regenerate token"
					description="Agents using the old token are refused from now on."
					actionLabel="Regenerate"
					danger
					disabled={!running}
					onAction={() => {
						setNotice(null);
						void (async () => {
							const answer = await host.confirm({
								kind: 'confirm',
								title: 'Regenerate token',
								message: 'Regenerate the access token?',
								detail: 'Agents using the current token are refused until you give them the new one.',
								buttons: [
									{ id: 'regenerate', label: 'Regenerate', primary: true, danger: true },
									{ id: 'cancel', label: 'Cancel' },
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
function PathRows({ p, name, label, description, kind, title, placeholder }: { p: PageProps; name: 'projectsFolder' | 'runnerPath' | 'dockerPath'; label: string; description: ReactNode; kind: 'folder' | 'file'; title: string; placeholder?: string }) {
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
				label={`Browse for ${label.toLowerCase()}`}
				actionLabel="Browse…"
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
			<SettingsGroup title="Folders">
				<PathRows p={p} name="projectsFolder" label="Projects folder" kind="folder" title="Choose the projects folder" placeholder="Not set" description="Where Open project… starts. Leave empty to start where the system chooses." />
			</SettingsGroup>
			<SettingsGroup title="Tools">
				<PathRows p={p} name="runnerPath" label="Runner" kind="file" title="Choose liminal-engine" placeholder="liminal-engine" description="The `liminal-engine` runner that builds use. It must answer `--version` before the path is saved. Builds from the Studio are not available yet." />
				<PathRows p={p} name="dockerPath" label="Docker (optional)" kind="file" title="Choose docker" placeholder="docker" description="Used for the retro and Linux package checks. It must answer `--version` before the path is saved." />
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
		void host.appInfo().then((i) => live && setInfo(i), () => undefined);
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
			<SettingsGroup title="Files">
				<ButtonRow label="Settings file" description={info?.settingsFile ?? 'Where your preferences are kept.'} actionLabel="Open location" onAction={() => attempt('settings', () => host.reveal('settings'))} {...error('settings')} />
				<ButtonRow label="Logs" description={info?.logDir ?? 'Where the Studio writes logs.'} actionLabel="Show location" onAction={() => attempt('logs', () => host.reveal('logs'))} {...error('logs')} />
			</SettingsGroup>
			<SettingsGroup title="Reset">
				<ButtonRow
					label="Reset all preferences"
					description="Put every setting back to its default, including the recent projects. Your projects are not touched."
					actionLabel="Reset all…"
					danger
					onAction={() =>
						attempt('reset', async () => {
							const answer = await host.confirm({
								kind: 'confirm',
								title: 'Reset preferences',
								message: 'Reset all preferences?',
								detail: 'Every setting goes back to its default. Your projects are not touched.',
								buttons: [
									{ id: 'reset', label: 'Reset all', primary: true, danger: true },
									{ id: 'cancel', label: 'Cancel' },
								],
							});
							if (answer === 'reset') await p.reset();
						})
					}
					{...error('reset')}
				/>
			</SettingsGroup>
			<SettingsGroup title="About">
				<SettingsRow label="Version" description={info ? `Liminal Installer Studio ${info.version}` : 'Liminal Installer Studio'} />
			</SettingsGroup>
		</>
	);
}
