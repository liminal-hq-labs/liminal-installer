// Converts src/styles/tokens.json into CSS custom properties in src/styles/tokens.css.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

/** Flattens `{color: {bg: "x"}}` to `--lh-color-bg: x`. */
export function flatten(group, prefix = '--lh') {
	const out = [];
	for (const [key, value] of Object.entries(group)) {
		if (key === 'themes') continue;
		const name = `${prefix}-${key}`;
		if (value !== null && typeof value === 'object') out.push(...flatten(value, name));
		else out.push([name, String(value)]);
	}
	return out;
}

const block = (selector, vars) =>
	`${selector} {\n${vars.map(([k, v]) => `\t${k}: ${v};`).join('\n')}\n}\n`;

/** The stylesheet: dark by default, with `[data-theme="light"]` and an automatic light variant. */
export function toCss(tokens) {
	let css = '/* Liminal HQ design tokens, generated from tokens.json by scripts/tokens-to-css.mjs. */\n/*\n * (c) Copyright 2026 Liminal HQ, Scott Morris\n * SPDX-License-Identifier: Apache-2.0 OR MIT\n */\n\n';
	css += block(':root,\n:root[data-theme="dark"]', flatten(tokens));
	const light = tokens.themes?.light;
	if (light) {
		const vars = flatten(light);
		css += '\n' + block(':root[data-theme="light"]', vars);
	}
	return css;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	const dir = new URL('../src/styles/', import.meta.url);
	const tokens = JSON.parse(readFileSync(new URL('tokens.json', dir), 'utf8'));
	writeFileSync(new URL('tokens.css', dir), toCss(tokens));
}
