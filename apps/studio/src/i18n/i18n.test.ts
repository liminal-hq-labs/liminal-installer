// @vitest-environment happy-dom
// Tests the message catalogue: lookups, plurals, the pseudo-locales, parity and the locale loader.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it } from 'vitest';
import { findJsxLiterals } from '../../scripts/jsx-literals';
import { resetActiveLocale, setActiveLocale } from './active';
import ar from './catalogues/ar-XB';
import en from './catalogues/en-XA';
import { catalogueParity, parityFailures, tokens } from './catalogueParity';
import { loadLocale } from './loadCatalogue';
import { resolveLocale } from './locales';
import { enMessages, t, tn } from './messages';
import { accentMessage, rightToLeftMessage } from './pseudo';

afterEach(() => resetActiveLocale());

describe('t and tn', () => {
	it('fills {tokens}, and leaves an unknown one visible', () => {
		expect(t('editor.status.position', { line: 4, column: 9 })).toBe('Ln 4, Col 9');
		expect(t('editor.status.position', { line: 4 })).toBe('Ln 4, Col {column}');
	});

	it('picks the plural form, formats the count and falls back to English for a missing key', () => {
		expect(tn('problems.errors', 1)).toBe('1 error');
		expect(tn('problems.errors', 2)).toBe('2 errors');
		expect(tn('problems.errors', 1200)).toBe('1,200 errors');
		setActiveLocale('en-CA', { 'problems.errors.other': '{count} fautes' });
		expect(tn('problems.errors', 3)).toBe('3 fautes');
		expect(tn('problems.errors', 1)).toBe('1 error');
	});
});

describe('the pseudo-locales', () => {
	it('accent, lengthen and bracket a message but leave {tokens} alone', () => {
		const out = accentMessage('Ln {line}, Col {column}');
		expect(out.startsWith('[')).toBe(true);
		expect(out.endsWith(']')).toBe(true);
		expect(out).toContain('{line}');
		expect(out).toContain('{column}');
		expect(out.length).toBeGreaterThan('Ln {line}, Col {column}'.length + 2);
		expect(out).not.toContain('Ln ');
	});

	it('wrap a message right to left, with Arabic punctuation and marks around tokens', () => {
		const out = rightToLeftMessage('Why, {name}?');
		expect(out.startsWith('‫')).toBe(true);
		expect(out.endsWith('‬')).toBe(true);
		expect(out).toContain('،');
		expect(out).toContain('؟');
		expect(out).toContain('‏{name}‏');
	});

	it('are generated for every English message', () => {
		expect(parityFailures('en-XA', en)).toEqual([]);
		expect(parityFailures('ar-XB', ar)).toEqual([]);
	});

	it('only load in a development build, and set the direction of the page', async () => {
		const root = document.createElement('html');
		expect(await loadLocale('?locale=ar-XB', false, root)).toBe('en-CA');
		expect(root.dir).toBe('ltr');
		expect(t('common.close')).toBe('Close');
		expect(await loadLocale('?locale=ar-XB', true, root)).toBe('ar-XB');
		expect(root.dir).toBe('rtl');
		expect(t('common.close')).toContain('Close');
		expect(t('common.close')).not.toBe('Close');
		expect(await loadLocale('?locale=en-XA', true, root)).toBe('en-XA');
		expect(t('common.close')).toMatch(/^\[Çĺöšé/);
		expect(resolveLocale('fr-CA', true)).toBe('en-CA');
	});
});

describe('parity', () => {
	it('reports a missing key, a stray key and a token mismatch', () => {
		const parity = catalogueParity({
			'common.close': 'Fermer {x}',
			'no.such.key': 'x',
		} as never);
		expect(parity.missing).toContain('common.cancel');
		expect(parity.extra).toEqual(['no.such.key']);
		expect(parity.tokenMismatch).toEqual(['common.close']);
		expect(tokens('{b} and {a}')).toBe('a,b');
	});

	it('has an .other for every .one, and no empty message', () => {
		for (const [key, value] of Object.entries(enMessages)) {
			if (key.endsWith('.one')) expect(`${key.slice(0, -4)}.other` in enMessages).toBe(true);
			expect(value.length).toBeGreaterThan(0);
		}
	});
});

describe('findJsxLiterals', () => {
	it('finds JSX text with its line and column', () => {
		const found = findJsxLiterals('const a = (\n\t<p>Hello there</p>\n);');
		expect(found).toEqual([{ line: 2, column: 5, where: 'text', text: 'Hello there' }]);
	});

	it('finds literals in visible attributes, braced or not', () => {
		const found = findJsxLiterals(
			`const a = <input title="Name" placeholder={'Type'} aria-label={\`Close\`} alt="Pic" description="Words" />;`,
		);
		expect(found.map((f) => f.where)).toEqual([
			'title',
			'placeholder',
			'aria-label',
			'alt',
			'description',
		]);
	});

	it('finds a string expression used as a child', () => {
		expect(findJsxLiterals(`const a = <p>{'Hello'}</p>;`)).toHaveLength(1);
	});

	it('accepts catalogue lookups, other attributes, punctuation and whitespace', () => {
		const source = [
			`const a = (`,
			`\t<div className="box" data-testid="window-label" aria-label={t('x')} title={name}>`,
			`\t\t{t('app.name')} · {count} —`,
			`\t\t<span>{value}</span>`,
			`\t\t{'<'}`,
			`\t</div>`,
			`);`,
		].join('\n');
		expect(findJsxLiterals(source)).toEqual([]);
	});
});
