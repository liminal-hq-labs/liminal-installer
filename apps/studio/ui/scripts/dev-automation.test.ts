// Checks that the debug-only automation bridge and the global Tauri object stay out of release builds.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync, readdirSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const tauri = new URL('../../src-tauri/', import.meta.url);
const read = (path: string) => readFileSync(new URL(path, tauri), 'utf8');
const json = (path: string) => JSON.parse(read(path)) as Record<string, any>;

/** Every window label the Studio opens: the main window and the specs in `windows.rs`. */
function windowLabels(): string[] {
	const fixed = [...read('src/windows.rs').matchAll(/label: "([a-z-]+)"\.into\(\)/g)].map((m) => m[1]!);
	const prefixed = [...read('src/windows.rs').matchAll(/label: format!\("([a-z-]+)-\{id\}"\)/g)].map((m) => `${m[1]}-*`);
	return ['main', ...fixed, ...prefixed];
}

describe('dev automation', () => {
	it('keeps the global Tauri object off in the release config', () => {
		expect(json('tauri.conf.json').app.withGlobalTauri ?? false).toBe(false);
	});

	it('turns it on only in the dev config, and changes nothing else there', () => {
		expect(json('tauri.conf.dev.json')).toEqual({ app: { withGlobalTauri: true } });
	});

	it('registers the bridge only in debug builds, and not on Windows', () => {
		const lib = read('src/lib.rs');
		const gated = /#\[cfg\(all\(debug_assertions, not\(windows\)\)\)\]\s*\{\s*builder = builder\.plugin\(tauri_plugin_mcp_bridge::init\(\)\);\s*\}/;
		expect(lib).toMatch(gated);
		expect(lib.match(/tauri_plugin_mcp_bridge/g)).toHaveLength(1);
	});

	it('depends on the bridge only off Windows', () => {
		const cargo = read('Cargo.toml');
		const section = cargo.slice(cargo.indexOf("[target.'cfg(not(windows))'.dependencies]"));
		expect(section).toContain('tauri-plugin-mcp-bridge');
		expect(cargo.slice(0, cargo.indexOf("[target.'cfg(not(windows))'.dependencies]"))).not.toContain('tauri-plugin-mcp-bridge');
	});

	it('grants the bridge to every window, on Linux and macOS only', () => {
		const cap = json('capabilities/dev-automation.json');
		expect(cap.identifier).toBe('dev-automation');
		expect(cap.platforms).toEqual(['linux', 'macOS']);
		expect(cap.permissions).toEqual(['mcp-bridge:default']);
		expect(cap.description).toMatch(/nothing in a release build/);
		for (const label of windowLabels()) expect(cap.windows, label).toContain(label);
	});

	it('gives no other capability the bridge', () => {
		for (const file of readdirSync(new URL('capabilities/', tauri))) {
			if (file === 'dev-automation.json') continue;
			expect(JSON.stringify(json(`capabilities/${file}`).permissions)).not.toContain('mcp-bridge');
		}
	});

	it('lists every child window in the default capability too', () => {
		const cap = json('capabilities/default.json');
		for (const label of windowLabels()) expect(cap.windows, label).toContain(label);
	});
});
