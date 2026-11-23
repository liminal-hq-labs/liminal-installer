// Converts src/theme/tokens.json, the Liminal HQ design system export, into src/theme/tokens.css, with `--lis-*` names.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

/** The theme every unthemed value belongs to, and the default look of the Studio. */
export const BASE_THEME = 'afterglow';

/** Colour families that belong to other products' palettes. The Studio does not use them. */
export const SKIPPED_COLOUR_PREFIXES = ['wp-', 'cadence-', 'calm-', 'booth-', 'term-', 'traffic-'];

/** Token groups that hold plain `{ name, value }` lists, each emitted as `--lis-<name>`. */
const PLAIN_GROUPS = [
	'spacing',
	'radius',
	'shadow',
	'blur',
	'layout',
	'font-size',
	'font-weight',
	'line-height',
	'letter-spacing',
];

/** Turns `{brand-purple}` references into `var(--lis-brand-purple)` so a theme change follows them. */
export function resolveRefs(value) {
	return String(value).replace(/\{([a-z0-9-]+)\}/g, (_, name) => `var(--lis-${name})`);
}

/** The value a token has in `theme`, or undefined when that theme does not override it. */
export function valueIn(token, theme) {
	const v = token.value;
	if (v !== null && typeof v === 'object')
		return v[theme] === undefined ? undefined : resolveRefs(v[theme]);
	return theme === BASE_THEME ? resolveRefs(v) : undefined;
}

const block = (selector, vars) =>
	`${selector} {\n${vars.map(([k, v]) => `\t${k}: ${v};`).join('\n')}\n}\n`;

const colourTokens = (tokens) =>
	tokens.color.tokens.filter((t) => !SKIPPED_COLOUR_PREFIXES.some((p) => t.name.startsWith(p)));

/** The variables a theme sets: every colour it overrides, and for the base theme all other groups too. */
export function themeVars(tokens, theme) {
	const vars = [];
	for (const t of colourTokens(tokens)) {
		const v = valueIn(t, theme);
		if (v !== undefined) vars.push([`--lis-${t.name}`, v]);
	}
	if (theme === BASE_THEME) {
		for (const group of PLAIN_GROUPS)
			for (const t of tokens[group].tokens) vars.push([`--lis-${t.name}`, String(t.value)]);
		for (const [name, family] of Object.entries(tokens.type.families))
			vars.push([`--lis-font-${name}`, family]);
	}
	return vars;
}

/** The stylesheet: the base theme on `:root`, one block per other theme under `[data-theme]`, then `mapping` (the chrome's `--lic-*` variables and the editor's syntax colours). */
export function toCss(tokens, mapping = '') {
	let css =
		'/* Liminal HQ design tokens, generated from tokens.json by scripts/tokens-to-css.mjs. */\n/*\n * (c) Copyright 2026 Liminal HQ, Scott Morris\n * SPDX-License-Identifier: Apache-2.0 OR MIT\n */\n\n';
	css += block(`:root,\n:root[data-theme="${BASE_THEME}"]`, [
		['color-scheme', 'dark'],
		...themeVars(tokens, BASE_THEME),
	]);
	for (const theme of tokens.color.themes) {
		if (theme.id === BASE_THEME) continue;
		const scheme = theme.id.endsWith('-light') ? 'light' : 'dark';
		css +=
			'\n' +
			block(`:root[data-theme="${theme.id}"]`, [
				['color-scheme', scheme],
				...themeVars(tokens, theme.id),
			]);
	}
	return mapping ? `${css}\n${mapping}` : css;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	const dir = new URL('../src/theme/', import.meta.url);
	const tokens = JSON.parse(readFileSync(new URL('tokens.json', dir), 'utf8'));
	const mapping = readFileSync(new URL('chrome-map.css', dir), 'utf8');
	writeFileSync(new URL('tokens.css', dir), toCss(tokens, mapping));
}
