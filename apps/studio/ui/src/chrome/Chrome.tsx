// The shell around the app: the shared window chrome in the desktop window, a plain host in the browser.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { StrictMode, useCallback, useMemo, useSyncExternalStore } from 'react';
import { createRoot } from 'react-dom/client';
import type { SelectableMenuItem } from '@liminal-hq/studio-chrome/ContextMenu/types';
import { Dialog } from '@liminal-hq/studio-chrome/Dialog/Dialog';
import { MenuBar } from '@liminal-hq/studio-chrome/MenuBar/MenuBar';
import { TitleBar } from '@liminal-hq/studio-chrome/TitleBar';
import { AppMenuButton } from '@liminal-hq/studio-chrome/TitleBar/AppMenuButton';
import { TitleBarTitle } from '@liminal-hq/studio-chrome/TitleBar/TitleBarTitle';
import type { WindowControls } from '@liminal-hq/studio-chrome/TitleBar/windowControls';
import { WindowChromeProvider } from '@liminal-hq/studio-chrome/WindowChromeProvider/WindowChromeProvider';
import { WindowFrame } from '@liminal-hq/studio-chrome/WindowFrame';
import type { ChromeStore } from './chromeStore';
import { APP_NAME, DialogButtons, DialogMessage, Mark } from './DialogBodies';
import { MNEMONICS, buildMenus, isActionId, type ActionId } from './menuModel';
import { PreferencesScreen } from './PreferencesScreen';

interface ShellProps {
	store: ChromeStore;
	onAction(id: ActionId): void;
	/** Called once with the element the app draws into. */
	appHost(element: HTMLElement | null): void;
}

/** A dialog drawn in the page. Only the browser demo uses it: the desktop opens windows. */
function InPageDialogView({ store }: { store: ChromeStore }) {
	const { dialog } = useSyncExternalStore(store.subscribe, store.get);
	if (!dialog) return null;
	const { spec, done } = dialog;
	return (
		<Dialog
			open
			onClose={() => done(null)}
			title={spec.title}
			size={spec.kind === 'preferences' ? 'large' : 'small'}
			{...(spec.kind === 'preferences' ? {} : { footer: <DialogButtons spec={spec} onAnswer={done} /> })}
		>
			{spec.kind === 'preferences' ? <div className="prefs-inline"><PreferencesScreen store={store} /></div> : <DialogMessage spec={spec} />}
		</Dialog>
	);
}

function DesktopShell({ store, controls, onAction, appHost }: ShellProps & { controls: WindowControls }) {
	const state = useSyncExternalStore(store.subscribe, store.get);
	const menus = useMemo(() => buildMenus(state.menu), [state.menu]);
	const choose = useCallback(
		(item: SelectableMenuItem) => {
			if (isActionId(item.id)) onAction(item.id);
		},
		[onAction],
	);
	const bar = state.menu.menuBar;
	return (
		<WindowChromeProvider controls={controls}>
			<WindowFrame className="chrome-surface">
				<TitleBar
					start={
						<span className="wordmark">
							{/* With the bar on, it carries the menus, F10 and Alt, and the wordmark opens nothing. */}
							<AppMenuButton label={APP_NAME} mark={<Mark />} items={menus.flat} onSelect={choose} interactive={!bar} />
						</span>
					}
					center={<TitleBarTitle>{state.title || APP_NAME}</TitleBarTitle>}
				/>
				{bar ? <MenuBar items={menus.bar} onSelect={choose} label="Menu bar" moreLabel="More" mnemonics={MNEMONICS} /> : null}
				<div id="app" className="app-host" ref={appHost} />
				{/* Empty on the desktop, where dialogs are windows; the browser demo draws them here. */}
				<InPageDialogView store={store} />
			</WindowFrame>
		</WindowChromeProvider>
	);
}

function PlainShell({ store, appHost }: ShellProps) {
	return (
		<>
			<div id="app" className="app-host plain" ref={appHost} />
			<InPageDialogView store={store} />
		</>
	);
}

/**
 * Draws the shell into `container` and resolves with the element the app should draw into. Pass
 * `controls` for the desktop chrome; without them the page keeps its own header and only the dialogs
 * come from here.
 */
export function mountShell(container: HTMLElement, props: Omit<ShellProps, 'appHost'> & { controls?: WindowControls }): Promise<HTMLElement> {
	return new Promise((resolve) => {
		let resolved = false;
		const appHost = (element: HTMLElement | null) => {
			if (element && !resolved) {
				resolved = true;
				resolve(element);
			}
		};
		const { controls, ...rest } = props;
		createRoot(container).render(<StrictMode>{controls ? <DesktopShell {...rest} controls={controls} appHost={appHost} /> : <PlainShell {...rest} appHost={appHost} />}</StrictMode>);
	});
}
