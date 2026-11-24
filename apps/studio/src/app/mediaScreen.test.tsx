// @vitest-environment happy-dom
// Tests the Media screen through the app: the list, adding and removing with a confirmation, undo and redo.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it } from 'vitest';
import { shortcutTable } from '../chrome/menuModel';
import { quillCd, quillFloppy, quillSet } from '../lib/mediaFixture';
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

describe('the capacity meter', () => {
	it('is a meter with the numbers as its value and as text, and its blocks are hidden', async () => {
		const { root } = await mountMedia([quillFloppy()]);
		const meter = root.querySelector('[role="meter"]')!;
		expect(meter.getAttribute('aria-valuemin')).toBe('0');
		expect(meter.getAttribute('aria-valuemax')).toBe('1457664');
		expect(meter.getAttribute('aria-valuenow')).toBe('1286656');
		expect(meter.getAttribute('aria-valuetext')).toBe(
			'1,286,656 of 1,457,664 bytes used; 171,008 free; Reserve met',
		);
		expect(meter.getAttribute('aria-label')).toBe('Space used on QUILL');
		const cells = meter.querySelector('[aria-hidden="true"]')!;
		expect(cells.textContent).toHaveLength(89);
		expect(cells.textContent).toMatch(/^█+▓+▒░+$/);
	});

	it('lists used, common pack, free and reserve under the bar', async () => {
		const { root } = await mountMedia([quillFloppy()]);
		const totals = root.querySelector('dl')!.textContent!;
		expect(totals).toContain('Used1,286,656');
		expect(totals).toContain('Common pack');
		expect(totals).toContain('Free171,008');
		expect(totals).toContain('Reserve16,384 · Reserve met');
	});

	it('draws a second bar for what the medium would need without sharing, and by how much it is over', async () => {
		const { root } = await mountMedia([quillFloppy()]);
		expect(root.textContent).toContain('Without sharing: needs 1,735,168 bytes — over by 277,504');
	});

	it('says by how much a medium is over, in the pill, the meter and a warning', async () => {
		const m = await mountMedia([{ ...quillFloppy(), exclude: [] }]);
		expect(m.rows()[0]!.textContent).toContain('Does not fit');
		const meter = m.root.querySelector('[role="meter"]')!;
		expect(meter.getAttribute('aria-valuetext')).toMatch(/bytes used; over by [\d,]+$/);
		expect(m.root.textContent).toContain('does_not_fit');
	});

	it('is a plain bar of 80 cells on a CD, with nothing to say about sharing', async () => {
		const { root } = await mountMedia([quillCd()]);
		const meter = root.querySelector('[role="meter"]')!;
		expect(meter.getAttribute('aria-valuemax')).toBe('681574400');
		expect(meter.querySelector('[aria-hidden="true"]')!.textContent).toHaveLength(80);
		expect(root.textContent).not.toContain('Without sharing');
	});
});

