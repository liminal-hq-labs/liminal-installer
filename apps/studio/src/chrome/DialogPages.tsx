// The content of the Studio's larger dialogs: Keyboard shortcuts, Connect an agent and New project.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useMemo, useState } from 'react';
import { t } from '../i18n/messages';
import { errorMessage } from '../lib/api';
import type { Host } from '../lib/host';
import type { Settings } from '../lib/settings';
import { CREATED_PREFIX, type DialogResult } from './dialogWindow';
import { shortcutTable } from './menuModel';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import chrome from './Chrome.module.css';

const message = errorMessage;

/** Every command that has a key, grouped by menu. */
export function ShortcutsBody() {
	const groups = useMemo(() => shortcutTable(), []);
	return (
		<div className={chrome.shortcuts}>
			{groups.map((g) => (
				<section key={g.menu} aria-label={g.menu}>
					<h2>{g.menu}</h2>
					<dl>
						{g.rows.map((r) => (
							<div key={r.id} className={chrome.shortcutRow}>
								<dt>{r.label}</dt>
								<dd>
									{r.keys.map((k) => (
										<kbd key={k}>{k}</kbd>
									))}
								</dd>
							</div>
						))}
					</dl>
				</section>
			))}
			<p className={cx(text.dim, text.small)}>{t('shortcuts.note')}</p>
		</div>
	);
}

/** The text of a client config that points an MCP client at the endpoint. */
export function clientConfig(url: string, token: string): string {
	return JSON.stringify(
		{
			mcpServers: {
				'liminal-installer-studio': {
					type: 'http',
					url,
					headers: { Authorization: `Bearer ${token}` },
				},
			},
		},
		null,
		2,
	);
}

/** Where the agent server listens, with the token on request and a sample client config. */
export function AgentBody({ host }: { host: Host }) {
	const [url, setUrl] = useState<string | null | undefined>(undefined);
	const [why, setWhy] = useState<string | null>(null);
	const [token, setToken] = useState<string | null>(null);
	const [note, setNote] = useState<{ text: string; error: boolean } | null>(null);
	useEffect(() => {
		let live = true;
		void host.agent().then(
			(a) => live && setUrl(a?.url ?? null),
			() => live && setUrl(null),
		);
		void host.agentError().then(
			(e) => live && setWhy(e),
			() => undefined,
		);
		return () => {
			live = false;
		};
	}, [host]);
	const say = (line: string, error = false) => setNote({ text: line, error });
	const fetchToken = async () => {
		const token = await host.agentToken();
		if (token === null) throw new Error(t('agent.serverOff'));
		return token;
	};
	const copy = (what: string, source: () => Promise<string>) => () => {
		setNote(null);
		source()
			.then((value) => host.copy(value))
			.then(
				() => say(t('agent.copied', { what })),
				(e) => say(message(e), true),
			);
	};
	if (url === undefined) return <p className={text.dim}>{t('agent.looking')}</p>;
	if (url === null) {
		return (
			<div className={chrome.agentConnect}>
				<p>{t('agent.notRunning')}</p>
				{why ? (
					<p className={text.err}>{t('agent.didNotStart', { why })}</p>
				) : (
					<p className={text.dim}>{t('agent.turnOn')}</p>
				)}
			</div>
		);
	}
	return (
		<div className={chrome.agentConnect}>
			<p>{t('agent.intro')}</p>
			<label className={ui.field}>
				<span>{t('agent.endpoint')}</span>
				<input type="text" readOnly value={url} onFocus={(e) => e.currentTarget.select()} />
			</label>
			<div className={chrome.row}>
				<button type="button" className={ui.btn} onClick={copy(t('agent.url'), async () => url)}>
					{t('agent.copyUrl')}
				</button>
				<button type="button" className={ui.btn} onClick={copy(t('agent.token'), fetchToken)}>
					{t('agent.copyToken')}
				</button>
				<button
					type="button"
					className={ui.btn}
					onClick={() => {
						if (token !== null) return setToken(null);
						setNote(null);
						fetchToken().then(setToken, (e) => say(message(e), true));
					}}
				>
					{token !== null ? t('agent.hideToken') : t('agent.showToken')}
				</button>
			</div>
			{token !== null ? <code className={cx(chrome.token, ui.token)}>{token}</code> : null}
			<h2>{t('agent.sampleHeading')}</h2>
			<p className={cx(text.dim, text.small)}>{t('agent.sampleNote')}</p>
			<pre className={chrome.snippet}>{clientConfig(url, 'YOUR_TOKEN')}</pre>
			<div className={chrome.row}>
				<button
					type="button"
					className={ui.btn}
					onClick={copy(t('agent.configuration'), async () =>
						clientConfig(url, await fetchToken()),
					)}
				>
					{t('agent.copyConfiguration')}
				</button>
			</div>
			<p role="status" className={note?.error ? text.err : text.dim}>
				{note?.text ?? ''}
			</p>
		</div>
	);
}

