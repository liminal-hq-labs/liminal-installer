// Fails when a component has a visible string literal, or the message catalogue is out of step.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readdirSync, readFileSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';
import accented from '../src/i18n/catalogues/en-XA';
import mirrored from '../src/i18n/catalogues/ar-XB';
import { parityFailures } from '../src/i18n/catalogueParity';
import { enMessages } from '../src/i18n/messages';
import { findJsxLiterals } from '@liminal-hq/installer-renderer/scripts/jsx-literals';
import { rendererMessages } from '@liminal-hq/installer-renderer';

// Run with `bun scripts/check-i18n.ts` from `apps/studio` (`bun run check:i18n`). Three checks:
//
// 1. Every string a person sees goes through `t()`, so JSX text and the visible attributes (`title`,
//    `label`, `placeholder`, `alt`, `aria-label`, `description` and friends) must not hold a
//    literal. Test files are exempt.
// 2. The pseudo-locales (`en-XA`, `ar-XB`) have exactly the keys and `{tokens}` English has.
// 3. Every message is used: its key, or the base of a plural, appears in the sources.

const app = resolve(import.meta.dirname, '..');
const scanned = join(app, 'src');

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
const files = sources(scanned);

for (const path of files.filter((f) => f.endsWith('.tsx'))) {
	for (const literal of findJsxLiterals(readFileSync(path, 'utf8'), path)) {
		problems.push(
			`${relative(app, path)}:${literal.line}:${literal.column}  ${literal.where} literal "${literal.text}" should come from t()`,
		);
	}
}

for (const [locale, catalogue] of [
	['en-XA', accented],
	['ar-XB', mirrored],
] as const) {
	problems.push(...parityFailures(locale, catalogue));
}
const english = Object.keys(enMessages);
// The renderer's own messages are checked where they are used, by `packages/renderer`'s check.
const shared = new Set(Object.keys(rendererMessages));
for (const key of english) {
	if (key.endsWith('.one') && !(`${key.slice(0, -4)}.other` in enMessages))
		problems.push(`${key} has no .other form`);
}

const text = files
	.filter((f) => !f.includes(`${join('src', 'i18n')}`) || f.includes('chromeLabels'))
	.map((f) => readFileSync(f, 'utf8'))
	.join('\n');
for (const key of english.filter((k) => !shared.has(k))) {
	const base = key.replace(/\.(zero|one|two|few|many|other)$/, '');
	if (!text.includes(`'${key}'`) && !text.includes(`'${base}'`))
		problems.push(`${key} is never used`);
}

if (problems.length > 0) {
	console.error('The message catalogue (apps/studio/src/i18n) and the components disagree:');
	for (const problem of problems) console.error(`  ${problem}`);
	process.exit(1);
}
console.log(`No visible string literals in JSX, ${english.length} messages, all used.`);
