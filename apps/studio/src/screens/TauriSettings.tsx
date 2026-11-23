// The Tauri screen's settings cards: product fields, the Windows install, WebView2 and updates.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useId, useState } from 'react';
import { t, type MessageId } from '../i18n/messages';
import {
	fieldBadges,
	formatSizeCost,
	hasTwoUpdaters,
	ifMissingText,
	ifPresentText,
	isIntervalText,
	MODE_LABEL,
	nextOverrides,
	UPDATE_MODES,
	updateNote,
	updatesPatch,
	webview2Overridden,
	webview2Patch,
	WEBVIEW2_MODES,
	type FieldBadge,
} from '../lib/tauri';
import type { InstallMode, TauriSettingsChange, TauriView, UpdateMode } from '../lib/types';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import styles from './TauriScreen.module.css';

/** Applies a settings change. A failure shows on the screen. */
export type Apply = (change: TauriSettingsChange, label?: string) => void;

interface CardProps {
	view: TauriView;
	busy: boolean;
	apply: Apply;
}

const BADGE: Record<FieldBadge, { label: MessageId; pill: string | undefined }> = {
	imported: { label: 'tauri.badge.imported', pill: ui.pillOk },
	overridden: { label: 'tauri.badge.overridden', pill: styles.pillNeutral },
	differs: { label: 'tauri.badge.differs', pill: ui.pillWarning },
};

const FIELD_LABEL: Record<string, MessageId> = {
	id: 'tauri.field.id',
	name: 'tauri.field.name',
	publisher: 'tauri.field.publisher',
	version: 'tauri.field.version',
};

/** A text input that keeps what is typed until it loses focus or Enter is pressed, so one edit is one history entry. */
function CommitField(props: {
	label: string;
	value: string;
	disabled?: boolean;
	placeholder?: string;
	validate?: (draft: string) => boolean;
	onCommit: (value: string) => void;
}) {
	const { value, validate, onCommit } = props;
	const [draft, setDraft] = useState(value);
	useEffect(() => setDraft(value), [value]);
	const commit = () => {
		if (draft === value) return;
		if (validate && !validate(draft)) setDraft(value);
		else onCommit(draft);
	};
	return (
		<label className={ui.field}>
			<span>{props.label}</span>
			<input
				type="text"
				value={draft}
				disabled={props.disabled}
				placeholder={props.placeholder}
				onChange={(e) => setDraft(e.target.value)}
				onBlur={commit}
				onKeyDown={(e) => {
					if (e.key === 'Enter') commit();
				}}
			/>
		</label>
	);
}

export function IdentityCard({ view, busy, apply }: CardProps) {
	return (
		<section className={ui.card} aria-labelledby="tauri-identity-h">
			<h2 id="tauri-identity-h">{t('tauri.identity.title')}</h2>
			<p className={cx(text.muted, styles.note)}>{t('tauri.identity.help')}</p>
			<table>
				<thead>
					<tr>
						<th scope="col">{t('tauri.identity.col.field')}</th>
						<th scope="col">{t('tauri.identity.col.imported')}</th>
						<th scope="col">{t('tauri.identity.col.project')}</th>
						<th scope="col">{t('tauri.identity.col.status')}</th>
						<th scope="col">{t('tauri.identity.col.keep')}</th>
					</tr>
				</thead>
				<tbody>
					{view.identity.map((row) => {
						const field = t(FIELD_LABEL[row.field] ?? 'tauri.field.id');
						return (
							<tr key={row.field}>
								<th scope="row">{FIELD_LABEL[row.field] ? field : row.field}</th>
								<td className={text.mono}>{row.imported ?? '—'}</td>
								<td className={text.mono}>{row.project ?? '—'}</td>
								<td>
									<span className={styles.badges}>
										{fieldBadges(row).map((b) => (
											<span key={b} className={cx(ui.pill, BADGE[b].pill)}>
												{t(BADGE[b].label)}
											</span>
										))}
									</span>
								</td>
								<td>
									<input
										type="checkbox"
										checked={row.overridden}
										disabled={busy || row.imported === null}
										aria-label={t('tauri.identity.keepAria', {
											field: FIELD_LABEL[row.field] ? field : row.field,
										})}
										onChange={(e) =>
											apply(
												{ overrides: nextOverrides(view.identity, row.field, e.target.checked) },
												t('history.tauriKeep', { field: row.field }),
											)
										}
									/>
								</td>
							</tr>
						);
					})}
				</tbody>
			</table>
		</section>
	);
}

