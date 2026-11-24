// An in-browser stand-in for the backend, built from an example project, for development.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { lex } from '../editor/luaLex';
import { validateFlow } from './flowEdit';
import { resolveForRenderer } from './layoutEdit';
import { composeScreen } from '@liminal-hq/installer-renderer';
import type { Api } from './api';
import { demoMedia } from './demoMedia';
import { MEDIA_DOC } from './media';
import { demoTauri } from './demoTauri';
import { demoTerminal } from './demoTerminal';
import type {
	ChangeEvent,
	DocKey,
	Flow,
	HistoryEntry,
	Layout,
	Preview,
	Problem,
	ProjectSummary,
	ScriptDiagnostic,
	ScriptReport,
	Source,
	TargetInfo,
} from './types';

const files = import.meta.glob(
	'../../../../examples/lt-fizz/installer/{*.toml,flows/*.json,layouts/*.json,themes/*/layouts/*.json,scripts/*.lua}',
	{
		query: '?raw',
		import: 'default',
		eager: true,
	},
) as Record<string, string>;

const byName = (suffix: string) => Object.entries(files).filter(([k]) => k.includes(suffix));
const stem = (path: string) =>
	path
		.split('/')
		.pop()!
		.replace(/\.(flow|layout)\.json$/, '');
/** A layout's id: the file stem, with a theme's own layouts prefixed `theme/`. */
const layoutId = (path: string) => {
	const theme = /\/themes\/([^/]+)\/layouts\//.exec(path)?.[1];
	return theme ? `${theme}/${stem(path)}` : stem(path);
};

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

type Doc = DocKey;
/** The media document has no variant in the generated `DocKey` yet (D84). */
const isMedia = (d: Doc): boolean => (d as { type: string }).type === 'media';
interface Entry extends HistoryEntry {
	before: string | undefined;
	after: string | undefined;
}

