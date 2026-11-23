// A child window of the Studio: About, Preferences or a confirmation, under the shared title bar.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { StrictMode, useEffect } from 'react';
import { createRoot } from 'react-dom/client';
import type { ButtonLayout } from '@liminal-hq/studio-chrome/TitleBar/buttonLayout';
import { TitleBar } from '@liminal-hq/studio-chrome/TitleBar';
import { TitleBarTitle } from '@liminal-hq/studio-chrome/TitleBar/TitleBarTitle';
import type { WindowControls } from '@liminal-hq/studio-chrome/TitleBar/windowControls';
import { WindowChromeProvider } from '@liminal-hq/studio-chrome/WindowChromeProvider/WindowChromeProvider';
import { WindowFrame } from '@liminal-hq/studio-chrome/WindowFrame';
import type { ChromeStore } from './chromeStore';
import { DialogButtons, DialogMessage } from './DialogBodies';
import { DIALOG_RESULT_EVENT, decodeSpec, type DialogAnswer, type DialogResult, type DialogSpec } from './dialogWindow';
import type { Host } from '../lib/host';
import { PreferencesScreen } from '../preferences/PreferencesScreen';
import { inTauri } from './windowControls';

/** Only the Close button, on the end side. */
const closeOnly: ButtonLayout = { start: [], end: ['close'] };

interface InfoWindowProps {
	spec: DialogSpec;
	/** Ties the answer to the window that asked. */
	id: string;
	store: ChromeStore;
	host: Host;
	controls: WindowControls;
}

/** Tells the window that opened this one what was chosen, then closes. */
async function answer(id: string, result: DialogResult, controls: WindowControls): Promise<void> {
	if (inTauri()) {
		const { emit } = await import('@tauri-apps/api/event');
		const payload: DialogAnswer = { id, result };
		await emit(DIALOG_RESULT_EVENT, payload);
	}
	await controls.close();
}

function InfoWindow({ spec, id, store, host, controls }: InfoWindowProps) {
	useEffect(() => {
		const onKey = (e: KeyboardEvent) => {
			if (e.key === 'Escape' && !e.defaultPrevented) void answer(id, null, controls);
		};
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	}, [id, controls]);
	// Closing the window any other way is an answer of "none", so the window that asked is not left waiting.
	useEffect(() => {
		if (!inTauri()) return;
		let stop: (() => void) | undefined;
		void import('@tauri-apps/api/window').then(async ({ getCurrentWindow }) => {
			stop = await getCurrentWindow().onCloseRequested(async () => {
				const { emit } = await import('@tauri-apps/api/event');
				const payload: DialogAnswer = { id, result: null };
				await emit(DIALOG_RESULT_EVENT, payload);
			});
		});
		return () => stop?.();
	}, [id]);

	return (
		<WindowChromeProvider controls={controls}>
			<WindowFrame className="chrome-surface">
				<TitleBar buttonLayout={closeOnly} titleAlign="start" center={<TitleBarTitle>{spec.title}</TitleBarTitle>} />
				{spec.kind === 'preferences' ? (
					<div className="app-host prefs-host">
						<PreferencesScreen port={store.port} host={host} />
					</div>
				) : (
					<div className="dlg-body">
						<DialogMessage spec={spec} />
						<DialogButtons spec={spec} onAnswer={(r) => void answer(id, r, controls)} />
					</div>
				)}
			</WindowFrame>
		</WindowChromeProvider>
	);
}

/** Draws the child window the page was opened as, or does nothing when the query does not describe one. */
export function mountInfoWindow(container: HTMLElement, search: string, store: ChromeStore, host: Host, controls: WindowControls): boolean {
	const params = new URLSearchParams(search);
	const kind = params.get('window');
	const spec = decodeSpec(search) ?? (kind === 'preferences' ? ({ kind: 'preferences', title: 'Preferences' } as DialogSpec) : undefined);
	if (!spec) return false;
	createRoot(container).render(
		<StrictMode>
			<InfoWindow spec={spec} id={params.get('id') ?? ''} store={store} host={host} controls={controls} />
		</StrictMode>,
	);
	return true;
}
