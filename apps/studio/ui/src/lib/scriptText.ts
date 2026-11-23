// Text helpers for the script editor: formatting on save and counting lines.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/**
 * The script as it is saved with Format on save on: one line feed per line, no trailing spaces or tabs, and a single
 * newline at the end. An empty script stays empty.
 */
export function formatScript(source: string): string {
	const lines = source.replace(/\r\n?/g, '\n').split('\n').map((line) => line.replace(/[ \t]+$/, ''));
	while (lines.length > 0 && lines[lines.length - 1] === '') lines.pop();
	return lines.length === 0 ? '' : `${lines.join('\n')}\n`;
}

/** How many lines the text shows: a final newline does not start another. */
export function lineCount(source: string): number {
	return Math.max(1, source.split('\n').length - (source.endsWith('\n') && source.length > 0 ? 1 : 0));
}