describe('the disk map', () => {
	const tree = (root: HTMLElement) => root.querySelector('[role="tree"]')!;
	const items = (root: HTMLElement) => [
		...tree(root).querySelectorAll<HTMLElement>('[role="treeitem"]'),
	];
	const names = (root: HTMLElement) =>
		items(root).map((i) => i.querySelector('span:nth-child(2)')!.textContent);

	it('is an ARIA tree of the disk with its levels, sizes and one tab stop', async () => {
		const { root } = await mountMedia([quillFloppy()]);
		expect(tree(root).getAttribute('aria-label')).toBe('Files on disk 1');
		expect(names(root)).toEqual([
			'A:\\ QUILL',
			'WIN31',
			'SETUP.EXE',
			'SETUP.LPK',
			'WIN95',
			'SETUP.EXE',
			'SETUP.LPK',
			'SETUP.EXE',
			'SETUP.LPK',
			'COMMON.LPK',
			'README.TXT',
		]);
		const level = (i: HTMLElement) => Number(i.getAttribute('aria-level'));
		expect(items(root).map(level)).toEqual([1, 2, 3, 3, 2, 3, 3, 2, 2, 2, 2]);
		expect(items(root).filter((i) => i.tabIndex === 0)).toHaveLength(1);
		expect(items(root)[0]!.getAttribute('aria-expanded')).toBe('true');
		const common = items(root)[9]!;
		expect(common.getAttribute('aria-expanded')).toBe('false');
		expect(common.textContent).toContain('Common pack');
		expect(common.textContent).toContain('239,104');
		expect(items(root)[7]!.getAttribute('aria-posinset')).toBe('3');
		expect(items(root)[7]!.getAttribute('aria-setsize')).toBe('6');
	});

	it('moves with the arrow keys, opens and closes with Right and Left, and picks with Enter', async () => {
		const m = await mountMedia([quillFloppy()]);
		const { root, key } = m;
		await key(items(root)[0]!, 'ArrowDown');
		expect(document.activeElement).toBe(items(root)[1]);
		await key(items(root)[1]!, 'End');
		expect(document.activeElement).toBe(items(root)[10]);
		await key(items(root)[10]!, 'ArrowUp');
		const common = items(root)[9]!;
		expect(document.activeElement).toBe(common);
		await key(common, 'ArrowRight');
		expect(items(root)[9]!.getAttribute('aria-expanded')).toBe('true');
		expect(items(root)[10]!.getAttribute('aria-level')).toBe('3');
		expect(items(root)[10]!.textContent).toContain('C0000001');
		await key(items(root)[9]!, 'ArrowRight');
		expect(document.activeElement).toBe(items(root)[10]);
		await key(items(root)[10]!, 'ArrowLeft');
		expect(document.activeElement).toBe(items(root)[9]);
		await key(items(root)[9]!, 'ArrowLeft');
		expect(items(root)[9]!.getAttribute('aria-expanded')).toBe('false');
		await key(items(root)[9]!, 'Home');
		expect(document.activeElement).toBe(items(root)[0]);
		expect(root.querySelector('[data-testid="details-title"]')!.textContent).toBe('Details');
		await key(items(root)[9]!, 'Enter');
		expect(items(root)[9]!.getAttribute('aria-selected')).toBe('true');
		expect(root.querySelector('[data-testid="details-title"]')!.textContent).toBe('COMMON.LPK');
	});

	it('shows a pack file’s sizes, where it is installed and what its switch would do', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.key(items(m.root)[9]!, 'ArrowRight');
		const dict = items(m.root).find((i) => i.textContent!.includes('C0000008'))!;
		await m.key(dict, 'Enter');
		const aside = m.root.querySelector('aside')!;
		expect(aside.querySelector('h3')!.textContent).toBe('En_ca.dic');
		expect(aside.textContent).toContain('300,086');
		expect(aside.textContent).toContain('210,547');
		expect(aside.textContent).toContain('Canadian dictionary');
		expect(aside.textContent).toMatch(/Leaving it off frees 21\d,\d{3}/);
		expect(aside.textContent).toContain('Windows 95 En_ca.dic');
	});

	it('picks a row by click and shows the pack’s chunk size and set id', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.click(items(m.root)[9]);
		const aside = m.root.querySelector('aside')!;
		expect(aside.textContent).toContain('Common');
		expect(aside.textContent).toContain('4 KiB');
		expect(aside.textContent).toMatch(/Set id[0-9A-F]{8}/);
	});

	it('splits a set of disks into a column for each, with its own meter, and says who is asked for disk 2', async () => {
		const m = await mountMedia([quillSet()]);
		const heads = [...m.root.querySelectorAll('section h3')].map((h) => h.textContent);
		expect(heads).toContain('Disk 1 — QUILL');
		expect(heads).toContain('Disk 2 — QUILL 2');
		expect(m.root.querySelectorAll('[role="tree"]')).toHaveLength(2);
		expect(m.root.querySelectorAll('[role="meter"]')).toHaveLength(2);
		expect(m.root.textContent).toContain(
			'Only people who tick More dictionaries are asked for disk 2.',
		);
		const second = m.root.querySelectorAll('[role="tree"]')[1]!;
		expect([...second.querySelectorAll('[role="treeitem"]')].map((i) => i.id)).toHaveLength(2);
		expect(second.getAttribute('aria-label')).toBe('Files on disk 2');
		expect(m.rows()[0]!.textContent).toContain('2 disks');
		// Each tree is one tab stop of its own.
		expect(m.root.querySelectorAll('[role="treeitem"][tabindex="0"]')).toHaveLength(2);
	});
});

