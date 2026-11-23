// The toast store: one message shown for a moment, replaced by the next.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';

export interface Toast {
	text: string;
	error: boolean;
}

export interface ToastState {
	toast: Toast | null;
}

export interface ToastStore extends StoreApi<ToastState> {
	/** Shows a message for `ms` milliseconds. */
	show(text: string, error: boolean, ms?: number): void;
	dismiss(): void;
}

export function createToastStore(): ToastStore {
	const store = createStore<ToastState>(() => ({ toast: null }));
	let timer: ReturnType<typeof setTimeout> | undefined;
	return Object.assign(store, {
		show(text: string, error: boolean, ms = 3000) {
			store.setState({ toast: { text, error } });
			clearTimeout(timer);
			timer = setTimeout(() => store.setState({ toast: null }), ms);
		},
		dismiss() {
			clearTimeout(timer);
			store.setState({ toast: null });
		},
	});
}