export function WindowsCard({ view, busy, apply }: CardProps) {
	const name = useId();
	const options: { mode: InstallMode; label: MessageId; help: MessageId }[] = [
		{ mode: 'native', label: 'tauri.windows.native', help: 'tauri.windows.nativeHelp' },
		{ mode: 'wrap', label: 'tauri.windows.wrap', help: 'tauri.windows.wrapHelp' },
	];
	return (
		<section className={ui.card} aria-labelledby="tauri-windows-h">
			<h2 id="tauri-windows-h">{t('tauri.windows.title')}</h2>
			<fieldset className={styles.group}>
				<legend>{t('tauri.windows.legend')}</legend>
				{options.map((o) => (
					<label key={o.mode} className={styles.option}>
						<input
							type="radio"
							name={name}
							value={o.mode}
							checked={view.windows === o.mode}
							disabled={busy}
							onChange={() => apply({ windows: o.mode })}
						/>
						<span className={styles.optionBody}>
							<span className={styles.optionName}>{t(o.label)}</span>
							<span className={cx(text.dim, text.small)}>{t(o.help)}</span>
						</span>
					</label>
				))}
			</fieldset>
			{view.needs_wrap ? (
				<p className={cx(styles.note, view.windows === 'native' && styles.warn)} role="note">
					{t('tauri.windows.needsWrap')}
				</p>
			) : null}
			<p className={cx(text.dim, text.small, styles.note)}>{t('tauri.windows.otherPlatforms')}</p>
		</section>
	);
}

export function Webview2Card({ view, busy, apply }: CardProps) {
	const name = useId();
	const w = view.webview2;
	if (!w) {
		return (
			<section className={ui.card} aria-labelledby="tauri-webview2-h">
				<h2 id="tauri-webview2-h">{t('tauri.webview2.title')}</h2>
				<p className={text.muted}>{t('tauri.webview2.none')}</p>
			</section>
		);
	}
	const overridden = webview2Overridden(w);
	const mode = w.effective.mode;
	return (
		<section className={ui.card} aria-labelledby="tauri-webview2-h">
			<div className={styles.cardHead}>
				<h2 id="tauri-webview2-h">{t('tauri.webview2.title')}</h2>
				<span className={styles.badges}>
					<span className={cx(ui.pill, overridden ? styles.pillNeutral : ui.pillOk)}>
						{t(overridden ? 'tauri.badge.overridden' : 'tauri.badge.imported')}
					</span>
				</span>
			</div>
			<p className={cx(text.muted, styles.note)}>
				{t('tauri.webview2.importedValue', { mode: t(MODE_LABEL[w.imported.mode]) })}
			</p>
			<fieldset className={styles.group}>
				<legend>{t('tauri.webview2.legend')}</legend>
				{WEBVIEW2_MODES.map((m) => {
					const info = w.modes.find((x) => x.mode === m);
					return (
						<label key={m} className={styles.option}>
							<input
								type="radio"
								name={name}
								value={m}
								checked={mode === m}
								disabled={busy}
								onChange={() => apply({ webview2: webview2Patch(w, { mode: m }) })}
							/>
							<span className={styles.optionBody}>
								<span className={styles.optionName}>{t(MODE_LABEL[m])}</span>
								{info ? (
									<span className={cx(text.dim, text.small)}>
										{[
											formatSizeCost(info.size_cost_mb),
											t(
												info.needs_network
													? 'tauri.webview2.needsNetwork'
													: 'tauri.webview2.offline',
											),
											info.carries ? t('tauri.webview2.carries', { file: info.carries }) : null,
											info.source ? t('tauri.webview2.source', { source: info.source }) : null,
										]
											.filter(Boolean)
											.join(' · ')}
									</span>
								) : null}
							</span>
						</label>
					);
				})}
			</fieldset>
			<div className={styles.fields}>
				<CommitField
					label={t('tauri.webview2.minVersion')}
					value={w.effective.min_version ?? ''}
					placeholder={t('tauri.webview2.minVersionHint')}
					disabled={busy}
					onCommit={(v) => apply({ webview2: webview2Patch(w, { min_version: v }) })}
				/>
				{mode === 'fixedRuntime' ? (
					<CommitField
						label={t('tauri.webview2.path')}
						value={w.effective.path ?? ''}
						placeholder={t('tauri.webview2.pathHint')}
						disabled={busy}
						onCommit={(v) => apply({ webview2: webview2Patch(w, { path: v }) })}
					/>
				) : null}
				<label className={ui.check}>
					<input
						type="checkbox"
						checked={w.effective.silent}
						disabled={busy}
						onChange={(e) => apply({ webview2: webview2Patch(w, { silent: e.target.checked }) })}
					/>
					<span>{t('tauri.webview2.silent')}</span>
				</label>
			</div>
			<div className={ui.toolbar}>
				<button
					type="button"
					className={ui.btn}
					disabled={busy || !overridden}
					onClick={() => apply({ webview2: {} }, t('history.tauriWebview2Imported'))}
				>
					{t('tauri.webview2.useImported')}
				</button>
			</div>
			<div className={styles.sentences}>
				<p>{t('tauri.webview2.onMissing', { what: ifMissingText(w.if_missing) })}</p>
				<p>
					{t('tauri.webview2.onPresent', {
						what: ifPresentText(w.if_present, w.effective.min_version),
					})}
				</p>
			</div>
		</section>
	);
}

