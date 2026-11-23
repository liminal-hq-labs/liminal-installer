// An in-browser stand-in for the backend, built from an example project, for development.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { validateFlow } from './flowEdit';
import { resolveForRenderer } from './layoutEdit';
import { renderLayoutHtml } from './previewRender';
import type {
	Api,
} from './api';
import type {
	ChangeEvent,
	Flow,
	HistoryEntry,
	Layout,
	Preview,
	Problem,
	ProjectSummary,
	ScriptReport,
	Source,
	TargetInfo,
} from './types';

const files = import.meta.glob('../../../../../examples/lt-fizz/installer/{*.toml,flows/*.json,layouts/*.json,scripts/*.lua}', {
	query: '?raw',
	import: 'default',
	eager: true,
}) as Record<string, string>;

const byName = (suffix: string) => Object.entries(files).filter(([k]) => k.includes(suffix));
const stem = (path: string) => path.split('/').pop()!.replace(/\.(flow|layout)\.json$/, '');

/** Reads `[[name]]` tables of `key = value` lines; enough for the example manifests. */
function tables(text: string, name: string): Record<string, string>[] {
	const out: Record<string, string>[] = [];
	let current: Record<string, string> | null = null;
	for (const line of text.split('\n')) {
		const t = line.trim();
		if (t.startsWith('[[')) current = t === `[[${name}]]` ? (out[out.push({}) - 1] ?? null) : null;
		else if (t.startsWith('[')) current = null;
		else if (current && t.includes('=')) {
			const i = t.indexOf('=');
			current[t.slice(0, i).trim()] = t.slice(i + 1).trim();
		}
	}
	return out;
}

const unquote = (s: string | undefined) => (s ?? '').replace(/^"|"$/g, '');
const list = (s: string | undefined) => [...(s ?? '').matchAll(/"([^"]*)"/g)].map((m) => m[1]!);

type Doc = { type: 'flow' | 'layout' | 'script'; id: string };
interface Entry extends HistoryEntry {
	before: string | undefined;
	after: string | undefined;
}

