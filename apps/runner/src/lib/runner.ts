// The run as the page sees it: events in, answers out, and the values the person has typed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PageView } from '@liminal-hq/installer-protocol/generated/PageView';
import type { ProgressView } from '@liminal-hq/installer-protocol/generated/ProgressView';
import type { RunEvent } from '@liminal-hq/installer-protocol/generated/RunEvent';
import type { RunInfo } from '@liminal-hq/installer-protocol/generated/RunInfo';
import { createStore, type StoreApi } from 'zustand/vanilla';
import { t } from '../i18n/messages';
import { describeError, type RunnerApi } from './api';
import { isForward } from './keys';

export type Phase = 'starting' | 'running' | 'done' | 'cancelled' | 'failed';

export interface RunnerState {
	phase: Phase;
	info: RunInfo | null;
	/** The page on show, or null before the first and between a commit and the next page. */
	page: PageView | null;
	/** What the page's controls hold, under their bound keys. */
	values: Record<string, unknown>;
	vars: Record<string, string>;
	/** Paths of the nodes the page has switched off. */
	disabled: string[];
	progress: ProgressView | null;
	/** Why the run failed, when it did. */
	message: string;
	/** Whether the flow's own finish page was shown. */
	sawFinish: boolean;
}

export interface Runner {
	store: StoreApi<RunnerState>;
	/** Starts listening and starts the run. */
	start(): Promise<() => void>;
	/** A control changed: remember it and ask which nodes are on now. */
	change(key: string, value: unknown): void;
	/** A button was pressed. */
	act(action: string): void;
}

const initial: RunnerState = {
	phase: 'starting',
	info: null,
	page: null,
	values: {},
	vars: {},
	disabled: [],
	progress: null,
	message: '',
	sawFinish: false,
};

/** Creates a run driven through `api`. */
export function createRunner(api: RunnerApi): Runner {
	const store = createStore<RunnerState>(() => ({ ...initial }));
	let asked = 0;
	// An answer is on its way: a second press (a double click) must not answer the next page too.
	let pending = false;

	const apply = (event: RunEvent): void => {
		switch (event.type) {
			case 'info':
				store.setState({ info: event.info, phase: 'running' });
				void api.setTitle(t(`runner.title.${event.info.mode}`, { name: event.info.product.name }));
				break;
			case 'page': {
				const page = event.page;
				pending = false;
				store.setState((s) => ({
					page,
					values: { ...page.state },
					vars: { ...page.vars },
					disabled: page.disabled,
					progress: page.kind === 'commit' ? null : s.progress,
					sawFinish: s.sawFinish || page.kind === 'finish',
				}));
				break;
			}
			case 'progress':
				store.setState((s) => ({
					progress: event.progress,
					vars: { ...s.vars, ...event.progress.vars },
				}));
				break;
			case 'done':
				store.setState({ phase: 'done' });
				// The flow's own finish page was the goodbye; there is nothing more to say.
				if (store.getState().sawFinish) void api.exit();
				break;
			case 'cancelled':
				store.setState({ phase: 'cancelled' });
				void api.exit();
				break;
			case 'failed':
				store.setState({ phase: 'failed', message: event.message, page: null });
				break;
		}
	};

	const answer = (action: string): void => {
		const { values, page } = store.getState();
		if (!page || pending) return;
		pending = true;
		void api.answer({ action, state: values });
	};

	const change = (key: string, value: unknown): void => {
		const next = { ...store.getState().values, [key]: value };
		store.setState((s) => ({
			values: next,
			vars: key === 'install.path' ? { ...s.vars, 'install.path': String(value) } : s.vars,
		}));
		const mine = ++asked;
		api
			.update(next)
			.then((update) => {
				if (mine !== asked) return;
				store.setState((s) => ({
					disabled: update.disabled,
					vars: { ...s.vars, ...update.vars, ...(s.progress?.vars ?? {}) },
				}));
			})
			.catch((e) => store.setState({ message: describeError(e) }));
	};

	const act = (action: string): void => {
		const s = store.getState();
		if (action === 'close') {
			void api.exit();
		} else if (action === 'browse') {
			const current = String(s.values['install.path'] ?? s.info?.install_dir ?? '');
			void api.chooseFolder(current).then((folder) => {
				if (folder) change('install.path', folder);
			});
		} else if (action === 'cancel') {
			if (s.page?.kind === 'commit' || !s.page) void api.cancel();
			else answer('cancel');
		} else if (action === 'back') {
			answer('back');
		} else if (isForward(action) && s.page && s.page.kind !== 'commit') {
			answer('next');
		}
	};

	return {
		store,
		async start() {
			const stop = await api.onEvent(apply);
			const seen = await api.start();
			for (const event of seen) apply(event);
			return stop;
		},
		change,
		act,
	};
}
