// The Label and README tab: the volume label with its live count, the serial, a disc's derived names and the README.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tn } from '../../i18n/messages';
import {
	checkLabel,
	checkSerial,
	formatSerial,
	isoVolumeId,
	jolietLabel,
	labelMax,
} from '../../lib/media';
import { README_WIDTH, checkReadme, type Change } from '../../lib/mediaEdit';
import type { MediaInfo, MediaPlan } from '../../lib/types';
import { cx } from '../../ui/cx';
import text from '../../ui/text.module.css';
import ui from '../../ui/ui.module.css';
import styles from '../MediaScreen.module.css';
import { useDraft } from './useDraft';

function LabelField({ info, onChange }: { info: MediaInfo; onChange(change: Change): void }) {
	const max = labelMax(info.kind);
	const check = (label: string): string | null => {
		const problem = checkLabel(label, info.kind);
		if (!problem) return null;
		return problem.kind === 'tooLong'
			? t('media.label.tooLong', { length: problem.length, max: problem.max })
			: t('media.label.badChar', { char: problem.char });
	};
	const draft = useDraft(
		info.label,
		check,
		(label) =>
			onChange({
				edit: { label },
				label: t('history.mediaLabel', { id: info.id }),
			}),
		(typed) => typed.toUpperCase(),
	);
	const count = [...draft.draft].length;
	return (
		<div className={styles.fieldBlock}>
			<label htmlFor="media-label">{t('media.label.field')}</label>
			<div className={styles.fieldRow}>
				<input
					id="media-label"
					type="text"
					className={cx(styles.input, text.mono)}
					spellCheck={false}
					autoComplete="off"
					aria-describedby="media-label-rule media-label-count"
					{...draft.props}
				/>
				<span
					id="media-label-count"
					className={cx(text.mono, count > max ? text.err : text.muted)}
					aria-live="polite"
				>
					{t('media.label.count', { count, max })}
				</span>
			</div>
			<p id="media-label-rule" className={cx(text.small, draft.error ? text.err : text.dim)}>
				{draft.error ?? t('media.label.rule', { max })}
			</p>
		</div>
	);
}

function SerialField({
	info,
	plan,
	onChange,
}: {
	info: MediaInfo;
	plan: MediaPlan;
	onChange(change: Change): void;
}) {
	const auto = info.serial.toLowerCase() === 'auto';
	const send = (serial: string) =>
		onChange({ edit: { serial }, label: t('history.mediaSerial', { id: info.id }) });
	const fixed = useDraft(
		auto ? plan.serial : info.serial,
		(v) => (checkSerial(v) === null || checkSerial(v) === 'auto' ? t('media.serial.bad') : null),
		(v) => {
			const n = checkSerial(v);
			if (typeof n === 'number') send(formatSerial(n));
		},
		(v) => v.toUpperCase(),
	);
	return (
		<fieldset className={styles.group}>
			<legend>{t('media.serial.title')}</legend>
			<label className={styles.inlineCheck}>
				<input type="radio" name="media-serial" checked={auto} onChange={() => send('auto')} />
				{t('media.serial.auto')}
			</label>
			{auto ? (
				<p className={cx(text.small, text.muted)}>
					{t('media.serial.autoValue', { serial: plan.serial })}
				</p>
			) : null}
			<label className={styles.inlineCheck}>
				<input
					type="radio"
					name="media-serial"
					checked={!auto}
					onChange={() => send(plan.serial)}
				/>
				{t('media.serial.fixed')}
			</label>
			{!auto ? (
				<div className={styles.fieldBlock}>
					<input
						type="text"
						className={cx(styles.input, text.mono)}
						aria-label={t('media.serial.field')}
						aria-describedby="media-serial-rule"
						spellCheck={false}
						autoComplete="off"
						{...fixed.props}
					/>
					<p id="media-serial-rule" className={cx(text.small, fixed.error ? text.err : text.dim)}>
						{fixed.error ?? t('media.serial.rule')}
					</p>
				</div>
			) : null}
		</fieldset>
	);
}

/** The README as a box of 76 columns in VT323, with whether DOS would be happy with it. */
export function ReadmePreview({
	info,
	plan,
	onPick,
	onGenerate,
}: {
	info: MediaInfo;
	plan: MediaPlan;
	onPick(): void;
	onGenerate(): void;
}) {
	const check = checkReadme(plan.readme.text);
	return (
		<section className={ui.card} aria-labelledby="readme-h">
			<h3 id="readme-h">{t('media.readme.title')}</h3>
			<p className={cx(text.small, text.muted)}>
				{plan.readme.generated
					? t('media.readme.generated')
					: t('media.readme.file', { file: info.readme ?? '' })}
			</p>
			<pre
				className={styles.readme}
				style={{ inlineSize: `${README_WIDTH + 2}ch` }}
				tabIndex={0}
				aria-label={t('media.readme.preview')}
			>
				{plan.readme.text.replace(/\r/g, '')}
			</pre>
			<p className={cx(text.small, check.tooWide > 0 ? text.err : text.ok)}>
				{check.tooWide > 0
					? tn('media.readme.tooWide', check.tooWide, { width: README_WIDTH })
					: t('media.readme.fits', { width: README_WIDTH })}
			</p>
			<p className={cx(text.small, check.lfOnly ? text.err : text.ok)}>
				{t(check.lfOnly ? 'media.readme.lf' : 'media.readme.crlf')}
			</p>
			<div className={ui.toolbar}>
				<button type="button" className={ui.btn} onClick={onPick}>
					{t('media.readme.pick')}
				</button>
				<button
					type="button"
					className={ui.btn}
					disabled={plan.readme.generated}
					onClick={onGenerate}
				>
					{t('media.readme.generate')}
				</button>
			</div>
		</section>
	);
}

export interface LabelCardProps {
	info: MediaInfo;
	plan: MediaPlan;
	onChange(change: Change): void;
	onPickReadme(): void;
}

export function LabelCard({ info, plan, onChange, onPickReadme }: LabelCardProps) {
	return (
		<div className={styles.sizes}>
			<section className={ui.card} aria-labelledby="label-h">
				<h3 id="label-h">{t('media.label.title')}</h3>
				<LabelField info={info} onChange={onChange} />
				<SerialField info={info} plan={plan} onChange={onChange} />
				{info.kind === 'cdrom' ? (
					<dl className={styles.facts}>
						<dt>{t('media.cd.iso')}</dt>
						<dd className={text.mono}>{isoVolumeId(info.label)}</dd>
						<dt>{t('media.cd.joliet')}</dt>
						<dd className={text.mono}>{jolietLabel(info.label)}</dd>
					</dl>
				) : null}
			</section>
			<ReadmePreview
				info={info}
				plan={plan}
				onPick={onPickReadme}
				onGenerate={() =>
					onChange({
						edit: { readme: '' },
						label: t('history.mediaReadme', { id: info.id }),
					})
				}
			/>
		</div>
	);
}
