// The hook that owns one live terminal session: opens it, sends keys in order, closes it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useRef, useState } from 'react';
import { errorMessage } from '../lib/api';
import type { TuiApi } from '../lib/api';
import type { TuiFrame, TuiKey, TuiOutcome } from '../lib/types';

export interface TuiSessionOptions {
	/** The `os-arch:renderer` target, a `tui` one. */
	target: string;
	/** An install-flow step id, or a layout id. */
	page: string;
	/** The page state the screen starts from. */
	state?: Record<string, unknown>;
	cols?: number;
	rows?: number;
	/** Reopens the session when it changes, such as after a layout edit. */
	refreshKey?: unknown;
	/** Called after a key with what it did. */
	onOutcome?: (outcome: TuiOutcome, frame: TuiFrame) => void;
}

export interface TuiSession {
	frame: TuiFrame | null;
	error: string | null;
	/** Sends one key. Keys are delivered in order; one pressed before the session opens is dropped. */
	press: (key: TuiKey) => void;
}

/**
 * Opens a terminal session on `target` and `page`, and keeps no terminal state of its own: the
 * frame is whatever the last reply drew. Changing the page, target, starting state or size closes
 * the session and opens another; the last frame stays on show until the new one arrives.
 */
export function useTuiSession(api: TuiApi, options: TuiSessionOptions): TuiSession {
	const { target, page, cols = 80, rows = 24, refreshKey } = options;
	const stateKey = JSON.stringify(options.state ?? {});
	const [frame, setFrame] = useState<TuiFrame | null>(null);
	const [error, setError] = useState<string | null>(null);
	const session = useRef<string | null>(null);
	const queue = useRef<Promise<void>>(Promise.resolve());
	const onOutcome = useRef(options.onOutcome);
	onOutcome.current = options.onOutcome;

	useEffect(() => {
		let live = true;
		let id: string | null = null;
		setError(null);
		void (async () => {
			try {
				const opened = await api.tuiSessionOpen(
					target,
					page,
					JSON.parse(stateKey) as Record<string, unknown>,
					{ cols, rows },
				);
				if (!live) {
					void api.tuiSessionClose(opened.session).catch(() => undefined);
					return;
				}
				id = opened.session;
				session.current = id;
				queue.current = Promise.resolve();
				setFrame(opened.frame);
			} catch (e) {
				if (live) {
					session.current = null;
					setError(errorMessage(e));
				}
			}
		})();
		return () => {
			live = false;
			if (session.current === id) session.current = null;
			if (id) void api.tuiSessionClose(id).catch(() => undefined);
		};
	}, [api, target, page, stateKey, cols, rows, refreshKey]);

	const press = useCallback(
		(key: TuiKey) => {
			const id = session.current;
			if (!id) return;
			queue.current = queue.current.then(async () => {
				if (session.current !== id) return;
				try {
					const result = await api.tuiSessionKey(id, key);
					if (session.current !== id) return;
					setFrame(result.frame);
					onOutcome.current?.(result.outcome, result.frame);
				} catch (e) {
					setError(errorMessage(e));
				}
			});
		},
		[api],
	);

	return { frame, error, press };
}
