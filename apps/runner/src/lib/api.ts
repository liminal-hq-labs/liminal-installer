// The runner's side of the Tauri commands, and a double for tests and the browser.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PageAnswer } from '@liminal-hq/installer-protocol/generated/PageAnswer';
import type { PageUpdate } from '@liminal-hq/installer-protocol/generated/PageUpdate';
import type { RunEvent } from '@liminal-hq/installer-protocol/generated/RunEvent';
import type { ShellError } from '@liminal-hq/installer-protocol/generated/ShellError';

/** Where the shell sends each event of the run. */
export const EVENT = 'runner://event';

/** What the page asks of the shell; `tauriApi` is the real one. */
export interface RunnerApi {
	/** Starts the run (once) and returns the events already sent, so a reload starts where it was. */
	start(): Promise<RunEvent[]>;
	/** Calls `handler` for each event after `start`; the result stops listening. */
	onEvent(handler: (event: RunEvent) => void): Promise<() => void>;
	answer(answer: PageAnswer): Promise<void>;
	/** Cancels: the shell stops the commit at its next operation and the engine rolls back. */
	cancel(): Promise<void>;
	/** Which nodes are switched off, and the values that follow, once the page's values are `state`. */
	update(state: Record<string, unknown>): Promise<PageUpdate>;
	/** Asks for a folder with the system's chooser; null when the person declines. */
	chooseFolder(current: string): Promise<string | null>;
	setTitle(title: string): Promise<void>;
	/** Ends the program with the run's exit code. */
	exit(): Promise<void>;
}

/** Whether this page runs inside the Tauri shell. */
export const inTauri = (): boolean =>
	typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** The text of a failed command: the shell's `{ kind, message }`, or whatever else was thrown. */
export function describeError(e: unknown): string {
	if (typeof e === 'object' && e !== null && 'message' in e) {
		return String((e as ShellError).message);
	}
	return String(e);
}

/** The commands of `src-tauri`. */
export async function tauriApi(): Promise<RunnerApi> {
	const { invoke } = await import('@tauri-apps/api/core');
	const { listen } = await import('@tauri-apps/api/event');
	const { getCurrentWindow } = await import('@tauri-apps/api/window');
	return {
		start: () => invoke<RunEvent[]>('runner_start'),
		onEvent: (handler) => listen<RunEvent>(EVENT, (e) => handler(e.payload)),
		answer: (answer) => invoke('runner_answer', { answer }),
		cancel: () => invoke('runner_cancel'),
		update: (state) => invoke<PageUpdate>('runner_update', { state }),
		chooseFolder: async (current) => {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const picked = await open({ directory: true, multiple: false, defaultPath: current });
			return typeof picked === 'string' ? picked : null;
		},
		setTitle: (title) => getCurrentWindow().setTitle(title),
		exit: () => invoke('runner_exit'),
	};
}
