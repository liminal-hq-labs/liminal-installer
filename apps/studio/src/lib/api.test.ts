// Tests for how the UI turns a failed command into a typed `StudioError`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { StudioError, errorMessage, invokeCommand, isStudioError, toStudioError } from './api';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

describe('toStudioError', () => {
	it('reads the { kind, message } the shell sends', () => {
		const e = toStudioError({ kind: 'not_found', message: 'flow `x` not found' });
		expect(e).toBeInstanceOf(StudioError);
		expect(e).toBeInstanceOf(Error);
		expect(e.kind).toBe('not_found');
		expect(e.message).toBe('flow `x` not found');
	});

	it('accepts every kind the shell can send', () => {
		for (const kind of [
			'not_found',
			'invalid_input',
			'io',
			'project',
			'script',
			'backend',
			'agent',
			'internal',
		] as const) {
			expect(toStudioError({ kind, message: 'm' }).kind).toBe(kind);
		}
	});

	it('takes a kind it does not know as internal, and keeps the message', () => {
		const e = toStudioError({ kind: 'from_the_future', message: 'odd' });
		expect([e.kind, e.message]).toEqual(['internal', 'odd']);
	});

	it('turns a string or an Error into an internal error with the same text', () => {
		const fromString = toStudioError('no project is open');
		expect([fromString.kind, fromString.message]).toEqual(['internal', 'no project is open']);
		expect(toStudioError(new Error('boom')).kind).toBe('internal');
		expect(toStudioError(42).message).toBe('42');
		expect(toStudioError({ kind: 'io' }).kind).toBe('internal');
	});

	it('keeps a StudioError as it is', () => {
		const e = new StudioError('io', 'denied');
		expect(toStudioError(e)).toBe(e);
	});
});

describe('isStudioError and errorMessage', () => {
	it('branch on the kind', () => {
		const e = new StudioError('invalid_input', 'bad');
		expect(isStudioError(e)).toBe(true);
		expect(isStudioError(e, 'invalid_input')).toBe(true);
		expect(isStudioError(e, 'io')).toBe(false);
		expect(isStudioError(new Error('bad'))).toBe(false);
	});

	it('shows the message of any failure', () => {
		expect(errorMessage({ kind: 'io', message: '/x: denied' })).toBe('/x: denied');
		expect(errorMessage(new Error('plain'))).toBe('plain');
		expect(errorMessage('text')).toBe('text');
	});
});

describe('invokeCommand', () => {
	beforeEach(() => {
		invoke.mockReset();
	});

	it('resolves with the command result', async () => {
		invoke.mockResolvedValue({ ok: true });
		await expect(invokeCommand('get_flow', { id: 'a' })).resolves.toEqual({ ok: true });
		expect(invoke).toHaveBeenCalledWith('get_flow', { id: 'a' });
	});

	it('rejects with a typed StudioError', async () => {
		invoke.mockRejectedValue({ kind: 'project', message: 'no project is open' });
		const error = await invokeCommand('project_summary').then(
			() => null,
			(e: unknown) => e,
		);
		expect(error).toBeInstanceOf(StudioError);
		expect(isStudioError(error, 'project')).toBe(true);
		expect(errorMessage(error)).toBe('no project is open');
	});
});