export function UpdatesCard({ view, busy, apply }: CardProps) {
	const u = view.updates;
	const modeLabel: Record<UpdateMode, MessageId> = {
		none: 'tauri.updates.mode.none',
		liminal: 'tauri.updates.mode.liminal',
		tauri: 'tauri.updates.mode.tauri',
		both: 'tauri.updates.mode.both',
	};
	const conflict = hasTwoUpdaters(u);
	return (
		<section className={ui.card} aria-labelledby="tauri-updates-h">
			<h2 id="tauri-updates-h">{t('tauri.updates.title')}</h2>
			<label className={ui.field}>
				<span>{t('tauri.updates.mode')}</span>
				<select
					value={u.mode}
					disabled={busy}
					onChange={(e) => apply(updatesPatch(u, { mode: e.target.value as UpdateMode }))}
				>
					{UPDATE_MODES.map((m) => (
						<option key={m} value={m}>
							{t(modeLabel[m])}
						</option>
					))}
				</select>
			</label>
			<p
				className={cx(styles.note, conflict && styles.warn)}
				role={conflict ? 'alert' : 'note'}
				data-testid="update-note"
			>
				{t(updateNote(u.mode, u.app_has_updater))}
			</p>
			<div className={styles.fields}>
				<CommitField
					label={t('tauri.updates.feed')}
					value={u.feed ?? ''}
					placeholder={t('tauri.updates.feedHint')}
					disabled={busy}
					onCommit={(v) => apply(updatesPatch(u, { feed: v }))}
				/>
				<CommitField
					label={t('tauri.updates.channel')}
					value={u.channel ?? ''}
					placeholder={t('tauri.updates.channelHint')}
					disabled={busy}
					onCommit={(v) => apply(updatesPatch(u, { channel: v }))}
				/>
				<CommitField
					label={t('tauri.updates.interval')}
					value={u.check_interval_hours === null ? '' : String(u.check_interval_hours)}
					placeholder={t('tauri.updates.intervalHint')}
					disabled={busy}
					validate={isIntervalText}
					onCommit={(v) => apply(updatesPatch(u, { interval: v }))}
				/>
			</div>
			<h3>{t('tauri.updates.endpoints')}</h3>
			{u.endpoints.length === 0 ? (
				<p className={text.dim}>{t('tauri.updates.noEndpoints')}</p>
			) : (
				<ul className={ui.plain} aria-label={t('tauri.updates.endpoints')}>
					{u.endpoints.map((e) => (
						<li key={e}>
							<code>{e}</code>
						</li>
					))}
				</ul>
			)}
			<p className={cx(text.dim, text.small, styles.note)}>{t('tauri.updates.endpointsNote')}</p>
		</section>
	);
}
