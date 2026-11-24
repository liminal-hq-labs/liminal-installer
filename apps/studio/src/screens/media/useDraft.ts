// A text field that keeps what is typed until it is valid, and writes it when the field is left or Enter is pressed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useRef, useState, type ChangeEvent, type KeyboardEvent } from 'react';

export interface Draft {
	/** What is in the field now. */
	draft: string;
	/** Why the draft cannot be written, or null. */
	error: string | null;
	/** Props for the `input`: its value, the handlers and `aria-invalid`. */
	props: {
		value: string;
		onChange(e: ChangeEvent<HTMLInputElement>): void;
		onBlur(): void;
		onKeyDown(e: KeyboardEvent<HTMLInputElement>): void;
		'aria-invalid': boolean;
	};
}

/**
 * `value` is what is stored. The field shows what is typed, checks it with `validate` on every change, and
 * calls `commit` once with a valid draft that differs from `value`, when the field loses focus or Enter is
 * pressed. Escape puts `value` back. When `value` changes from outside, as an undo does, the draft follows it.
 */
export function useDraft(
	value: string,
	validate: (draft: string) => string | null,
	commit: (draft: string) => void,
	transform: (typed: string) => string = (typed) => typed,
): Draft {
	const [draft, setDraft] = useState(value);
	const seen = useRef(value);
	if (seen.current !== value) {
		seen.current = value;
		setDraft(value);
	}
	const error = validate(draft);
	const flush = () => {
		if (error === null && draft !== value) commit(draft);
	};
	return {
		draft,
		error,
		props: {
			value: draft,
			onChange: (e) => setDraft(transform(e.target.value)),
			onBlur: flush,
			onKeyDown: (e) => {
				if (e.key === 'Enter') flush();
				else if (e.key === 'Escape') setDraft(value);
			},
			'aria-invalid': error !== null,
		},
	};
}
