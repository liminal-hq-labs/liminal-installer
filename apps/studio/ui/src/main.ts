// Entry point: picks the window this page is (the app, or one of its child windows), then wires menus, keys and dialogs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import './styles/tokens.css';
import '@fontsource/vt323/latin-400.css';
import './styles/app.css';
import './styles/preview.css';
import { startApp, type App } from './app';
import { mountShell } from './chrome/Chrome';
import { createChromeStore, type ChromeStore } from './chrome/chromeStore';
import {
	CREATED_PREFIX,
	createDialogs,
	inPageDialogBackend,
	openChildWindow,
	tauriDialogBackend,
	type DialogBackend,
} from './chrome/dialogWindow';
import { mountInfoWindow } from './chrome/InfoWindow';
import { actionForEvent, isActionId, recentPath } from './chrome/menuModel';
import { createWindowControls, inTauri } from './chrome/windowControls';
import { applyAppearance, defaultSystem, followSystem } from './lib/appearance';
import { createHost } from './lib/host';
import { browserSettings, tauriSettings, type SettingsPort } from './lib/settings';

const storage = () => {
	try {
		return window.localStorage;
	} catch {
		return undefined;
	}
};

/** Every window follows the appearance settings: theme (or the system's), density, text size and motion. */
async function followAppearance(store: ChromeStore): Promise<void> {
	let system = defaultSystem;
	const apply = () => applyAppearance(document.documentElement, store.get().settings, system);
	store.subscribe(apply);
	apply();
	await followSystem((next) => {
		system = next;
		apply();
	}, inTauri());
}

/**
 * The desktop window draws the shared chrome. A browser tab keeps the page's own header, and
 * `?chrome` in a development server draws the chrome there too, for looking at it.
 */
const wantsChrome = () =>
	inTauri() || (import.meta.env.DEV && new URLSearchParams(location.search).has('chrome'));

async function boot(): Promise<void> {
	const container = document.getElementById('root');
	if (!container) return;
	document.documentElement.dataset.shell = inTauri() ? 'tauri' : 'browser';
	const port: SettingsPort = inTauri() ? await tauriSettings() : browserSettings(storage());
	const store = createChromeStore(port);
	await store.init();
	await followAppearance(store);
	const controls = createWindowControls();
	const host = await createHost();

	// A child window is this same page with `?window=<kind>`.
	if (new URLSearchParams(location.search).has('window')) {
		if (mountInfoWindow(container, location.search, store, host, controls)) return;
	}

	const chrome = wantsChrome();
	let app: App | undefined;
	const backend: DialogBackend = inTauri()
		? await tauriDialogBackend()
		: inPageDialogBackend((dialog) => store.setDialog(dialog));
	const dialogs = createDialogs(backend);

	/** Asks what to do with unsaved script edits. True when it is fine to go on. */
	const settleUnsaved = async (): Promise<boolean> => {
		if (!store.get().menu.scriptDirty) return true;
		// With the confirmation off, unsaved edits are discarded without asking.
		if (!store.get().settings.confirmDiscard) {
			app?.discard();
			return true;
		}
		const answer = await dialogs.open({
			kind: 'confirm',
			title: 'Unsaved changes',
			message: 'The script has changes that are not saved.',
			detail: 'Save them first, or discard them and carry on.',
			buttons: [
				{ id: 'save', label: 'Save', primary: true },
				{ id: 'discard', label: 'Discard changes', danger: true },
				{ id: 'cancel', label: 'Cancel' },
			],
		}).result;
		// Going on after a save that failed would lose the edits, so the save settles it.
		if (answer === 'save') return (await app?.dispatch('file.save')) === true;
		if (answer === 'discard') app?.discard();
		return answer === 'discard';
	};

	/** Its own window in the desktop shell, which does not hold up other dialogs; a dialog in the page elsewhere. */
	const openInfoWindow = async (
		kind: 'preferences' | 'shortcuts' | 'agent',
		title: string,
	): Promise<void> => {
		if (inTauri()) await openChildWindow(kind);
		else await dialogs.open({ kind, title }).result;
	};

	/** The chrome owns the actions about itself; the app runs the rest. */
	const perform = (id: string): void => {
		void (async () => {
			const recent = recentPath(id, store.get().menu);
			if (recent !== undefined) {
				if (await settleUnsaved()) await app?.openPath(recent);
			} else if (!isActionId(id)) return;
			else if (id === 'view.menuBar') await store.toggleMenuBar();
			else if (id === 'file.clearRecent') await store.updateSettings({ recentProjects: [] });
			else if (id === 'help.about')
				await dialogs.open({
					kind: 'about',
					title: 'About Liminal Installer Studio',
					buttons: [{ id: 'close', label: 'Close', primary: true }],
				}).result;
			else if (id === 'app.preferences') await openInfoWindow('preferences', 'Preferences');
			else if (id === 'help.shortcuts') await openInfoWindow('shortcuts', 'Keyboard Shortcuts');
			else if (id === 'help.agent') await openInfoWindow('agent', 'Connect an Agent');
			else if (id === 'file.new') {
				if (!(await settleUnsaved())) return;
				const answer = await dialogs.open({ kind: 'new-project', title: 'New Project' }).result;
				if (answer?.startsWith(CREATED_PREFIX))
					await app?.openPath(answer.slice(CREATED_PREFIX.length));
			} else if (id === 'file.quit') {
				// In the desktop window the close request asks about unsaved edits; a tab has no such request.
				if (inTauri() || (await settleUnsaved())) await controls.close();
			} else if (id === 'file.open' || id === 'file.reload') {
				if (await settleUnsaved()) await app?.dispatch(id);
			} else await app?.dispatch(id);
		})().catch(() => undefined);
	};

	const appHost = await mountShell(container, {
		store,
		host,
		onAction: perform,
		...(chrome ? { controls } : {}),
	});
	app = await startApp(appHost, store, { header: !chrome, settleUnsaved });

	if (inTauri()) {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		await getCurrentWindow().onCloseRequested(async (event) => {
			if (!store.get().menu.scriptDirty || !store.get().settings.confirmDiscard) return;
			event.preventDefault();
			if (await settleUnsaved()) await getCurrentWindow().destroy();
		});
	}

	window.addEventListener('keydown', (e) => {
		if (e.defaultPrevented || store.get().dialog) return;
		const target = e.target as HTMLElement | null;
		const inTextField =
			!!target &&
			(target.tagName === 'TEXTAREA' ||
				target.tagName === 'INPUT' ||
				target.tagName === 'SELECT' ||
				!!target.closest?.('.cm-editor'));
		const id = actionForEvent(e, store.get().menu, inTextField);
		if (!id) return;
		e.preventDefault();
		perform(id);
	});
}

void boot();
