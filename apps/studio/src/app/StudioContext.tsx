// Gives screens the Studio and hooks to read its stores.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useSyncExternalStore } from 'react';
import { useStore } from 'zustand';
import type { ChromeState } from '../chrome/chromeStore';
import type { PreviewState } from '../stores/preview';
import type { ProjectState } from '../stores/project';
import type { ScriptState } from '../stores/script';
import type { Studio } from '../stores/studio';
import type { ToastState } from '../stores/toasts';

const StudioContext = createContext<Studio | null>(null);

export const StudioProvider = StudioContext.Provider;

export function useStudio(): Studio {
	const studio = useContext(StudioContext);
	if (!studio) throw new Error('useStudio needs a StudioProvider');
	return studio;
}

export function useProject<T>(select: (state: ProjectState) => T): T {
	return useStore(useStudio().project, select);
}

export function usePreview<T>(select: (state: PreviewState) => T): T {
	return useStore(useStudio().preview, select);
}

export function useScript<T>(select: (state: ScriptState) => T): T {
	return useStore(useStudio().script, select);
}

export function useToast<T>(select: (state: ToastState) => T): T {
	return useStore(useStudio().toasts, select);
}

/** The window chrome's state: settings, menu and title. */
export function useChromeState(): ChromeState {
	const { chrome } = useStudio();
	return useSyncExternalStore(chrome.subscribe, chrome.get);
}
