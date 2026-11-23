// Puts the backend's findings in the editor: a squiggle on the span and a marker in the gutter.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { lintGutter, linter, type Diagnostic } from '@codemirror/lint';
import type { Extension, Text } from '@codemirror/state';
import type { ScriptDiagnostic } from '../lib/types';

/** One mark per finding. Lines and columns start at 1; a position past the end of the text lands on its end, so nothing is lost. */
export function toLintDiagnostics(doc: Text, found: readonly ScriptDiagnostic[]): Diagnostic[] {
	return found.map((d) => {
		const start = doc.line(Math.min(Math.max(Math.trunc(d.line), 1), doc.lines));
		const end = doc.line(Math.min(Math.max(Math.trunc(d.end_line), Math.trunc(d.line), 1), doc.lines));
		const from = Math.min(start.from + Math.max(d.column - 1, 0), start.to);
		let to = Math.min(end.from + Math.max(d.end_column - 1, 0), end.to);
		if (to <= from) to = Math.min(from + 1, doc.length);
		return { from, to: Math.max(to, from), severity: d.severity, source: d.code, message: d.message };
	});
}

/** How many of each kind, for the footer. */
export function countFindings(found: readonly ScriptDiagnostic[]): { errors: number; warnings: number } {
	const errors = found.filter((d) => d.severity === 'error').length;
	return { errors, warnings: found.length - errors };
}

export interface CheckOptions {
	/** Asks the backend about the text. */
	check(source: string): Promise<ScriptDiagnostic[]>;
	/** Called with every answer, for the footer. */
	onResult?(found: ScriptDiagnostic[]): void;
	/** Milliseconds to wait after the last edit before asking. */
	delay?: number;
}

/** Asks the backend after each pause in typing, then marks what it finds. A failed check leaves the marks as they were. */
export function checkExtension({ check, onResult, delay = 400 }: CheckOptions): Extension[] {
	return [
		lintGutter(),
		linter(
			async (view) => {
				try {
					const found = await check(view.state.doc.toString());
					onResult?.(found);
					return toLintDiagnostics(view.state.doc, found);
				} catch {
					return [];
				}
			},
			{ delay },
		),
	];
}
