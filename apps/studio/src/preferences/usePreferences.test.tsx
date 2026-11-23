// @vitest-environment happy-dom
// Tests the Preferences hook: optimistic apply, rollback on refusal, and following a change made elsewhere.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it } from 'vitest';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { refusal, usePreferences } from './usePreferences';
import { testPort } from './testPort';

afterEach(cleanup);

describe('usePreferences', () => {
	it('is empty until the settings are read, then holds them', async () => {
		const port = testPort({ tabWidth: 2 });
		const { result } = renderHook(() => usePreferences(port));
		expect(result.current.settings).toBeNull();
		await waitFor(() => expect(result.current.settings?.tabWidth).toBe(2));
	});

	it('shows a change before the write is answered', async () => {
		const port = testPort();
		let release!: () => void;
		const slow = new Promise<void>((r) => (release = r));
		const write = port.write.bind(port);
		port.write = async (patch) => {
			await slow;
			return write(patch);
		};
		const { result } = renderHook(() => usePreferences(port));
		await waitFor(() => expect(result.current.settings).not.toBeNull());
		let done!: Promise<boolean>;
		act(() => {
			done = result.current.update({ menuBar: true });
		});
		expect(result.current.settings?.menuBar).toBe(true);
		expect(port.current.menuBar).toBe(false);
		release();
		await act(async () => void (await done));
		expect(port.current.menuBar).toBe(true);
	});

	it('writes a patch, not the whole settings', async () => {
		const port = testPort();
		const { result } = renderHook(() => usePreferences(port));
		await waitFor(() => expect(result.current.settings).not.toBeNull());
		await act(async () => void (await result.current.update({ wrapLines: false })));
		expect(port.writes).toEqual([{ wrapLines: false }]);
	});

	it('puts the value back and keeps the reason when a write is refused', async () => {
		const port = testPort();
		const { result } = renderHook(() => usePreferences(port));
		await waitFor(() => expect(result.current.settings).not.toBeNull());
		port.refuseNext('port 8123 is in use');
		let saved = true;
		await act(async () => {
			saved = await result.current.update({ mcpPort: 8123 });
		});
		expect(saved).toBe(false);
		expect(result.current.settings?.mcpPort).toBe(0);
		expect(result.current.failures.mcpPort).toBe('port 8123 is in use');
		expect(refusal(result.current.failures, 'mcpPort')).toEqual({
			error: 'Not saved: port 8123 is in use',
		});
		expect(refusal(result.current.failures, 'tabWidth')).toEqual({});
	});

	it('clears the reason when the same setting is changed again', async () => {
		const port = testPort();
		const { result } = renderHook(() => usePreferences(port));
		await waitFor(() => expect(result.current.settings).not.toBeNull());
		port.refuseNext('no');
		await act(async () => void (await result.current.update({ tabWidth: 2 })));
		expect(result.current.failures.tabWidth).toBe('no');
		await act(async () => void (await result.current.update({ tabWidth: 3 })));
		expect(result.current.failures.tabWidth).toBeUndefined();
		expect(result.current.settings?.tabWidth).toBe(3);
	});

	it('refuses an out-of-range value through the port and restores the old one', async () => {
		const port = testPort();
		const { result } = renderHook(() => usePreferences(port));
		await waitFor(() => expect(result.current.settings).not.toBeNull());
		await act(async () => void (await result.current.update({ textSize: 500 })));
		expect(result.current.settings?.textSize).toBe(100);
		expect(result.current.failures.textSize).toBe('text size must be between 90 and 130');
	});

	it('follows a change made elsewhere', async () => {
		const port = testPort();
		const { result } = renderHook(() => usePreferences(port));
		await waitFor(() => expect(result.current.settings).not.toBeNull());
		act(() => port.external({ theme: 'afterglow-light' }));
		expect(result.current.settings?.theme).toBe('afterglow-light');
	});

	it('stops listening when it unmounts', async () => {
		const port = testPort();
		const { result, unmount } = renderHook(() => usePreferences(port));
		await waitFor(() => expect(result.current.settings).not.toBeNull());
		unmount();
		expect(() => port.external({ theme: 'afterglow-light' })).not.toThrow();
	});

	it('resets every setting', async () => {
		const port = testPort({ tabWidth: 2 });
		const { result } = renderHook(() => usePreferences(port));
		await waitFor(() => expect(result.current.settings?.tabWidth).toBe(2));
		await act(async () => void (await result.current.reset()));
		expect(result.current.settings?.tabWidth).toBe(4);
	});
});
