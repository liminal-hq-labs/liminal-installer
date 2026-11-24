// The Studio: the stores, the api and the actions that move data between them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Screens read the stores and call the actions here. The backend arrives as `api`, so a test
// hands in a fake one. An action that can fail runs through `run`, which shows its error as a toast.

import { isStageId, stageIdFor, type Scheme } from '@liminal-hq/installer-renderer';
import type { ChromeStore } from '../chrome/chromeStore';
import type { DialogResult, DialogSpec } from '../chrome/dialogWindow';
import { SCREEN_ACTIONS, type ActionId, type Screen } from '../chrome/menuModel';
import type { ScriptEditor } from '../editor/scriptEditor';
import { formatLua } from '../editor/format';
import { t, tn } from '../i18n/messages';
import { errorMessage, isStudioError, type Api } from '../lib/api';
import type { Settings } from '../lib/settings';
import type { Flow, Layout } from '../lib/types';
import type { TerminalView } from '../preview/TerminalPreview';
import { createPreviewStore, type PreviewStore } from './preview';
import { createMedia, type Media } from './media';
import { createProjectStore, type ProjectStore } from './project';
import { createScriptStore, initialScriptState, type ScriptStore } from './script';
import { createTauri, type Tauri } from './tauri';
import { createToastStore, type ToastStore } from './toasts';

/** Moves between the seven screens. The router implements it; tests may stand in. */
export interface Navigator {
	go(screen: Screen): Promise<void>;
	current(): Screen;
	subscribe(listener: () => void): () => void;
}

/** The screen's title in the title bar and the sidebar, by screen. */
export const SCREEN_TITLE: Record<Screen, Parameters<typeof t>[0]> = {
	project: 'nav.overview',
	flow: 'nav.flow',
	layouts: 'nav.layouts',
	script: 'nav.scripts',
	preview: 'nav.dryRun',
	tauri: 'nav.tauri',
	media: 'nav.media',
};

/** What the shell drives the app with. */
export interface App {
	/** Runs the action a menu item or shortcut names. Resolves once it is done, true when it succeeded; an error shows as a toast. */
	dispatch(id: ActionId): Promise<boolean>;
	/** Opens the project in a folder, as the Open recent menu does. Resolves true when it opened. */
	openPath(path: string): Promise<boolean>;
	/** Throws away unsaved script edits. */
	discard(): void;
}

export interface StudioOptions {
	api: Api;
	chrome: ChromeStore;
	nav: Navigator;
	/** Settles unsaved script edits before they would be lost: true to go on. Without it they are discarded. */
	settleUnsaved?: () => Promise<boolean>;
	/**
	 * Asks the person in a confirmation window. Without it nothing is confirmed, so a removal that needs
	 * a yes does not happen.
	 */
	confirm?: (spec: DialogSpec) => Promise<DialogResult>;
	/** Opens the full-screen preview in a window of its own (the desktop shell). Without it the preview fills the page. */
	openPreviewWindow?: (
		target: string,
		step: number,
		view: TerminalView,
		scheme: Scheme,
	) => Promise<void>;
}

export interface Studio {
	readonly api: Api;
	readonly chrome: ChromeStore;
	readonly nav: Navigator;
	readonly project: ProjectStore;
	readonly preview: PreviewStore;
	readonly script: ScriptStore;
	readonly toasts: ToastStore;
	/** The project's Tauri import and settings. */
	readonly tauri: Tauri;
	/** The project's media and their plans. */
	readonly media: Media;
	/** The live editor, registered by the Scripts screen while it is on screen. */
	readonly editor: { current: ScriptEditor | null };
	readonly settings: () => Settings;
	/** Runs an action, shows its error as a toast, and resolves true when it succeeded. */
	run(action: () => Promise<unknown>): Promise<boolean>;
	go(screen: Screen): void;
	loadFlow(id: string): Promise<void>;
	saveFlow(flow: Flow, label: string): Promise<void>;
	loadLayout(id: string): Promise<void>;
	saveLayout(layout: Layout, label: string): Promise<void>;
	runPreview(target: string): Promise<void>;
	/** Removes a medium after the person confirms it in a window of its own. True when it was removed. */
	removeMedia(id: string): Promise<boolean>;
	openProject(): Promise<void>;
	openPath(path: string): Promise<void>;
	showChecks(): void;
	loadScript(path: string): Promise<void>;
	selectScript(path: string): Promise<void>;
	saveScript(): Promise<void>;
	testScript(): Promise<void>;
	setDraft(text: string): void;
	discardDraft(): void;
	formatDraft(): void;
	clearScript(): void;
	/** The app a shell drives. */
	app(): App;
	/** Loads the first project and starts following backend changes. */
	start(): Promise<void>;
	dispose(): void;
}

