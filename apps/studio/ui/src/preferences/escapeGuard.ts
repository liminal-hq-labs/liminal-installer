// Keeps Escape from closing Preferences while a box holds text that was typed and not applied.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useLayoutEffect, useRef, useState, type FormEvent, type KeyboardEvent, type RefObject } from 'react';

export interface EscapeGuard {
	/** Changes each time Escape puts back typed text; a row keyed by it drops a draft it keeps itself. */
	epoch: number;
	/** Goes on the element that holds the rows, so focus can return to the box that was put back. */
	containerRef: RefObject<HTMLDivElement | null>;
	handlers: {
		onInput(event: FormEvent): void;
		onBlur(): void;
		onKeyDown(event: KeyboardEvent): void;
	};
}

/**
 * The first Escape in a box with text that was typed and not applied puts the old text back and is taken, so the
 * window stays; the next one is left alone and the window closes. Enter and leaving the box apply the text and clear it.
 */
export function useEscapeGuard(): EscapeGuard {
	const typing = useRef<EventTarget | null>(null);
	const [epoch, setEpoch] = useState(0);
	const containerRef = useRef<HTMLDivElement | null>(null);
	const focusIndex = useRef(-1);
	// A row that keeps a draft of its own is remounted to drop it, which loses focus; put it back on the same box.
	useLayoutEffect(() => {
		if (focusIndex.current < 0) return;
		containerRef.current?.querySelectorAll('input')[focusIndex.current]?.focus();
		focusIndex.current = -1;
	}, [epoch]);
	return {
		epoch,
		containerRef,
		handlers: {
			onInput: (event) => {
				if ((event.target as HTMLElement).tagName === 'INPUT') typing.current = event.target;
			},
			onBlur: () => {
				typing.current = null;
			},
			onKeyDown: (event) => {
				if (event.key === 'Enter') typing.current = null;
				if (event.key !== 'Escape') return;
				const input = event.target as HTMLInputElement;
				if (input.tagName === 'INPUT' && typing.current === input) {
					event.preventDefault();
					typing.current = null;
					focusIndex.current = [...(containerRef.current?.querySelectorAll('input') ?? [])].indexOf(input);
					setEpoch((e) => e + 1);
				}
			},
		},
	};
}
