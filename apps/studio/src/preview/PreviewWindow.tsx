// The full-screen preview as a child window: its own copy of the dry run, opened at the screen's natural size and able to go truly full screen.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { StrictMode, useCallback, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import type { ButtonLayout } from '@liminal-hq/studio-chrome/TitleBar/buttonLayout';
import { TitleBar } from '@liminal-hq/studio-chrome/TitleBar';
import { TitleBarTitle } from '@liminal-hq/studio-chrome/TitleBar/TitleBarTitle';
import type { WindowControls } from '@liminal-hq/studio-chrome/TitleBar/windowControls';
import { WindowChromeProvider } from '@liminal-hq/studio-chrome/WindowChromeProvider/WindowChromeProvider';
import { WindowFrame } from '@liminal-hq/studio-chrome/WindowFrame';
import { chromeLabels } from '../i18n/chromeLabels';
import { t } from '../i18n/messages';
import chrome from '../chrome/Chrome.module.css';
import { createApi, errorMessage, type Api } from '../lib/api';
import type { Preview, ProjectSummary } from '../lib/types';
import { FullScreenBody } from './FullScreenBody';
import { fitWindow, parsePreviewQuery } from './fullScreen';
import { lookFor, type Scheme } from '@liminal-hq/installer-renderer';
import { stepFrom } from './sim';
import type { TerminalView } from './TerminalPreview';
import { useRun } from './useRun';

const closeOnly: ButtonLayout = { start: [], end: ['close'] };

/** Fits the window to the screen it opens on, centred. A failure leaves the size the shell gave it. */
async function sizeToScreen(look: ReturnType<typeof lookFor>): Promise<void> {
	try {
		const { getCurrentWindow, LogicalSize } = await import('@tauri-apps/api/window');
		const fit = fitWindow(look, {
			width: window.screen.availWidth,
			height: window.screen.availHeight,
		});
		const win = getCurrentWindow();
		await win.setSize(new LogicalSize(fit.width, fit.height));
		await win.center();
	} catch {
		// The shell's default size is fine.
	}
}

function PreviewWindow({ search, controls }: { search: string; controls: WindowControls }) {
	const start = parsePreviewQuery(search);
	const [api, setApi] = useState<Api | null>(null);
	const [summary, setSummary] = useState<ProjectSummary | null>(null);
	const [target, setTarget] = useState(start?.target ?? '');
	const [pv, setPv] = useState<Preview | null>(null);
	const [index, setIndex] = useState(start?.step ?? 0);
	const [view, setView] = useState<TerminalView>(start?.view ?? 'rich');
	const [scheme, setScheme] = useState<Scheme>(start?.scheme ?? 'system');
	const [fullscreen, setFullscreen] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const run = useRun(pv, summary?.components ?? [], index, setIndex);

	useEffect(() => {
		void (async () => {
			try {
				const a = await createApi();
				setApi(a);
				setSummary(await a.projectSummary());
			} catch (e) {
				setError(errorMessage(e));
			}
		})();
	}, []);

	const load = useCallback(
		async (to: string, keepStep: boolean) => {
			if (!api || !to) return;
			try {
				const result = await api.runPreview(to);
				setPv(result);
				setIndex((i) => {
					const at = keepStep ? i : 0;
					return result.steps[at] && !result.steps[at].skipped ? at : stepFrom(result.steps, at, 1);
				});
			} catch (e) {
				setError(errorMessage(e));
			}
		},
		[api],
	);
	useEffect(() => {
		void load(target, true);
	}, [load, target]);

	// Open at the look's natural size once the target's look is known.
	const renderer = pv?.renderer;
	const sized = renderer !== undefined;
	useEffect(() => {
		if (renderer) void sizeToScreen(lookFor(renderer, pv?.theme));
	}, [sized]); // eslint-disable-line react-hooks/exhaustive-deps

	const toggleFullscreen = useCallback(async () => {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		const win = getCurrentWindow();
		const next = !(await win.isFullscreen());
		await win.setFullscreen(next);
		setFullscreen(next);
	}, []);

	// Esc leaves full screen first and closes the window the second time; F11 toggles. A key the
	// terminal view took (Esc among them) is marked handled, so it never reaches this.
	useEffect(() => {
		const onKey = (e: KeyboardEvent) => {
			if (e.defaultPrevented) return;
			const tag = (e.target as HTMLElement | null)?.tagName;
			if (e.key === 'F11') {
				e.preventDefault();
				void toggleFullscreen();
			} else if (e.key === 'Escape' && tag !== 'SELECT') {
				e.preventDefault();
				if (fullscreen) void toggleFullscreen();
				else void controls.close();
			}
		};
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	}, [fullscreen, toggleFullscreen, controls]);

	return (
		<WindowChromeProvider controls={controls}>
			<WindowFrame className={chrome.chromeSurface}>
				{fullscreen ? null : (
					<TitleBar
						labels={chromeLabels()}
						buttonLayout={closeOnly}
						titleAlign="start"
						center={<TitleBarTitle>{t('dialog.fullScreenPreview')}</TitleBarTitle>}
					/>
				)}
				{error ? (
					<p role="alert">{error}</p>
				) : (
					<FullScreenBody
						pv={pv}
						run={run}
						components={summary?.components ?? []}
						targets={summary?.targets ?? []}
						target={target}
						index={index}
						setIndex={setIndex}
						onTarget={(next) => {
							setPv(null);
							setTarget(next);
						}}
						view={view}
						onView={setView}
						scheme={scheme}
						onScheme={setScheme}
						api={api}
						fullscreen={fullscreen}
						onToggleFullscreen={() => void toggleFullscreen()}
						loading={!pv}
					/>
				)}
			</WindowFrame>
		</WindowChromeProvider>
	);
}

/** Draws the preview window when the page was opened as one; false when the query does not describe it. */
export function mountPreviewWindow(
	container: HTMLElement,
	search: string,
	controls: WindowControls,
): boolean {
	if (!parsePreviewQuery(search)) return false;
	createRoot(container).render(
		<StrictMode>
			<PreviewWindow search={search} controls={controls} />
		</StrictMode>,
	);
	return true;
}
