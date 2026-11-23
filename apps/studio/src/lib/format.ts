// Small formatting helpers shared by the views.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tn } from '../i18n/messages';
import type { HistoryEntry, Operation, Problem, Screenshot } from './types';

/** An `<img src>` value for a preview screenshot. */
export function screenshotSrc(shot: Screenshot): string {
	return shot.encoding === 'base64'
		? `data:${shot.mime};base64,${shot.data}`
		: `data:${shot.mime};charset=utf-8,${encodeURIComponent(shot.data)}`;
}

/** One line describing a plan operation. */
export function formatOp(op: Operation): { name: string; detail: string } {
	const fields = op as Record<string, unknown>;
	const s = (k: string) => (typeof fields[k] === 'string' ? (fields[k] as string) : '');
	switch (op.op) {
		case 'copy':
			return { name: 'copy', detail: `${s('from')} → ${s('to')}` };
		case 'mkdir':
		case 'receipt.write':
			return { name: op.op, detail: s('path') };
		case 'write_file':
			return { name: op.op, detail: s('path') };
		case 'shortcut':
			return { name: 'shortcut', detail: `${s('name')} → ${s('target')} (${s('place')})` };
		case 'file_type':
			return { name: 'file_type', detail: `${s('extension')} · ${s('name')}` };
		case 'url_scheme':
			return { name: 'url_scheme', detail: `${s('scheme')}://` };
		case 'path.add':
			return { name: 'path.add', detail: s('entry') };
		case 'prereq.webview2':
			return {
				name: op.op,
				detail: `${t('op.webview2', { mode: op.mode })}${
					op.min_version ? ` · ${t('op.webview2Min', { version: op.min_version })}` : ''
				}`,
			};
		case 'tauri.wrap':
			return {
				name: op.op,
				detail: `${t('op.tauriWrap', { product: op.product_name })}${
					op.args.length ? ` ${op.args.join(' ')}` : ''
				}`,
			};
		case 'os.register':
			return { name: 'os.register', detail: `${s('product')} ${s('version')}` };
		default:
			return {
				name: op.op,
				detail: Object.entries(op)
					.filter(([k]) => k !== 'op')
					.map(([k, v]) => `${k}=${String(v)}`)
					.join(' '),
			};
	}
}

/** Problems whose scope is `scope` or starts with it. */
export function problemsFor(problems: Problem[], scope: string): Problem[] {
	return problems.filter((p) => p.scope === scope || p.scope.startsWith(`${scope}:`));
}

export const errorCount = (problems: Problem[]): number =>
	problems.filter((p) => p.severity === 'error').length;

/** The history in newest-first order with a marker for entries that can be redone. */
export function historyRows(
	entries: HistoryEntry[],
): { label: string; source: string; doc: string; undone: boolean }[] {
	return [...entries].reverse().map((e) => ({
		label: e.label,
		source: e.source === 'mcp' ? t('history.sourceAgent') : t('history.sourceYou'),
		doc: e.doc.type === 'tauri' ? e.doc.type : `${e.doc.type} ${e.doc.id}`,
		undone: !e.applied,
	}));
}

/** The status shown for a target in the project table, from the problems that name it. */
export function targetStatus(problems: Problem[], fullId: string): 'ok' | 'warning' | 'error' {
	const mine = problemsFor(problems, `target:${fullId}`);
	if (mine.some((p) => p.severity === 'error')) return 'error';
	return mine.length > 0 ? 'warning' : 'ok';
}

/** "2 errors, 1 warning": the counts the status bar shows, leaving out a zero. */
export function problemSummary(errors: number, warnings: number): string {
	return [
		...(errors === 0 ? [] : [tn('problems.errors', errors)]),
		...(warnings === 0 ? [] : [tn('problems.warnings', warnings)]),
	].join(', ');
}