export function demoApi(): Api {
	const flows = new Map<string, Flow>(
		byName('/flows/').map(([k, v]) => [stem(k), JSON.parse(v) as Flow]),
	);
	const layouts = new Map<string, Layout>(
		byName('/layouts/').map(([k, v]) => [
			layoutId(k),
			{ ...(JSON.parse(v) as Layout), id: layoutId(k) },
		]),
	);
	const scripts = new Map<string, string>(
		byName('/scripts/').map(([k, v]) => [`scripts/${k.split('/').pop()}`, v]),
	);
	const installer = byName('installer.toml')[0]?.[1] ?? '';
	const components = byName('components.toml')[0]?.[1] ?? '';
	const listeners = new Set<(e: ChangeEvent) => void>();
	let entries: Entry[] = [];
	let cursor = 0;
	let preview: Preview | null = null;

	const tauri = demoTauri();
	const media = demoMedia();
	const read = (d: Doc): string | undefined =>
		isMedia(d)
			? media.snapshot()
			: d.type === 'tauri'
				? tauri.snapshot()
				: d.type === 'flow'
					? json(flows.get(d.id))
					: d.type === 'layout'
						? json(layouts.get(d.id))
						: scripts.get(d.id);
	const json = (v: unknown) => (v === undefined ? undefined : JSON.stringify(v));
	const write = (d: Doc, text: string | undefined) => {
		if (isMedia(d)) media.restore(text);
		else if (d.type === 'tauri') tauri.restore(text);
		else if (d.type === 'script')
			text === undefined ? scripts.delete(d.id) : scripts.set(d.id, text);
		else {
			const map = (d.type === 'flow' ? flows : layouts) as Map<string, unknown>;
			text === undefined ? map.delete(d.id) : map.set(d.id, JSON.parse(text));
		}
	};
	const view = () => ({
		entries: entries.map((e, i) => ({ ...e, index: i, applied: i < cursor })),
		can_undo: cursor > 0,
		can_redo: cursor < entries.length,
	});
	const emit = (kind: ChangeEvent['kind'], doc: Doc, label: string, source: Source) =>
		listeners.forEach((l) =>
			l({ kind, doc, label, source, can_undo: cursor > 0, can_redo: cursor < entries.length }),
		);
	const edit = (doc: Doc, after: string, label: string, before: string | undefined = read(doc)) => {
		if (before === after) return;
		write(doc, after);
		entries = [
			...entries.slice(0, cursor),
			{ index: 0, label, source: 'ui', doc, applied: true, before, after },
		];
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
				components: comps
					.filter((c) => !c.targets || list(c.targets).includes(id))
					.map((c) => unquote(c.id)),
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
			{ id: 'fizz-modern', name: 'Fizz Windows 11', renderers: ['web', 'canvas'] },
		],
		flows: [...flows.values()].map((f) => ({
			id: f.id,
			steps: f.steps.length,
			errors: validateFlow(f).length,
		})),
		layouts: [...layouts.keys()],
		scripts: [...scripts.keys()],
	});
	const report = (path: string, source: string, target?: string): ScriptReport => {
		const hooks = [
			['detect', 'A page opens'],
			['validate', 'Before Next'],
			['on_commit', 'Plan is built'],
			['on_uninstall', 'Uninstall plan is built'],
		].map(([name, when]) => ({
			name: name!,
			when: when!,
			present: source.includes(`function ${name}(`),
		}));
		const header = source
			.split('\n')
			.filter((l) => /^\s*--/.test(l))
			.flatMap((l) => (/capabilities:(.*)$/.exec(l)?.[1] ?? '').split(/[,\s]+/).filter(Boolean));
		const capabilities = [...new Set(header.map((c) => `li.${c.split('.')[0]}`))].sort();
		const diagnostics: Problem[] = hooks.some((h) => h.present)
			? []
			: [{ severity: 'error', scope: `script:${path}`, message: 'the script defines no hook' }];
		const present = hooks.filter((h) => h.present);
		return {
			ok: diagnostics.length === 0,
			hooks,
			capabilities,
			diagnostics,
			output: present.map((h) => `${h.name}: ok (demo run)`),
			target: target ?? null,
			tests: present.map((h) => ({
				hook: h.name,
				ok: true,
				summary: 'ok (demo run)',
				lines: target ? [`sample: ${target}`] : [],
			})),
		};
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
		testScript: async (path, source, target) => {
			await new Promise((resolve) => setTimeout(resolve, 150));
			return report(path, source ?? scripts.get(path) ?? '', target);
		},
		checkScriptSyntax: async (source) => demoCheck(source),
		runPreview: async (target) => {
			const flow = flows.get('install')!;
			const [targetId = '', renderer = 'canvas'] = target.split(':');
			const info = targets().find((t) => t.full_id === target);
			const theme = info?.theme ?? 'fizz-win95';
			const comps = summary().components.filter(
				(c) => !c.targets.length || c.targets.includes(targetId),
			);
			const slug = 'LT-FIZZ';
			const winish = targetId.startsWith('dos') || targetId.startsWith('win');
			const vars: Record<string, string> = {
				'product.id': 'hq.liminal.lt-fizz',
				'product.name': 'Lieutenant Fizz — Episode One',
				'product.publisher': 'Liminal HQ',
				'product.version': '1.0.0',
				'target.id': targetId,
				'target.renderer': renderer,
				'install.path': winish ? `C:\\${slug}` : `~/.local/share/${slug.toLowerCase()}`,
				'install.size': '48 MB',
				'system.free_space': '12.4 GB',
				'system.free_space_after': '12.3 GB',
				'progress.file': winish ? 'C:\\LT-FIZZ\\SOUND\\SB16.DRV' : 'sound/SB16.DRV',
				'progress.percent': '55',
				'plan.summary': `${comps.length} components · 41 files · 48 MB`,
				'sound.detected': 'Sound Blaster',
				'validate.problems': '',
			};
			const state = {
				'components.selected': comps.map((c) => c.id),
				'licence.accepted': 'yes',
				'install.scope': 'user',
				'finish.launch': ['launch'],
				'sound.card': 'sbpro',
				'sound.port': '220',
				'sound.irq': '5',
				'sound.dma': '1',
				'sound.cards': [
					{ id: 'none', name: 'No sound' },
					{ id: 'speaker', name: 'PC speaker' },
					{ id: 'adlib', name: 'AdLib music card' },
					{ id: 'sbpro', name: 'Sound Blaster / Pro / 16' },
					{ id: 'gus', name: 'Gravis UltraSound' },
				],
			};
			const stageLayout = layouts.get(`${theme}/stage`);
			const steps = flow.steps.map((s) => {
				const skipped = Boolean(s.when && !s.when.includes(`'${renderer}'`));
				const layout = s.layout
					? (layouts.get(`${theme}/${s.layout}`) ?? layouts.get(s.layout))
					: undefined;
				return {
					id: s.id,
					kind: s.type,
					layout: s.layout ?? null,
					skipped,
					screenshot: null,
					layout_tree:
						layout && !skipped
							? composeScreen(
									stageLayout ? resolveForRenderer(stageLayout.root, renderer) : null,
									resolveForRenderer(layout.root, renderer),
									{ id: s.id, kind: s.type },
								).root
							: undefined,
					state,
				};
			});
			const next: Preview = {
				target,
				renderer,
				theme,
				product: {
					id: vars['product.id']!,
					name: vars['product.name']!,
					version: '1.0.0',
					publisher: 'Liminal HQ',
				},
				vars,
				plan: {
					ops: [
						{ op: 'mkdir', path: '{install}' },
						{ op: 'copy', from: 'dist/FIZZ.EXE', to: '{install}/FIZZ.EXE', sha256: null },
						{ op: 'receipt.write', path: '{install}/.liminal/receipt.json' },
					],
				},
				problems: [],
				steps,
			};
			preview = next;
			return next;
		},
		getPreview: async () => preview,
		// The renderer is Rust, so the browser demo draws a stand-in screen.
		...demoTerminal(),
		checkProject: async () => [...flows.values()].flatMap((f) => validateFlow(f)),
		history: async () => view(),
		undo: async () => step(-1),
		redo: async () => step(1),
		listMedia: async () => media.list(),
		getMediaPlan: async (id, overrides) => media.plan(id, overrides),
		addMedia: async (kind, label) => {
			const before = media.snapshot();
			const added = media.add(kind);
			edit(MEDIA_DOC, media.snapshot(), label, before);
			return added;
		},
		setMedia: async (id, change, label) => {
			const before = media.snapshot();
			const next = media.set(id, change);
			edit(MEDIA_DOC, media.snapshot(), label, before);
			return next;
		},
		removeMedia: async (id, label) => {
			const before = media.snapshot();
			media.remove(id);
			edit(MEDIA_DOC, media.snapshot(), label, before);
		},
		pickReadme: async () => 'media/README.TXT',
		getTauri: async () => tauri.view(),
		importTauri: async (_config, label) => {
			const before = tauri.snapshot();
			const result = tauri.importConfig();
			edit({ type: 'tauri' }, tauri.snapshot(), label, before);
			return result;
		},
		setTauriSettings: async (change, label) => {
			const before = tauri.snapshot();
			const next = tauri.set(change);
			edit({ type: 'tauri' }, tauri.snapshot(), label, before);
			return next;
		},
		mcpInfo: async () => null,
		mcpError: async () => null,
		onChange: async (handler) => {
			listeners.add(handler);
			return () => listeners.delete(handler);
		},
		pickProject: async () => null,
		pickTauriConfig: async () => 'src-tauri/tauri.conf.json',
	};
}

