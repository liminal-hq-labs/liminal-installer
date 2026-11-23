// Dialogs as windows: open one, wait for its answer, close it. The browser demo draws them in the page instead.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { previewWindowQuery } from '../preview/fullScreen';

export interface DialogButton {
	id: string;
	label: string;
	primary?: boolean;
	danger?: boolean;
}

export type DialogKind =
	'about' | 'confirm' | 'preferences' | 'shortcuts' | 'agent' | 'new-project';

export const DIALOG_KINDS: DialogKind[] = [
	'about',
	'confirm',
	'preferences',
	'shortcuts',
	'agent',
	'new-project',
];

/** The answer of New project when it made a project: `created:` and the project's folder. */
export const CREATED_PREFIX = 'created:';

export interface DialogSpec {
	kind: DialogKind;
	title: string;
	message?: string;
	detail?: string;
	/** The buttons of a confirmation, in order. The last one is the safe choice. */
	buttons?: DialogButton[];
}

/** The id of the button chosen, or null when the dialog was closed without choosing. */
export type DialogResult = string | null;

/**
 * What shows a dialog. `show` starts it and calls `done` once with the answer; the function it
 * returns closes the dialog from outside.
 */
export interface DialogBackend {
	show(spec: DialogSpec, done: (result: DialogResult) => void): () => void;
}

export interface DialogHandle {
	/** Settles once, when the dialog is answered or closed. Never rejects. */
	result: Promise<DialogResult>;
	/** Closes the dialog, or drops it from the queue; the result is then null. */
	close(): void;
}

/** One dialog at a time: a second `open` waits for the first to finish. */
export function createDialogs(backend: DialogBackend) {
	let tail: Promise<unknown> = Promise.resolve();
	return {
		open(spec: DialogSpec): DialogHandle {
			let resolveResult!: (result: DialogResult) => void;
			const result = new Promise<DialogResult>((resolve) => {
				resolveResult = resolve;
			});
			let closed = false;
			let over = false;
			const settle = (value: DialogResult) => {
				over = true;
				resolveResult(value);
			};
			let hide: (() => void) | undefined;
			const finished = result.then(() => undefined);
			tail = tail.then(() => {
				if (closed) return undefined;
				try {
					hide = backend.show(spec, settle);
				} catch {
					settle(null);
				}
				return finished;
			});
			return {
				result,
				close() {
					closed = true;
					if (!over) hide?.();
					settle(null);
				},
			};
		},
	};
}

export type Dialogs = ReturnType<typeof createDialogs>;

/** The spec as a query string for the page a dialog window loads. */
export function encodeSpec(spec: DialogSpec): string {
	return new URLSearchParams({ spec: JSON.stringify(spec) }).toString();
}

/** The spec in a dialog window's query string, or undefined when it is missing or not one. */
export function decodeSpec(search: string): DialogSpec | undefined {
	try {
		const raw = new URLSearchParams(search).get('spec');
		const v = raw ? (JSON.parse(raw) as Partial<DialogSpec>) : undefined;
		if (!v || typeof v.title !== 'string' || !DIALOG_KINDS.includes(v.kind as DialogKind))
			return undefined;
		return v as DialogSpec;
	} catch {
		return undefined;
	}
}

export const DIALOG_RESULT_EVENT = 'studio://dialog-result';

export interface DialogAnswer {
	id: string;
	result: DialogResult;
}

/** Opens dialogs as child windows of the window that asks, and hears the answer by event. */
export async function tauriDialogBackend(): Promise<DialogBackend> {
	const { invoke } = await import('@tauri-apps/api/core');
	const { listen } = await import('@tauri-apps/api/event');
	const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
	let counter = 0;
	return {
		show(spec, done) {
			const id = `${Date.now().toString(36)}${(counter += 1)}`;
			let stop: (() => void) | undefined;
			let over = false;
			const finish = (result: DialogResult) => {
				if (over) return;
				over = true;
				stop?.();
				done(result);
			};
			void listen<DialogAnswer>(DIALOG_RESULT_EVENT, (e) => {
				if (e.payload.id === id) finish(e.payload.result);
			}).then((off) => {
				if (over) off();
				else stop = off;
			});
			invoke('open_window', { kind: spec.kind, id, query: encodeSpec(spec) + `&id=${id}` }).catch(
				() => finish(null),
			);
			return () => {
				const label = spec.kind === 'confirm' ? `confirm-${id}` : spec.kind;
				void WebviewWindow.getByLabel(label).then((w) => w?.close());
				finish(null);
			};
		},
	};
}

/** Shows dialogs in the page through a function that draws and clears them. For the browser demo only. */
export function inPageDialogBackend(
	draw: (dialog: { spec: DialogSpec; done(result: DialogResult): void } | null) => void,
): DialogBackend {
	return {
		show(spec, done) {
			draw({
				spec,
				done: (result) => {
					draw(null);
					done(result);
				},
			});
			return () => draw(null);
		},
	};
}

/**
 * Opens a window of its own that does not wait for an answer and does not hold up the dialogs that follow
 * (Preferences, Keyboard shortcuts, Connect an agent), or brings it forward when it is open.
 */
export async function openChildWindow(kind: 'preferences' | 'shortcuts' | 'agent'): Promise<void> {
	const { invoke } = await import('@tauri-apps/api/core');
	await invoke('open_window', { kind });
}

/** Opens the full-screen preview window for a target and step, or brings it forward when it is open. */
export async function openPreviewWindow(target: string, step: number): Promise<void> {
	const { invoke } = await import('@tauri-apps/api/core');
	await invoke('open_window', { kind: 'preview', query: previewWindowQuery(target, step) });
}

export const openPreferencesWindow = () => openChildWindow('preferences');
