// The content of the Studio's larger dialogs: Keyboard shortcuts, Connect an agent and New project.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useMemo, useState } from 'react';
import type { Host } from '../lib/host';
import type { Settings } from '../lib/settings';
import { CREATED_PREFIX, type DialogResult } from './dialogWindow';
import { shortcutTable } from './menuModel';

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Every command that has a key, grouped by menu. */
export function ShortcutsBody() {
	const groups = useMemo(() => shortcutTable(), []);
	return (
		<div className="shortcuts">
			{groups.map((g) => (
				<section key={g.menu} aria-label={g.menu}>
					<h2>{g.menu}</h2>
					<dl>
						{g.rows.map((r) => (
							<div key={r.id} className="shortcut-row">
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
			<p className="dim small">F10 or a lone Alt opens the menu. Alt and a letter open a menu in the bar.</p>
		</div>
	);
}

/** The text of a client config that points an MCP client at the endpoint. */
export function clientConfig(url: string, token: string): string {
	return JSON.stringify({ mcpServers: { 'liminal-installer-studio': { type: 'http', url, headers: { Authorization: `Bearer ${token}` } } } }, null, 2);
}

/** Where the agent server listens, with the token on request and a sample client config. */
export function AgentBody({ host }: { host: Host }) {
	const [url, setUrl] = useState<string | null | undefined>(undefined);
	const [why, setWhy] = useState<string | null>(null);
	const [token, setToken] = useState<string | null>(null);
	const [note, setNote] = useState<{ text: string; error: boolean } | null>(null);
	useEffect(() => {
		let live = true;
		void host.agent().then((a) => live && setUrl(a?.url ?? null), () => live && setUrl(null));
		void host.agentError().then((e) => live && setWhy(e), () => undefined);
		return () => {
			live = false;
		};
	}, [host]);
	const say = (text: string, error = false) => setNote({ text, error });
	const fetchToken = async () => {
		const t = await host.agentToken();
		if (t === null) throw new Error('The agent server is off.');
		return t;
	};
	const copy = (what: string, text: () => Promise<string>) => () => {
		setNote(null);
		text()
			.then((t) => host.copy(t))
			.then(() => say(`${what} copied.`), (e) => say(message(e), true));
	};
	if (url === undefined) return <p className="dim">Looking for the agent server…</p>;
	if (url === null) {
		return (
			<div className="agent-connect">
				<p>The agent server is not running.</p>
				{why ? <p className="err">It did not start: {why}</p> : <p className="dim">Turn on the MCP server in Preferences, under Agents.</p>}
			</div>
		);
	}
	return (
		<div className="agent-connect">
			<p>Agents edit this project over MCP, through the same undo history as you. The server listens on this computer only.</p>
			<label className="field">
				<span>Endpoint</span>
				<input type="text" readOnly value={url} onFocus={(e) => e.currentTarget.select()} />
			</label>
			<div className="row">
				<button type="button" className="btn" onClick={copy('URL', async () => url)}>
					Copy URL
				</button>
				<button type="button" className="btn" onClick={copy('Token', fetchToken)}>
					Copy token
				</button>
				<button
					type="button"
					className="btn"
					onClick={() => {
						if (token !== null) return setToken(null);
						setNote(null);
						fetchToken().then(setToken, (e) => say(message(e), true));
					}}
				>
					{token !== null ? 'Hide token' : 'Show token'}
				</button>
			</div>
			{token !== null ? <code className="token">{token}</code> : null}
			<h2>Sample client configuration</h2>
			<p className="dim small">For a client that reads an `mcpServers` block, such as Claude Code or Claude Desktop. Copying it puts the real token in.</p>
			<pre className="snippet">{clientConfig(url, 'YOUR_TOKEN')}</pre>
			<div className="row">
				<button type="button" className="btn" onClick={copy('Configuration', async () => clientConfig(url, await fetchToken()))}>
					Copy configuration
				</button>
			</div>
			<p role="status" className={note?.error ? 'err' : 'dim'}>
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

const join = (folder: string, leaf: string) => `${folder.replace(/[\\/]+$/, '')}${folder.includes('\\') && !folder.includes('/') ? '\\' : '/'}${leaf}`;

/** A form that scaffolds a project the way `liminst init` does, then answers with the new folder. */
export function NewProjectBody({ host, settings, onAnswer }: { host: Host; settings: Settings; onAnswer(result: DialogResult): void }) {
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
			const folder = await host.newProject(join(parent.trim(), leaf), name.trim(), shownId.trim(), publisher.trim() || 'Unknown publisher');
			onAnswer(`${CREATED_PREFIX}${folder}`);
		} catch (e) {
			setError(message(e));
			setBusy(false);
		}
	};
	return (
		<form
			className="new-project"
			onSubmit={(e) => {
				e.preventDefault();
				if (ready && !busy) void create();
			}}
		>
			<label className="field">
				<span>Name</span>
				<input type="text" value={name} autoFocus onChange={(e) => setName(e.target.value)} placeholder="Fizz Level" />
			</label>
			<label className="field">
				<span>Id</span>
				<input type="text" value={shownId} onChange={(e) => (setId(e.target.value), setIdTouched(true))} placeholder="hq.liminal.fizz" />
			</label>
			<label className="field">
				<span>Publisher</span>
				<input type="text" value={publisher} onChange={(e) => setPublisher(e.target.value)} placeholder="Liminal HQ" />
			</label>
			<label className="field">
				<span>Create in</span>
				<span className="row">
					<input type="text" value={parent} onChange={(e) => setParent(e.target.value)} placeholder="A folder for the project" />
					<button
						type="button"
						className="btn"
						onClick={() =>
							void host.browse('folder', 'Choose where to create the project', parent || undefined).then(
								(p) => p && setParent(p),
								(e) => setError(message(e)),
							)
						}
					>
						Browse…
					</button>
				</span>
			</label>
			<p className="dim small">{leaf && parent ? `The files go in ${join(parent, leaf)}/installer.` : 'Writes the same starting files as `liminst init`.'}</p>
			{error ? (
				<p className="err" role="alert">
					{error}
				</p>
			) : null}
			<div className="dlg-actions">
				<button type="button" className="btn" onClick={() => onAnswer(null)}>
					Cancel
				</button>
				<button type="submit" className="btn primary" disabled={!ready || busy}>
					{busy ? 'Creating…' : 'Create project'}
				</button>
			</div>
		</form>
	);
}

/** The body for a kind of dialog that has more than a message. */
export function PageDialog({ kind, host, settings, onAnswer }: { kind: 'shortcuts' | 'agent' | 'new-project'; host: Host; settings: Settings; onAnswer(result: DialogResult): void }) {
	if (kind === 'shortcuts') return <ShortcutsBody />;
	if (kind === 'agent') return <AgentBody host={host} />;
	return <NewProjectBody host={host} settings={settings} onAnswer={onAnswer} />;
}