/**
 * A stand-in for the backend's check, for the browser demo only: it balances blocks and warns about modules used without
 * their capability. The desktop shell compiles the source with Lua and reports far more.
 */
export function demoCheck(source: string): ScriptDiagnostic[] {
	const found: ScriptDiagnostic[] = [];
	const lines = lex(source);
	const stack: { word: string; line: number; col: number }[] = [];
	const pairs: Record<string, string> = { ')': '(', ']': '[', '}': '{' };
	for (const [index, line] of lines.entries()) {
		if (line.startsInside) continue;
		for (const t of line.tokens) {
			const at = { line: index + 1, column: t.from + 1, end_line: index + 1, end_column: t.to + 1 };
			if (['function', 'if', 'do', 'repeat', '(', '[', '{'].includes(t.text))
				stack.push({ word: t.text, line: index + 1, col: t.from + 1 });
			else if (['end', 'until', ')', ']', '}'].includes(t.text)) {
				const top = stack[stack.length - 1];
				const wants = pairs[t.text];
				const ok =
					top &&
					(wants
						? top.word === wants
						: t.text === 'until'
							? top.word === 'repeat'
							: ['function', 'if', 'do'].includes(top.word));
				if (ok) stack.pop();
				else
					found.push({
						severity: 'error',
						code: 'syntax',
						message: `unexpected symbol near '${t.text}'`,
						...at,
					});
			}
		}
	}
	for (const open of stack) {
		const last = Math.max(lines.length - 1, 1);
		const closers: Record<string, string> = {
			repeat: "'until'",
			'(': "')'",
			'[': "']'",
			'{': "'}'",
		};
		const closer = closers[open.word] ?? "'end'";
		found.push({
			severity: 'error',
			code: 'syntax',
			message: `${closer} expected (to close '${open.word}' at line ${open.line}) near <eof>`,
			line: last,
			column: 1,
			end_line: last,
			end_column: (lines[last - 1]?.text.length ?? 0) + 2,
		});
	}
	if (found.length > 0) return found;
	const declared = new Set(
		source
			.split('\n')
			.filter((l) => /^\s*--/.test(l))
			.flatMap((l) => (/capabilities:(.*)$/.exec(l)?.[1] ?? '').split(/[,\s]+/).filter(Boolean)),
	);
	const need: Record<string, string> = {
		env: 'env',
		fs: 'fs.read',
		ui: 'ui',
		plan: 'plan',
		net: 'net',
	};
	const warned = new Set<string>();
	for (const [index, line] of lines.entries()) {
		if (line.startsInside) continue;
		line.tokens.forEach((t, i) => {
			const m = line.tokens[i + 2];
			if (
				t.text !== 'li' ||
				line.tokens[i + 1]?.text !== '.' ||
				!m ||
				!(m.text in need) ||
				warned.has(m.text)
			)
				return;
			const cap = need[m.text]!;
			if (declared.has(cap) || (cap === 'fs.read' && declared.has('fs.write'))) return;
			warned.add(m.text);
			found.push({
				severity: 'warning',
				code: 'undeclared-capability',
				message: `\`li.${m.text}\` needs the \`${cap}\` capability; add \`-- capabilities: ${cap}\` at the top of the file`,
				line: index + 1,
				column: t.from + 1,
				end_line: index + 1,
				end_column: m.to + 1,
			});
		});
	}
	return found;
}
