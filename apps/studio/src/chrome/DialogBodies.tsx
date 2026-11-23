// The content of the Studio's dialogs, drawn in a child window or, in the browser demo, in the page.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import { cx } from '../ui/cx';
import text from '../ui/text.module.css';
import ui from '../ui/ui.module.css';
import chrome from './Chrome.module.css';
import type { DialogResult, DialogSpec } from './dialogWindow';

/** The product name, for the wordmark and the title bar. */
export const appName = (): string => t('app.name');

/** The mark beside the wordmark: a small rounded square with the brand gradient. */
export function Mark() {
	return (
		<svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
			<defs>
				<linearGradient id="lis-mark" x1="0" y1="0" x2="1" y2="1">
					<stop offset="0" stopColor="#22d3ee" />
					<stop offset="0.5" stopColor="#60a5fa" />
					<stop offset="1" stopColor="#a78bfa" />
				</linearGradient>
			</defs>
			<rect
				x="0.5"
				y="0.5"
				width="15"
				height="15"
				rx="4"
				fill="#16161c"
				stroke="rgba(255,255,255,0.14)"
			/>
			<path
				d="M4.5 4.5v7h7"
				stroke="url(#lis-mark)"
				strokeWidth="1.8"
				strokeLinecap="round"
				strokeLinejoin="round"
			/>
		</svg>
	);
}

/** The buttons of a spec: its own, or one Close for a dialog that asks nothing. */
export function buttonsOf(spec: DialogSpec) {
	return spec.buttons && spec.buttons.length > 0
		? spec.buttons
		: [{ id: 'close', label: t('common.close'), primary: true }];
}

export function DialogButtons({
	spec,
	onAnswer,
}: {
	spec: DialogSpec;
	onAnswer(result: DialogResult): void;
}) {
	return (
		<div className={chrome.dlgActions}>
			{buttonsOf(spec).map((b) => (
				<button
					key={b.id}
					type="button"
					className={cx(ui.btn, b.primary && ui.primary, b.danger && ui.danger)}
					onClick={() => onAnswer(b.id)}
					autoFocus={b.primary}
				>
					{b.label}
				</button>
			))}
		</div>
	);
}

/** What an About or confirmation dialog says. */
export function DialogMessage({ spec }: { spec: DialogSpec }) {
	if (spec.kind === 'about') {
		return (
			<>
				<div className={chrome.dlgBrand}>
					<Mark />
					<span className={chrome.dlgWordmark}>{appName()}</span>
				</div>
				<p>{t('about.body')}</p>
				<p className={text.dim}>{t('about.labs')}</p>
			</>
		);
	}
	return (
		<>
			{spec.message ? <p className={chrome.dlgMessage}>{spec.message}</p> : null}
			{spec.detail ? <p className={text.dim}>{spec.detail}</p> : null}
		</>
	);
}
