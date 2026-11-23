// @vitest-environment happy-dom
// Tests that Escape puts back typed text before it is left to close the window.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { useEscapeGuard } from './escapeGuard';

afterEach(cleanup);

/** A window that closes on any Escape nobody took, like the Preferences window. */
function Host({ onClose }: { onClose(): void }) {
	const guard = useEscapeGuard();
	const [value, setValue] = useState('old');
	return (
		<div
			ref={guard.containerRef}
			{...guard.handlers}
			onKeyDown={(e) => {
				guard.handlers.onKeyDown(e);
				if (e.key === 'Escape' && !e.defaultPrevented) onClose();
			}}
		>
			<input
				key={guard.epoch}
				aria-label="Name"
				defaultValue={value}
				onBlur={(e) => setValue(e.target.value)}
			/>
			<button type="button">Other</button>
		</div>
	);
}

describe('escape guard', () => {
	it('takes the first Escape while text is typed, and lets the second close', () => {
		const onClose = vi.fn();
		render(<Host onClose={onClose} />);
		const input = screen.getByLabelText('Name') as HTMLInputElement;
		input.focus();
		fireEvent.input(input, { target: { value: 'new' } });
		fireEvent.keyDown(input, { key: 'Escape' });
		expect(onClose).not.toHaveBeenCalled();
		const again = screen.getByLabelText('Name') as HTMLInputElement;
		expect(again.value).toBe('old');
		expect(document.activeElement).toBe(again);
		fireEvent.keyDown(again, { key: 'Escape' });
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('closes at once when nothing was typed', () => {
		const onClose = vi.fn();
		render(<Host onClose={onClose} />);
		fireEvent.keyDown(screen.getByLabelText('Name'), { key: 'Escape' });
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('closes after Enter or leaving the box applied the text', () => {
		const onClose = vi.fn();
		render(<Host onClose={onClose} />);
		const input = screen.getByLabelText('Name');
		fireEvent.input(input, { target: { value: 'a' } });
		fireEvent.keyDown(input, { key: 'Enter' });
		fireEvent.keyDown(input, { key: 'Escape' });
		expect(onClose).toHaveBeenCalledTimes(1);
		fireEvent.input(input, { target: { value: 'b' } });
		fireEvent.blur(input);
		fireEvent.keyDown(screen.getByLabelText('Name'), { key: 'Escape' });
		expect(onClose).toHaveBeenCalledTimes(2);
	});

	it('leaves Escape alone on a button', () => {
		const onClose = vi.fn();
		render(<Host onClose={onClose} />);
		fireEvent.input(screen.getByLabelText('Name'), { target: { value: 'x' } });
		fireEvent.keyDown(screen.getByText('Other'), { key: 'Escape' });
		expect(onClose).toHaveBeenCalledTimes(1);
	});
});