export function createStudio(options: StudioOptions): Studio {
	const { api, chrome, nav } = options;
	const project = createProjectStore();
	const preview = createPreviewStore();
	const script = createScriptStore();
	const toasts = createToastStore();
	const tauri = createTauri(api);
	const media = createMedia(api);
	const editor: Studio['editor'] = { current: null };
	const disposers: (() => void)[] = [];

	const p = () => project.getState();
	const pv = () => preview.getState();
	const sc = () => script.getState();
	const settings = () => chrome.get().settings;

	// ---- chrome sync: the menus and the title follow the stores and the route.
	function syncChrome(): void {
		const route = nav.current();
		const s = p();
		const sx = sc();
		chrome.setMenu({
			hasProject: s.summary !== null,
			canUndo: s.history.can_undo,
			canRedo: s.history.can_redo,
			screen: route,
			scriptDirty: sx.scriptDirty && sx.scriptPath !== '',
		});
		const screen = route === 'project' ? '' : ` · ${t(SCREEN_TITLE[route])}`;
		chrome.setTitle(s.summary ? `${s.summary.name}${screen}` : '');
	}
	disposers.push(
		project.subscribe(syncChrome),
		script.subscribe(syncChrome),
		nav.subscribe(syncChrome),
	);

	async function run(action: () => Promise<unknown>): Promise<boolean> {
		try {
			await action();
			return true;
		} catch (e) {
			toasts.show(errorMessage(e), true, 6000);
			return false;
		}
	}

	// ---- scripts
	async function refreshReport(): Promise<void> {
		const { summary } = p();
		const wanted = sc().scriptTarget;
		const target = summary?.targets.some((x) => x.full_id === wanted) ? wanted : '';
		try {
			const report = await api.testScript(sc().scriptPath, undefined, target || undefined);
			script.setState({ report });
		} catch {
			script.setState({ report: null });
		}
	}

	function replaceDraft(text: string): void {
		script.setState((s) => ({ scriptDraft: text, scriptDirty: text !== s.scriptSource }));
		editor.current?.setText(text);
	}

	const scriptActions = {
		/** Forgets the open script, draft and test run, as a project switch does. */
		clearScript(): void {
			script.setState({ ...initialScriptState });
			editor.current?.setText('', true);
		},
		/** The editor's text changed. */
		setDraft(text: string): void {
			script.setState((s) => ({ scriptDraft: text, scriptDirty: text !== s.scriptSource }));
		},
		async loadScript(path: string): Promise<void> {
			const source = await api.readScript(path);
			const same = path === sc().scriptPath;
			// Our own save comes back as a change event with the text we hold; there is nothing to reload.
			if (same && source === sc().scriptSource) return;
			script.setState((s) => ({
				scriptPath: path,
				scriptSource: source,
				scriptDraft: source,
				scriptDirty: false,
				testRun: same ? s.testRun : null,
				scriptTarget: !s.scriptTarget && pv().previewTarget ? pv().previewTarget : s.scriptTarget,
			}));
			await refreshReport();
		},
		/** Opens another script, asking first when the open one has unsaved edits. */
		async selectScript(path: string): Promise<void> {
			if (path === sc().scriptPath) return;
			if (sc().scriptDirty && !(await guard())) return;
			if (sc().scriptDirty) return;
			await scriptActions.loadScript(path);
		},
		/** Saves the draft, formatted first when Format on save is on. */
		async saveScript(): Promise<void> {
			const path = sc().scriptPath;
			if (!path) return;
			if (settings().formatOnSave) {
				if (editor.current) editor.current.format();
				else script.setState({ scriptDraft: formatLua(sc().scriptDraft) });
			}
			const text = sc().scriptDraft;
			await api.writeScript(path, text, t('history.editScript', { path }));
			script.setState((s) => ({ scriptSource: text, scriptDirty: s.scriptDraft !== text }));
			await refreshReport();
		},
		/** Throws the unsaved edits away. */
		discardDraft(): void {
			replaceDraft(sc().scriptSource);
		},
		/** Formats the draft as an edit that can be undone. */
		formatDraft(): void {
			if (!sc().scriptPath) return;
			const before = sc().scriptDraft;
			if (editor.current) editor.current.format();
			else replaceDraft(formatLua(before));
			toasts.show(
				sc().scriptDraft === before ? t('toast.alreadyFormatted') : t('toast.formatted'),
				false,
				1800,
			);
		},
		/** Runs the draft, saved or not, against the chosen target and shows the result in the console. */
		async testScript(): Promise<void> {
			const path = sc().scriptPath;
			if (!path || sc().testRun?.status === 'running') return;
			const text = sc().scriptDraft;
			const target = sc().scriptTarget;
			const unsaved = text !== sc().scriptSource;
			script.setState({ testRun: { status: 'running', target, text, unsaved } });
			const started = performance.now();
			try {
				const report = await api.testScript(path, text, target || undefined);
				const ms = Math.max(1, Math.round(performance.now() - started));
				script.setState({
					testRun: { status: 'done', target, text, unsaved, at: new Date(), ms, report },
					report,
				});
				const failed =
					(report.tests ?? []).filter((x) => !x.ok).length ||
					report.diagnostics.filter((x) => x.severity === 'error').length;
				if (report.ok)
					toasts.show(
						target ? t('toast.testPassedOn', { target }) : t('toast.testPassed'),
						false,
						3000,
					);
				else toasts.show(tn('toast.testFailed', failed), true, 5000);
			} catch (e) {
				const error = errorMessage(e);
				script.setState({
					testRun: {
						status: 'error',
						target,
						text,
						unsaved,
						at: new Date(),
						ms: Math.round(performance.now() - started),
						error,
					},
				});
				toasts.show(t('toast.testCouldNotRun', { error }), true, 6000);
			}
		},
	};

	/** Settles unsaved edits before they would be lost; without a window to ask in they are discarded. */
	const guard =
		options.settleUnsaved ??
		(async () => {
			scriptActions.discardDraft();
			return true;
		});

	// ---- documents
	async function loadFlow(id: string): Promise<void> {
		const flow = await api.getFlow(id);
		project.setState((s) => ({
			flowId: id,
			flow,
			selectedStep: flow.flow.steps.some((x) => x.id === s.selectedStep) ? s.selectedStep : null,
		}));
	}
	async function loadLayout(id: string): Promise<void> {
		const layout = await api.getLayout(id);
		const stageId = stageIdFor(id);
		const hasStage = stageId !== null && stageId !== id && p().summary?.layouts.includes(stageId);
		const stageLayout = hasStage ? await api.getLayout(stageId) : null;
		project.setState({
			layoutId: id,
			layout,
			stageLayout,
			layoutScope: 'page',
			layoutPath: [],
			overrideRenderer: null,
		});
	}

	async function runPreview(target: string): Promise<void> {
		preview.setState({ previewTarget: target });
		const result = await api.runPreview(target);
		preview.setState({
			preview: result,
			previewStep: Math.max(
				0,
				result.steps.findIndex((x) => !x.skipped),
			),
		});
	}

	/** The target a dry run starts on: the one the Preview setting names when the project has it, else the first. */
	function defaultTarget(ids: string[]): string {
		const wanted = settings().previewTarget;
		return ids.includes(wanted) ? wanted : (ids[0] ?? '');
	}

	/** Forgets everything tied to the project that was open: ids, selections, loaded documents, the preview and the check results. */
	function resetProjectState(): void {
		project.setState({
			flowId: '',
			flow: null,
			selectedStep: null,
			connectFrom: null,
			layoutId: '',
			layout: null,
			stageLayout: null,
			layoutScope: 'page',
			layoutPath: [],
			overrideRenderer: null,
			problems: [],
			showHistory: false,
		});
		scriptActions.clearScript();
		preview.setState({ previewTarget: '', previewStep: 0, preview: null });
		tauri.clear();
		media.clear();
	}

	/** Reloads whatever the current project and screen show. A dirty script is kept. A selection the project no longer has falls back to its first. */
	async function refresh(): Promise<void> {
		const summary = await api.projectSummary();
		project.setState({ summary });
		project.setState({ history: await api.history() });
		project.setState({ problems: await api.checkProject() });
		project.setState({ mcp: await api.mcpInfo() });
		project.setState({ mcpError: await api.mcpError().catch(() => null) });
		// Reads its own failure into the store, so a project with no Tauri state does not fail the refresh.
		await tauri.load();
		// Media are read when the screen is first shown; after that a change from anywhere, an undo included, reads them again.
		await media.refresh();
		if (!summary.targets.some((x) => x.full_id === pv().previewTarget))
			preview.setState({ previewTarget: defaultTarget(summary.targets.map((x) => x.full_id)) });
		const current = await api.getPreview();
		preview.setState({ preview: current });
		if (current) preview.setState({ previewTarget: current.target });
		if (!summary.flows.some((f) => f.id === p().flowId)) {
			project.setState({
				flowId: (summary.flows.find((f) => f.id === 'install') ?? summary.flows[0])?.id ?? '',
				flow: null,
				selectedStep: null,
				connectFrom: null,
			});
		}
		if (p().flowId) await loadFlow(p().flowId);
		if (!summary.layouts.includes(p().layoutId)) {
			project.setState({
				layoutId: summary.layouts[0] ?? '',
				layout: null,
				layoutPath: [],
				overrideRenderer: null,
			});
		}
		if (p().layoutId) await loadLayout(p().layoutId);
		const { scriptPath, scriptDirty } = sc();
		if (scriptPath && !summary.scripts.includes(scriptPath)) {
			// The script is gone from this project. The Scripts screen opens the first one when it is next shown.
			scriptActions.clearScript();
		} else if (scriptPath && !scriptDirty) await scriptActions.loadScript(scriptPath);
	}

	async function openPath(path: string): Promise<void> {
		try {
			const previous = p().summary?.root;
			const opened = await api.openProject(path);
			// What belongs to the old project (selections, loaded documents, the preview) must not leak into the new one.
			if (opened.root !== previous) resetProjectState();
			project.setState({ summary: opened });
		} catch (e) {
			// A project that has moved or gone should not stay in the menu. One that is there but will not
			// load (`invalid_input`) stays, so it can be fixed and opened again.
			if (!isStudioError(e, 'invalid_input')) {
				await chrome.forgetProject(path).catch(() => undefined);
			}
			throw e;
		}
		await refresh();
		await chrome.recordProject(p().summary!.root).catch(() => undefined);
	}

	/** Shows a screen and loads the document it needs first. */
	async function goTo(screen: Screen): Promise<void> {
		project.setState({ connectFrom: null });
		await nav.go(screen);
		const s = p();
		if (screen === 'flow' && !s.flow && s.flowId) await loadFlow(s.flowId);
		if (screen === 'layouts' && !s.layout && s.layoutId) await loadLayout(s.layoutId);
		if (screen === 'tauri' && !tauri.store.getState().view) await tauri.load();
		if (screen === 'media' && !media.store.getState().loaded) await media.load();
		if (screen === 'script' && !sc().scriptPath && s.summary?.scripts[0])
			await scriptActions.loadScript(s.summary.scripts[0]);
	}

	function go(screen: Screen): void {
		void run(() => goTo(screen));
	}

	// ---- actions behind the menu
	const actions: Partial<Record<ActionId, () => Promise<void>>> = {
		'file.open': () => studio.openProject(),
		'file.reload': async () => {
			const summary = p().summary;
			if (!summary) return;
			resetProjectState();
			project.setState({ summary: await api.openProject(summary.root) });
			await refresh();
			toasts.show(t('toast.reloaded'), false, 2500);
		},
		'file.save': async () => {
			if (sc().scriptPath && sc().scriptDirty) await scriptActions.saveScript();
		},
		'edit.undo': async () => void (await api.undo()),
		'edit.redo': async () => void (await api.redo()),
		'project.check': async () => {
			const problems = await api.checkProject();
			project.setState({ problems });
			const n = problems.length;
			const errors = problems.filter((x) => x.severity === 'error').length;
			toasts.show(
				n === 0
					? t('toast.checkPassed')
					: errors
						? t('toast.checkFoundWithErrors', { found: tn('toast.problems', n), errors })
						: t('toast.checkFound', { found: tn('toast.problems', n) }),
				errors > 0,
				4000,
			);
		},
		'view.fullScreenPreview': async () => {
			if (!pv().preview) await runPreview(pv().previewTarget);
			if (options.openPreviewWindow)
				await options.openPreviewWindow(
					pv().previewTarget,
					pv().previewStep,
					pv().previewView,
					pv().previewScheme,
				);
			else preview.setState({ fullscreen: true });
		},
		'project.preview': async () => {
			await runPreview(pv().previewTarget);
			await nav.go('preview');
		},
	};
	for (const [route, id] of Object.entries(SCREEN_ACTIONS) as [Screen, ActionId][])
		actions[id] = () => goTo(route);

	// ---- the backend's change events and the settings that change what is on screen
	let autoRunTimer: ReturnType<typeof setTimeout> | undefined;
	function scheduleAutoRun(): void {
		clearTimeout(autoRunTimer);
		autoRunTimer = setTimeout(() => {
			if (!p().summary || !pv().previewTarget || !settings().previewAutoRun) return;
			void run(() => runPreview(pv().previewTarget));
		}, 250);
	}

	async function start(): Promise<void> {
		const off = await api.onChange((event) => {
			// Keep the layout selection when the change came from elsewhere.
			const keepPath = p().layoutPath;
			const keepScope = p().layoutScope;
			const keepOverride = p().overrideRenderer;
			void run(async () => {
				await refresh();
				if (event.doc?.type === 'layout')
					project.setState({
						layoutPath: keepPath,
						layoutScope: keepScope,
						overrideRenderer: keepOverride,
					});
				if (event.source === 'mcp')
					toasts.show(t('toast.agent', { label: event.label }), false, 4000);
				if (
					settings().previewAutoRun &&
					(event.doc?.type === 'flow' || event.doc?.type === 'layout')
				)
					scheduleAutoRun();
			});
		});
		disposers.push(off);

		let seen = settings();
		disposers.push(
			chrome.subscribe(() => {
				const next = settings();
				const was = seen;
				seen = next;
				if (next.mcpEnabled !== was.mcpEnabled || next.mcpPort !== was.mcpPort) {
					// Only the card changes, so text typed in the script editor stays as it is.
					void Promise.all([api.mcpInfo(), api.mcpError().catch(() => null)]).then(
						([mcp, mcpError]) => project.setState({ mcp, mcpError }),
						() => undefined,
					);
				}
				const summary = p().summary;
				if (next.previewTarget !== was.previewTarget && summary) {
					const wanted = defaultTarget(summary.targets.map((x) => x.full_id));
					if (wanted !== pv().previewTarget) preview.setState({ previewTarget: wanted });
				}
			}),
		);

		syncChrome();
		await run(async () => {
			try {
				project.setState({ summary: await api.projectSummary() });
			} catch {
				return;
			}
			await refresh();
		});
		const summary = p().summary;
		if (summary) {
			// A project given on the command line counts as a recent one.
			await chrome.recordProject(summary.root).catch(() => undefined);
		} else {
			const last = settings().lastProject;
			if (settings().reopenLastProject && last) await run(() => openPath(last));
		}
	}

	const studio: Studio = {
		api,
		chrome,
		nav,
		project,
		preview,
		script,
		toasts,
		tauri,
		media,
		editor,
		settings,
		run,
		go,
		loadFlow,
		saveFlow: async (flow, label) => {
			project.setState({ flow: await api.setFlow(flow.id, flow, label) });
		},
		loadLayout,
		saveLayout: async (layout, label) => {
			const saved = await api.setLayout(layout.id, layout, label);
			// A stage is saved from the page that shows it; the open layout stays the page.
			if (layout.id !== p().layoutId && isStageId(layout.id))
				project.setState({ stageLayout: saved });
			else project.setState({ layout: saved });
		},
		runPreview,
		removeMedia: async (id) => {
			const found = media.store.getState().items.find((m) => m.id === id);
			if (!found || !options.confirm) return false;
			const answer = await options.confirm({
				kind: 'confirm',
				title: t('media.remove.title'),
				message: t('media.remove.message', { label: found.label, id }),
				detail: t('media.remove.detail'),
				buttons: [
					{ id: 'remove', label: t('media.remove.confirm'), danger: true },
					{ id: 'cancel', label: t('common.cancel') },
				],
			});
			return answer === 'remove' && (await media.remove(id));
		},
		openProject: async () => {
			const path = await api.pickProject(settings().projectsFolder || undefined);
			if (path) await openPath(path);
		},
		openPath,
		showChecks: () => {
			project.setState((s) => ({ showHistory: false, checksRequest: s.checksRequest + 1 }));
			void nav.go('project');
		},
		...scriptActions,
		app: () => ({
			dispatch(id) {
				const action = actions[id];
				return action ? run(action) : Promise.resolve(false);
			},
			openPath: (path) => run(() => openPath(path)),
			discard: () => scriptActions.discardDraft(),
		}),
		start,
		dispose() {
			clearTimeout(autoRunTimer);
			for (const d of disposers.splice(0)) d();
		},
	};
	return studio;
}
