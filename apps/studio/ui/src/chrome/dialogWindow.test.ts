// Tests the dialog window abstraction with a fake backend.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import {
	createDialogs,
	decodeSpec,
	encodeSpec,
	inPageDialogBackend,
	type DialogBackend,
	type DialogResult,
	type DialogSpec,
} from './dialogWindow';

const confirm: DialogSpec = {
	kind: 'confirm',
	title: 'Unsaved changes',
	message: 'Save them?',
	buttons: [
		{ id: 'save', label: 'Save', primary: true },
		{ id: 'cancel', label: 'Cancel' },
	],
};

function fakeBackend() {
	const shown: {
		spec: DialogSpec;
		done: (r: DialogResult) => void;
		hide: ReturnType<typeof vi.fn>;
	}[] = [];
	const backend: DialogBackend = {
		show(spec, done) {
			const hide = vi.fn();
			shown.push({ spec, done, hide });
			return hide;
		},
	};
	return { backend, shown };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe('dialog windows', () => {
	it('shows a dialog and resolves with the button chosen', async () => {
		const { backend, shown } = fakeBackend();
		const handle = createDialogs(backend).open(confirm);
		await tick();
		expect(shown).toHaveLength(1);
		expect(shown[0]!.spec).toBe(confirm);
		shown[0]!.done('save');
		await expect(handle.result).resolves.toBe('save');
	});

	it('resolves null when closed from outside, and hides the dialog', async () => {
		const { backend, shown } = fakeBackend();
		const handle = createDialogs(backend).open(confirm);
		await tick();
		handle.close();
		await expect(handle.result).resolves.toBeNull();
		expect(shown[0]!.hide).toHaveBeenCalledOnce();
	});

	it('settles once and ignores a late answer', async () => {
		const { backend, shown } = fakeBackend();
		const handle = createDialogs(backend).open(confirm);
		await tick();
		shown[0]!.done(null);
		shown[0]!.done('save');
		await expect(handle.result).resolves.toBeNull();
		handle.close();
		expect(shown[0]!.hide).not.toHaveBeenCalled();
	});

	it('shows one at a time, in order', async () => {
		const { backend, shown } = fakeBackend();
		const dialogs = createDialogs(backend);
		const first = dialogs.open(confirm);
		const second = dialogs.open({ ...confirm, title: 'Second' });
		await tick();
		expect(shown).toHaveLength(1);
		shown[0]!.done('cancel');
		await first.result;
		await tick();
		expect(shown).toHaveLength(2);
		expect(shown[1]!.spec.title).toBe('Second');
		shown[1]!.done('save');
		await expect(second.result).resolves.toBe('save');
	});

	it('never shows a queued dialog that was closed first', async () => {
		const { backend, shown } = fakeBackend();
		const dialogs = createDialogs(backend);
		const first = dialogs.open(confirm);
		const second = dialogs.open({ ...confirm, title: 'Second' });
		second.close();
		await tick();
		shown[0]!.done('cancel');
		await first.result;
		await tick();
		expect(shown).toHaveLength(1);
		await expect(second.result).resolves.toBeNull();
	});

	it('answers null when the backend cannot show it', async () => {
		const handle = createDialogs({
			show() {
				throw new Error('no window');
			},
		}).open(confirm);
		await expect(handle.result).resolves.toBeNull();
	});

	it('carries a spec through a window query string', () => {
		expect(decodeSpec('?' + encodeSpec(confirm))).toEqual(confirm);
		expect(decodeSpec('?spec=%7Bbroken')).toBeUndefined();
		expect(decodeSpec('')).toBeUndefined();
		expect(
			decodeSpec('?spec=' + encodeURIComponent('{"kind":"shell","title":"x"}')),
		).toBeUndefined();
	});

	it('draws in the page for the browser demo, and clears when answered', async () => {
		const drawn: (null | { spec: DialogSpec; done: (r: DialogResult) => void })[] = [];
		const dialogs = createDialogs(inPageDialogBackend((d) => void drawn.push(d)));
		const handle = dialogs.open(confirm);
		await tick();
		expect(drawn[0]?.spec).toBe(confirm);
		drawn[0]!.done('cancel');
		await expect(handle.result).resolves.toBe('cancel');
		expect(drawn.at(-1)).toBeNull();
	});
});
