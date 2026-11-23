// Fails when the renderer has a visible string literal in JSX, or a message nothing uses.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readdirSync, readFileSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';
import { rendererMessages } from '../src/i18n/messages';
import { findJsxLiterals } from './jsx-literals';

// Run with `bun scripts/check-i18n.ts` from `packages/renderer` (`bun run check:i18n`).

const root = resolve(import.meta.dirname, '..');

function sources(directory: string): string[] {
	const files: string[] = [];
	for (const entry of readdirSync(directory, { withFileTypes: true })) {
		const path = join(directory, entry.name);
		if (entry.isDirectory()) files.push(...sources(path));
		else if (/\.tsx?$/.test(entry.name) && !/\.test\.tsx?$/.test(entry.name)) files.push(path);
	}
	return files.sort();
}

const problems: string[] = [];
const files = sources(join(root, 'src'));
for (const path of files.filter((f) => f.endsWith('.tsx'))) {
	for (const literal of findJsxLiterals(readFileSync(path, 'utf8'), path)) {
		problems.push(
			`${relative(root, path)}:${literal.line}:${literal.column}  ${literal.where} literal "${literal.text}" should come from t()`,
		);
	}
}
const text = files
	.filter((f) => !f.endsWith(join('i18n', 'messages.ts')))
	.map((f) => readFileSync(f, 'utf8'))
	.join('\n');
for (const key of Object.keys(rendererMessages)) {
	if (!text.includes(`'${key}'`)) problems.push(`${key} is never used`);
}
if (problems.length > 0) {
	console.error('The renderer sources and its messages disagree:');
	for (const problem of problems) console.error(`  ${problem}`);
	process.exit(1);
}
console.log(
	`No visible string literals in JSX, ${Object.keys(rendererMessages).length} messages, all used.`,
);
