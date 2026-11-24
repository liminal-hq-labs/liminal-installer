// @vitest-environment happy-dom
// Tests the Media screen through the app: the list, adding and removing with a confirmation, undo and redo.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it } from 'vitest';
import { shortcutTable } from '../chrome/menuModel';
import { quillCd, quillFloppy } from '../lib/mediaFixture';
import { mountMedia } from './mediaTestKit';

afterEach(() => {
	document.body.replaceChildren();
});

describe('the Media screen', () => {
	it('is the seventh screen, with Ctrl+7 in the shortcuts table and its name in the title', async () => {
		const { root, chrome } = await mountMedia();
		expect(root.querySelector('main h1')?.textContent).toBe('Media');
		expect(chrome.get().title).toBe('Plume · Media');
		expect(root.querySelector('nav a[aria-current="page"]')?.textContent).toBe('Media');
		const view = shortcutTable().find((g) => g.menu === 'View')!;
		expect(view.rows.find((r) => r.id === 'view.media')?.keys).toEqual(['Ctrl+7']);
	});

	it('explains what a medium is when the project has none, and offers to add one', async () => {
		const { root, button } = await mountMedia();
		expect(root.textContent).toContain('No media yet');
		expect(root.querySelector('[role="listbox"]')).toBeNull();
		expect(button('Add floppy')?.disabled).toBe(false);
		expect(button('Add CD')?.disabled).toBe(false);
	});

	it('lists each medium with its kind, a fits pill, the free bytes and a warning count', async () => {
		const { rows } = await mountMedia([quillFloppy(), quillCd()]);
		const [floppy, cd] = rows();
		expect(floppy!.textContent).toContain('floppy');
		expect(floppy!.textContent).toContain('Fits');
		expect(floppy!.textContent).toContain('171,008 bytes free');
		expect(floppy!.textContent).toContain('2 warnings');
		expect(cd!.textContent).toContain('CD');
		expect(floppy!.getAttribute('aria-selected')).toBe('true');
		expect(cd!.getAttribute('aria-selected')).toBe('false');
	});

	it('adds a floppy and a CD with every target, picks the new one and records an undo entry for each', async () => {
		const { root, button, click, rows, media } = await mountMedia();
		await click(button('Add floppy'));
		expect(rows().map((r) => r.id)).toEqual(['media-row-floppy']);
		await click(button('Add CD'));
		await click(button('Add floppy'));
		expect(rows().map((r) => r.id)).toEqual([
			'media-row-floppy',
			'media-row-cd',
			'media-row-floppy-2',
		]);
		expect(rows()[2]!.getAttribute('aria-selected')).toBe('true');
		expect(root.querySelector('#media-detail-h')?.textContent).toBe('floppy-2');
		expect(media.calls.filter((c) => c.startsWith('addMedia'))).toHaveLength(3);
		const added = await media.listMedia();
		expect(added[0]).toMatchObject({
			kind: 'floppy-1440',
			label: 'QUILL',
			common: 'auto',
			overflow: 'error',
			reserve: 16384,
		});
		expect(added[0]!.targets.map((t) => [t.target, t.dir])).toEqual([
			['dos-x86', ''],
			['win16-x86', 'WIN31'],
			['windows-x86', 'WIN95'],
		]);
		expect(added[1]).toMatchObject({ kind: 'cdrom', autorun: 'windows-x86' });
		expect(added[0]!.autorun).toBeUndefined();
	});

	it('asks in a confirmation window before it removes a medium, and does nothing when asked not to', async () => {
		const m = await mountMedia([quillFloppy(), quillCd()], { answer: 'cancel' });
		await m.click(m.button('Remove medium'));
		expect(m.asked).toHaveLength(1);
		expect(m.asked[0]).toMatchObject({ kind: 'confirm', title: 'Remove Medium' });
		expect(m.asked[0]!.message).toContain('floppy');
		expect(m.asked[0]!.buttons!.map((b) => [b.id, b.danger ?? false])).toEqual([
			['remove', true],
			['cancel', false],
		]);
		expect(m.rows()).toHaveLength(2);
		expect(m.media.calls.some((c) => c.startsWith('removeMedia'))).toBe(false);
	});

	it('removes the medium when confirmed and picks the one that is left', async () => {
		const m = await mountMedia([quillFloppy(), quillCd()], { answer: 'remove' });
		await m.click(m.button('Remove medium'));
		expect(m.rows().map((r) => r.id)).toEqual(['media-row-cd']);
		expect(m.rows()[0]!.getAttribute('aria-selected')).toBe('true');
		await m.click(m.button('Remove medium'));
		expect(m.root.textContent).toContain('No media yet');
	});

	it('removes nothing when the window is closed or nothing can ask', async () => {
		const closed = await mountMedia([quillFloppy()], { answer: null });
		await closed.click(closed.button('Remove medium'));
		expect(closed.rows()).toHaveLength(1);
		document.body.replaceChildren();
		const mute = await mountMedia([quillFloppy()]);
		await mute.click(mute.button('Remove medium'));
		expect(mute.rows()).toHaveLength(1);
	});

	it('undoes and redoes an add and a removal from the Edit menu', async () => {
		const m = await mountMedia([quillFloppy()], { answer: 'remove' });
		await m.click(m.button('Add CD'));
		expect(m.rows()).toHaveLength(2);
		await m.click(m.button('Remove medium'));
		expect(m.rows().map((r) => r.id)).toEqual(['media-row-floppy']);
		await m.act(() => m.app.dispatch('edit.undo'));
		expect(m.rows().map((r) => r.id)).toEqual(['media-row-floppy', 'media-row-cd']);
		await m.act(() => m.app.dispatch('edit.undo'));
		expect(m.rows().map((r) => r.id)).toEqual(['media-row-floppy']);
		await m.act(() => m.app.dispatch('edit.redo'));
		expect(m.rows().map((r) => r.id)).toEqual(['media-row-floppy', 'media-row-cd']);
	});

	it('moves the pick with the arrow keys, Home and End, and asks to remove on Delete', async () => {
		const m = await mountMedia([quillFloppy(), quillCd()], { answer: 'cancel' });
		const list = m.root.querySelector('[role="listbox"]')!;
		await m.key(m.rows()[0]!, 'ArrowDown');
		expect(m.rows()[1]!.getAttribute('aria-selected')).toBe('true');
		expect(m.rows()[1]!.tabIndex).toBe(0);
		expect(m.rows()[0]!.tabIndex).toBe(-1);
		await m.key(m.rows()[1]!, 'ArrowDown');
		expect(m.rows()[1]!.getAttribute('aria-selected')).toBe('true');
		await m.key(m.rows()[1]!, 'Home');
		expect(m.rows()[0]!.getAttribute('aria-selected')).toBe('true');
		await m.key(m.rows()[0]!, 'End');
		expect(m.rows()[1]!.getAttribute('aria-selected')).toBe('true');
		await m.key(m.rows()[1]!, 'Delete');
		expect(m.asked).toHaveLength(1);
		expect(m.asked[0]!.message).toContain('cd');
		expect(list.getAttribute('aria-label')).toBe('Media');
	});

	it('says why when the backend cannot plan media, and offers no way to add', async () => {
		const { root, button } = await mountMedia([], { unavailable: true });
		const alert = root.querySelector('[role="alert"]');
		expect(alert?.textContent).toContain('not available in this build yet');
		expect(button('Add floppy')?.disabled).toBe(true);
	});
});