describe('the Sizes tab', () => {
	const open = async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.click(m.root.querySelector('#media-tab-sizes'));
		return m;
	};

	it('is a tab that the arrow keys reach, with its panel labelled by it', async () => {
		const m = await mountMedia([quillFloppy()]);
		const tabs = [...m.root.querySelectorAll<HTMLElement>('[role="tab"]')];
		expect(tabs.map((x) => x.textContent)).toEqual(['Disk map', 'Sizes']);
		expect(tabs[0]!.getAttribute('aria-selected')).toBe('true');
		await m.key(tabs[0]!, 'ArrowRight');
		expect(tabs[1]!.getAttribute('aria-selected')).toBe('true');
		expect(document.activeElement).toBe(tabs[1]);
		const panel = m.root.querySelector('[role="tabpanel"]')!;
		expect(panel.getAttribute('aria-labelledby')).toBe('media-tab-sizes');
		await m.key(tabs[1]!, 'ArrowRight');
		expect(tabs[0]!.getAttribute('aria-selected')).toBe('true');
	});

	it('lists each component with its sizes and what its switch would change', async () => {
		const m = await open();
		const rows = [...m.root.querySelectorAll('table')[0]!.querySelectorAll('tbody tr')].map((r) =>
			[...r.children].map((c) => c.textContent),
		);
		expect(rows.map((r) => r[0])).toEqual([
			'Quill program',
			'Help file',
			'Sample documents',
			'Canadian dictionary',
			'More dictionaries',
		]);
		const dict = rows[3]!;
		expect(dict.slice(1, 5)).toEqual(['On', '300,086', '210,547', 'Common']);
		expect(dict[5]).toBe('DOS, Windows 3.x, Windows 95');
		expect(dict[7]).toMatch(/^Leaving it off frees 21\d,\d{3}$/);
		const more = rows[4]!;
		expect(more[1]).toBe('Off');
		expect(more[7]).toMatch(/^Putting it back needs 4\d\d,\d{3} \(does not fit\)$/);
		expect(rows[0]![1]).toBe('Required');
		expect(rows[0]![7]).toBe('—');
	});

	it('lists each file, and sorts a table by a heading, either way, saying which way in aria-sort', async () => {
		const m = await open();
		const table = m.root.querySelectorAll('table')[1]!;
		const heading = (name: string) =>
			[...table.querySelectorAll<HTMLElement>('thead th')].find((h) =>
				h.textContent!.startsWith(name),
			)!;
		const first = () => table.querySelector('tbody tr th')!.textContent;
		expect(heading('Raw').getAttribute('aria-sort')).toBe('none');
		await m.click(heading('Raw').querySelector('button'));
		expect(heading('Raw').getAttribute('aria-sort')).toBe('ascending');
		expect(first()).toBe('Licence.txt');
		await m.click(heading('Raw').querySelector('button'));
		expect(heading('Raw').getAttribute('aria-sort')).toBe('descending');
		expect(first()).toBe('QUILL32.EXE');
		await m.click(heading('File').querySelector('button'));
		expect(heading('Raw').getAttribute('aria-sort')).toBe('none');
		expect(heading('File').getAttribute('aria-sort')).toBe('ascending');
		expect(table.querySelectorAll('tbody tr').length).toBeGreaterThan(10);
	});
});