export function demoApi(): Api {
	const flows = new Map<string, Flow>(byName('/flows/').map(([k, v]) => [stem(k), JSON.parse(v) as Flow]));
	const layouts = new Map<string, Layout>(byName('/layouts/').map(([k, v]) => [stem(k), JSON.parse(v) as Layout]));
	const scripts = new Map<string, string>(byName('/scripts/').map(([k, v]) => [`scripts/${k.split('/').pop()}`, v]));
	const installer = byName('installer.toml')[0]?.[1] ?? '';
	const components = byName('components.toml')[0]?.[1] ?? '';
	const listeners = new Set<(e: ChangeEvent) => void>();
	let entries: Entry[] = [];
	let cursor = 0;
	let preview: Preview | null = null;

	const read = (d: Doc): string | undefined =>
		d.type === 'flow' ? json(flows.get(d.id)) : d.type === 'layout' ? json(layouts.get(d.id)) : scripts.get(d.id);
	const json = (v: unknown) => (v === undefined ? undefined : JSON.stringify(v));
	const write = (d: Doc, text: string | undefined) => {
		if (d.type === 'script') text === undefined ? scripts.delete(d.id) : scripts.set(d.id, text);
		else {
			const map = (d.type === 'flow' ? flows : layouts) as Map<string, unknown>;
			text === undefined ? map.delete(d.id) : map.set(d.id, JSON.parse(text));
		}
	};
	const view = () => ({ entries: entries.map((e, i) => ({ ...e, index: i, applied: i < cursor })), can_undo: cursor > 0, can_redo: cursor < entries.length });
	const emit = (kind: ChangeEvent['kind'], doc: Doc, label: string, source: Source) =>
		listeners.forEach((l) => l({ kind, doc, label, source, can_undo: cursor > 0, can_redo: cursor < entries.length }));
	const edit = (doc: Doc, after: string, label: string) => {
		const before = read(doc);
		if (before === after) return;
		write(doc, after);
		entries = [...entries.slice(0, cursor), { index: 0, label, source: 'ui', doc, applied: true, before, after }];
		cursor = entries.length;
		preview = null;
		emit('edit', doc, label, 'ui');
	};
	const step = (dir: -1 | 1) => {
		const entry = dir < 0 ? entries[cursor - 1] : entries[cursor];
		if (!entry) throw new Error(`nothing to ${dir < 0 ? 'undo' : 'redo'}`);
		write(entry.doc, dir < 0 ? entry.before : entry.after);
		cursor += dir;
		preview = null;
		emit(dir < 0 ? 'undo' : 'redo', entry.doc, entry.label, 'ui');
		return view();
	};

	const targets = (): TargetInfo[] => {
		const comps = tables(components, 'component');
		return tables(installer, 'target').map((t) => {
			const id = unquote(t.id);
			const renderer = unquote(t.renderer);
			return {
				full_id: `${id}:${renderer}`,
				id,
				renderer,
				theme: unquote(t.theme) || 'fizz-win95',
				outputs: list(t.outputs),
				components: comps.filter((c) => !c.targets || list(c.targets).includes(id)).map((c) => unquote(c.id)),
			};
		});
	};
	const summary = (): ProjectSummary => ({
		root: 'examples/lt-fizz/installer (demo)',
		id: 'hq.liminal.lt-fizz',
		name: 'Lieutenant Fizz — Episode One',
		publisher: 'Liminal HQ',
		version: 'from git-tag',
		default_theme: 'fizz-win95',
		targets: targets(),
		components: tables(components, 'component').map((c) => ({
			id: unquote(c.id),
			name: unquote(c.name),
			required: c.required === 'true',
			targets: list(c.targets),
			files: 1,
		})),
		themes: [
			{ id: 'fizz-dos', name: 'Fizz DOS', renderers: ['tui-retro'] },
			{ id: 'fizz-win95', name: 'Fizz Windows 95', renderers: ['win16', 'win32-classic', 'tui'] },
		],
		flows: [...flows.values()].map((f) => ({ id: f.id, steps: f.steps.length, errors: validateFlow(f).length })),
		layouts: [...layouts.keys()],
		scripts: [...scripts.keys()],
	});
	const report = (path: string, source: string): ScriptReport => {
		const hooks = [
			['detect', 'A page opens'],
			['validate', 'Before Next'],
			['on_commit', 'Plan is built'],
			['on_uninstall', 'Uninstall plan is built'],
		].map(([name, when]) => ({ name: name!, when: when!, present: source.includes(`function ${name}(`) }));
		const capabilities = [...new Set([...source.matchAll(/require\s*\(?\s*["'](li\.[a-z.]+)["']/g)].map((m) => m[1]!))];
		const diagnostics: Problem[] = hooks.some((h) => h.present)
			? []
			: [{ severity: 'error', scope: `script:${path}`, message: 'the script defines no hook' }];
		return { ok: diagnostics.length === 0, hooks, capabilities, diagnostics, output: hooks.filter((h) => h.present).map((h) => `${h.name}: ok (demo run)`) };
	};

	return {
		openProject: async () => summary(),
		projectSummary: async () => summary(),
		listTargets: async () => targets(),
		getFlow: async (id) => {
			const flow = flows.get(id);
			if (!flow) throw new Error(`flow "${id}" not found`);
			return { flow, errors: validateFlow(flow) };
		},
		setFlow: async (id, flow, label) => {
			edit({ type: 'flow', id }, JSON.stringify(flow), label);
			return { flow, errors: validateFlow(flow) };
		},
		getLayout: async (id, renderer) => {
			const l = layouts.get(id);
			if (!l) throw new Error(`layout "${id}" not found`);
			return renderer ? { ...l, root: resolveForRenderer(l.root, renderer) } : l;
		},
		setLayout: async (id, layout, label) => {
			edit({ type: 'layout', id }, JSON.stringify(layout), label);
			return layout;
		},
		readScript: async (path) => {
			const s = scripts.get(path);
			if (s === undefined) throw new Error(`script "${path}" not found`);
			return s;
		},
		writeScript: async (path, source, label) => edit({ type: 'script', id: path }, source, label),
		testScript: async (path, source) => report(path, source ?? scripts.get(path) ?? ''),
		runPreview: async (target) => {
			const flow = flows.get('install')!;
			const renderer = target.split(':')[1] ?? 'canvas';
			const steps = flow.steps.map((s) => {
				const skipped = Boolean(s.when && !s.when.includes(`'${renderer}'`));
				const layout = s.layout ? layouts.get(s.layout) : undefined;
				return {
					id: s.id,
					kind: s.type,
					layout: s.layout ?? null,
					skipped,
					screenshot:
						layout && !skipped
							? {
									mime: 'image/svg+xml',
									encoding: 'utf8' as const,
									data: `<svg xmlns="http://www.w3.org/2000/svg" width="480" height="60"><rect width="480" height="60" fill="#0c0c10"/><text x="12" y="34" fill="#e0e0e0" font-family="monospace">${s.id} (${renderLayoutHtml(layout, renderer).length} bytes of HTML)</text></svg>`,
								}
							: null,
				};
			});
			preview = {
				target,
				plan: { ops: [{ op: 'mkdir', path: '{install}' }, { op: 'copy', from: 'dist/FIZZ.EXE', to: '{install}/FIZZ.EXE' }, { op: 'receipt.write', path: '{install}/.liminal/receipt.json' }] },
				problems: [],
				steps,
			};
			return preview;
		},
		getPreview: async () => preview,
		checkProject: async () => [...flows.values()].flatMap((f) => validateFlow(f)),
		history: async () => view(),
		undo: async () => step(-1),
		redo: async () => step(1),
		mcpInfo: async () => null,
		onChange: async (handler) => {
			listeners.add(handler);
			return () => listeners.delete(handler);
		},
		pickProject: async () => null,
	};
}