const slug = (name: string) =>
	name
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '');

const join = (folder: string, leaf: string) =>
	`${folder.replace(/[\\/]+$/, '')}${folder.includes('\\') && !folder.includes('/') ? '\\' : '/'}${leaf}`;

/** A form that scaffolds a project the way `liminst init` does, then answers with the new folder. */
export function NewProjectBody({
	host,
	settings,
	onAnswer,
}: {
	host: Host;
	settings: Settings;
	onAnswer(result: DialogResult): void;
}) {
	const [name, setName] = useState('');
	const [id, setId] = useState('');
	const [idTouched, setIdTouched] = useState(false);
	const [publisher, setPublisher] = useState('');
	const [parent, setParent] = useState(settings.projectsFolder);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const leaf = slug(name);
	const shownId = idTouched ? id : leaf ? `example.${leaf.replace(/-/g, '')}` : '';
	const ready = name.trim() !== '' && shownId !== '' && parent.trim() !== '' && leaf !== '';
	const create = async () => {
		setBusy(true);
		setError(null);
		try {
			const folder = await host.newProject(
				join(parent.trim(), leaf),
				name.trim(),
				shownId.trim(),
				publisher.trim() || t('newProject.unknownPublisher'),
			);
			onAnswer(`${CREATED_PREFIX}${folder}`);
		} catch (e) {
			setError(message(e));
			setBusy(false);
		}
	};
	return (
		<form
			className={chrome.newProject}
			onSubmit={(e) => {
				e.preventDefault();
				if (ready && !busy) void create();
			}}
		>
			<label className={ui.field}>
				<span>{t('newProject.name')}</span>
				<input
					type="text"
					value={name}
					autoFocus
					onChange={(e) => setName(e.target.value)}
					placeholder={t('newProject.namePlaceholder')}
				/>
			</label>
			<label className={ui.field}>
				<span>{t('newProject.id')}</span>
				<input
					type="text"
					value={shownId}
					onChange={(e) => (setId(e.target.value), setIdTouched(true))}
					placeholder={t('newProject.idPlaceholder')}
				/>
			</label>
			<label className={ui.field}>
				<span>{t('newProject.publisher')}</span>
				<input
					type="text"
					value={publisher}
					onChange={(e) => setPublisher(e.target.value)}
					placeholder={t('newProject.publisherPlaceholder')}
				/>
			</label>
			<label className={ui.field}>
				<span>{t('newProject.createIn')}</span>
				<span className={chrome.row}>
					<input
						type="text"
						value={parent}
						onChange={(e) => setParent(e.target.value)}
						placeholder={t('newProject.folderPlaceholder')}
					/>
					<button
						type="button"
						className={ui.btn}
						onClick={() =>
							void host.browse('folder', t('newProject.chooseFolder'), parent || undefined).then(
								(p) => p && setParent(p),
								(e) => setError(message(e)),
							)
						}
					>
						{t('newProject.browse')}
					</button>
				</span>
			</label>
			<p className={cx(text.dim, text.small)}>
				{leaf && parent
					? t('newProject.filesGoIn', { folder: `${join(parent, leaf)}/installer` })
					: t('newProject.sameFiles')}
			</p>
			{error ? (
				<p className={text.err} role="alert">
					{error}
				</p>
			) : null}
			<div className={chrome.dlgActions}>
				<button type="button" className={ui.btn} onClick={() => onAnswer(null)}>
					{t('common.cancel')}
				</button>
				<button type="submit" className={cx(ui.btn, ui.primary)} disabled={!ready || busy}>
					{busy ? t('newProject.creating') : t('newProject.create')}
				</button>
			</div>
		</form>
	);
}

/** The body for a kind of dialog that has more than a message. */
export function PageDialog({
	kind,
	host,
	settings,
	onAnswer,
}: {
	kind: 'shortcuts' | 'agent' | 'new-project';
	host: Host;
	settings: Settings;
	onAnswer(result: DialogResult): void;
}) {
	if (kind === 'shortcuts') return <ShortcutsBody />;
	if (kind === 'agent') return <AgentBody host={host} />;
	return <NewProjectBody host={host} settings={settings} onAnswer={onAnswer} />;
}
