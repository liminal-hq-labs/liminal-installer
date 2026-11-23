// A scripted stand-in for the shell: tests (and a browser, in development) drive the page with it, no window needed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PageAnswer } from '@liminal-hq/installer-protocol/generated/PageAnswer';
import type { PageUpdate } from '@liminal-hq/installer-protocol/generated/PageUpdate';
import type { RunEvent } from '@liminal-hq/installer-protocol/generated/RunEvent';
import type { RunnerApi } from './api';

/** What the test double saw the page ask for. */
export interface TestApi extends RunnerApi {
	answers: PageAnswer[];
	cancels: number;
	exits: number;
	titles: string[];
	updates: Record<string, unknown>[];
	/** Sends an event as the shell would. */
	emit(event: RunEvent): void;
	/** What `update` answers next. */
	nextUpdate: PageUpdate;
	/** What the folder chooser answers next. */
	folder: string | null;
}

/** A double that starts with `initial` already seen. */
export function createTestApi(initial: RunEvent[] = []): TestApi {
	const handlers = new Set<(event: RunEvent) => void>();
	const api: TestApi = {
		answers: [],
		cancels: 0,
		exits: 0,
		titles: [],
		updates: [],
		nextUpdate: { disabled: [], vars: {} },
		folder: null,
		emit: (event) => handlers.forEach((h) => h(event)),
		start: async () => initial,
		onEvent: async (handler) => {
			handlers.add(handler);
			return () => handlers.delete(handler);
		},
		answer: async (answer) => {
			api.answers.push(answer);
		},
		cancel: async () => {
			api.cancels += 1;
		},
		update: async (state) => {
			api.updates.push(state);
			return api.nextUpdate;
		},
		chooseFolder: async () => api.folder,
		setTitle: async (title) => {
			api.titles.push(title);
		},
		exit: async () => {
			api.exits += 1;
		},
	};
	return api;
}
