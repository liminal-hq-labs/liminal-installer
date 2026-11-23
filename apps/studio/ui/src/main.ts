// Entry point: picks the window this page is (the app, or one of its child windows), then wires menus, keys and dialogs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import './styles/tokens.css';
import './styles/app.css';
import { startApp, type App } from './app';
import { mountShell } from './chrome/Chrome';
import { createChromeStore, type ChromeStore } from './chrome/chromeStore';
import { createDialogs, inPageDialogBackend, tauriDialogBackend, type DialogBackend } from './chrome/dialogWindow';
import { mountInfoWindow } from './chrome/InfoWindow';
import { actionForEvent, type ActionId } from './chrome/menuModel';
import { createWindowControls, inTauri } from './chrome/windowControls';
import { browserSettings, tauriSettings, type SettingsPort } from './lib/settings';

const storage = () => {
	try {
		return window.localStorage;
	} catch {
		return undefined;
	}
};

/** Every window follows the theme setting. */
function followTheme(store: ChromeStore): void {
	const apply = () => {
		document.documentElement.dataset.theme = store.get().settings.theme;
	};
	store.subscribe(apply);
	apply();
}

/**
 * The desktop window draws the shared chrome. A browser tab keeps the page's own header, and
 * `?chrome` in a development server draws the chrome there too, for looking at it.
 */
const wantsChrome = () => inTauri() || (import.meta.env.DEV && new URLSearchParams(location.search).has('chrome'));

async function boot(): Promise<void> {
	const container = document.getElementById('root');
	if (!container) return;
	document.documentElement.dataset.shell = inTauri() ? 'tauri' : 'browser';
	const port: SettingsPort = inTauri() ? await tauriSettings() : browserSettings(storage());
	const store = createChromeStore(port);
	await store.init();
	followTheme(store);
	const controls = createWindowControls();

	// A child window is this same page with `?window=<kind>`.
	if (new URLSearchParams(location.search).has('window')) {
		if (mountInfoWindow(container, location.search, store, controls)) return;
	}

	const chrome = wantsChrome();
	let app: App | undefined;
	const backend: DialogBackend = inTauri() ? await tauriDialogBackend() : inPageDialogBackend((dialog) => store.setDialog(dialog));
	const dialogs = createDialogs(backend);

	/** Asks what to do with unsaved script edits. True when it is fine to go on. */
	const settleUnsaved = async (): Promise<boolean> => {
		if (!store.get().menu.scriptDirty) return true;
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
		if (answer === 'save') app?.dispatch('file.save');
		return answer === 'save' || answer === 'discard';
	};

	/** The chrome owns the actions about itself; the app runs the rest. */
	const perform = (id: ActionId): void => {
		void (async () => {
			if (id === 'view.menuBar') await store.toggleMenuBar();
			else if (id === 'help.about') await dialogs.open({ kind: 'about', title: 'About Liminal Installer Studio', buttons: [{ id: 'close', label: 'Close', primary: true }] }).result;
			else if (id === 'app.preferences') await dialogs.open({ kind: 'preferences', title: 'Preferences' }).result;
			else if (id === 'file.quit') {
				// In the desktop window the close request asks about unsaved edits; a tab has no such request.
				if (inTauri() || (await settleUnsaved())) await controls.close();
			} else if (id === 'file.open' || id === 'file.reload') {
				if (await settleUnsaved()) app?.dispatch(id);
			} else app?.dispatch(id);
		})().catch(() => undefined);
	};

	const appHost = await mountShell(container, { store, onAction: perform, ...(chrome ? { controls } : {}) });
	app = await startApp(appHost, store, { header: !chrome });

	if (inTauri()) {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		await getCurrentWindow().onCloseRequested(async (event) => {
			if (!store.get().menu.scriptDirty) return;
			event.preventDefault();
			if (await settleUnsaved()) await getCurrentWindow().destroy();
		});
	}

	window.addEventListener('keydown', (e) => {
		if (e.defaultPrevented || store.get().dialog) return;
		const target = e.target as HTMLElement | null;
		const inTextField = !!target && (target.tagName === 'TEXTAREA' || target.tagName === 'INPUT' || target.tagName === 'SELECT');
		const id = actionForEvent(e, store.get().menu, inTextField);
		if (!id) return;
		e.preventDefault();
		perform(id);
	});
}

void boot();
