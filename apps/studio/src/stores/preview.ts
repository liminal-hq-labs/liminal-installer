// The preview store: the dry-run target, its result and the step on show.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { Preview } from '../lib/types';

export interface PreviewState {
	previewTarget: string;
	preview: Preview | null;
	previewStep: number;
}

export type PreviewStore = StoreApi<PreviewState>;

export const createPreviewStore = (): PreviewStore =>
	createStore<PreviewState>(() => ({ previewTarget: '', preview: null, previewStep: 0 }));
