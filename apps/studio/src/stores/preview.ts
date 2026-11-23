// The preview store: the dry-run target, its result and the step on show.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { Preview } from '../lib/types';
import type { Scheme } from '@liminal-hq/installer-renderer';
import type { TerminalView } from '../preview/TerminalPreview';

export interface PreviewState {
	previewTarget: string;
	preview: Preview | null;
	previewStep: number;
	/** How a `tui` target is drawn: the Studio's own drawing or the renderer's real terminal. */
	previewView: TerminalView;
	/** Light, dark or the system's colours, for a look that has both (Windows 11). */
	previewScheme: Scheme;
	/** The in-page full-viewport preview, for a browser tab that has no windows to open. */
	fullscreen: boolean;
}

export type PreviewStore = StoreApi<PreviewState>;

export const createPreviewStore = (): PreviewStore =>
	createStore<PreviewState>(() => ({
		previewTarget: '',
		preview: null,
		previewStep: 0,
		previewView: 'rich',
		previewScheme: 'system',
		fullscreen: false,
	}));
