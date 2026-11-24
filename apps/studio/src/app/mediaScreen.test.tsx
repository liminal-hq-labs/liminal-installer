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
		expect(m.root.textContent).toContain('Does not fit');
		expect(m.root.textContent).toContain('Open Sizes');
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
		expect(tabs.map((x) => x.textContent)).toEqual([
			'Disk map',
			'Sizes',
			'Targets',
			'Label and README',
			'Autorun',
		]);
		expect(tabs[0]!.getAttribute('aria-selected')).toBe('true');
		await m.key(tabs[0]!, 'ArrowRight');
		expect(tabs[1]!.getAttribute('aria-selected')).toBe('true');
		expect(document.activeElement).toBe(tabs[1]);
		const panel = m.root.querySelector('[role="tabpanel"]')!;
		expect(panel.getAttribute('aria-labelledby')).toBe('media-tab-sizes');
		await m.key(tabs[1]!, 'End');
		expect(tabs[4]!.getAttribute('aria-selected')).toBe('true');
		await m.key(tabs[4]!, 'ArrowRight');
		expect(tabs[0]!.getAttribute('aria-selected')).toBe('true');
		await m.key(tabs[0]!, 'ArrowLeft');
		expect(tabs[4]!.getAttribute('aria-selected')).toBe('true');
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
		expect(dict.slice(1, 5)).toEqual(['■ On', '300,086', '210,547', 'Common']);
		expect(dict[5]).toBe('DOS, Windows 3.x, Windows 95');
		expect(dict[7]).toMatch(/^Leaving it off frees 21\d,\d{3}$/);
		const more = rows[4]!;
		expect(more[1]).toBe('□ Off');
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

describe('the switches', () => {
	const freeText = (root: HTMLElement) => root.querySelector('[role="status"]')!.textContent;
	const meterText = (root: HTMLElement) =>
		root.querySelector('[role="meter"]')!.getAttribute('aria-valuetext');
	const toggle = (root: HTMLElement, component: string) =>
		root.querySelector<HTMLElement>(`[role="switch"][aria-label="On this medium: ${component}"]`)!;

	it('are role=switch, say whether they are on, and are described by a note of what they would change', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('sizes');
		const dict = toggle(m.root, 'Canadian dictionary');
		expect(dict.getAttribute('aria-checked')).toBe('true');
		expect(toggle(m.root, 'More dictionaries').getAttribute('aria-checked')).toBe('false');
		const note = m.root.querySelector(`#${dict.getAttribute('aria-describedby')}`)!;
		expect(note.textContent).toMatch(/^Leaving it off frees 21\d,\d{3}$/);
		expect(m.root.querySelectorAll('[role="switch"]')).toHaveLength(3);
	});

	it('leaving a component off frees the bytes its note promised, at once, and undo takes it back', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('sizes');
		expect(freeText(m.root)).toBe('171,008 bytes free on QUILL');
		const note = m.root.querySelector('#switch-note-dicts')!.textContent!;
		const frees = Number(/frees ([\d,]+)/.exec(note)![1]!.replace(/,/g, ''));
		await m.click(toggle(m.root, 'Canadian dictionary'));
		const freed = `${(171_008 + frees).toLocaleString('en-CA')} bytes free on QUILL`;
		expect(freeText(m.root)).toBe(freed);
		expect(meterText(m.root)).toContain(`${(171_008 + frees).toLocaleString('en-CA')} free`);
		expect(toggle(m.root, 'Canadian dictionary').getAttribute('aria-checked')).toBe('false');
		expect(m.root.querySelector('#switch-note-dicts')!.textContent).toBe(
			`Putting it back needs ${frees.toLocaleString('en-CA')}`,
		);
		expect(m.media.edits().at(-1)).toBe('setMedia(floppy, {"exclude":["dicts-more","dicts"]})');
		expect((await m.media.history()).entries.at(-1)!.label).toBe(
			'Leave Canadian dictionary off medium floppy',
		);
		await m.act(() => m.app.dispatch('edit.undo'));
		expect(freeText(m.root)).toBe('171,008 bytes free on QUILL');
		expect(toggle(m.root, 'Canadian dictionary').getAttribute('aria-checked')).toBe('true');
		await m.act(() => m.app.dispatch('edit.redo'));
		expect(freeText(m.root)).toBe(freed);
	});

	it('warn that putting the other dictionaries back does not fit, and then say so on the list', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('sizes');
		expect(m.root.querySelector('#switch-note-dicts-more')!.textContent).toMatch(
			/^Putting it back needs 4\d\d,\d{3} \(does not fit\)$/,
		);
		expect(m.rows()[0]!.textContent).toContain('Fits');
		await m.click(toggle(m.root, 'More dictionaries'));
		expect(m.rows()[0]!.textContent).toContain('Does not fit');
		expect(freeText(m.root)).toMatch(/^-\d/);
		expect(m.root.querySelector('#switch-note-dicts-more')!.textContent).toMatch(
			/^Leaving it off frees/,
		);
	});

	it('leave a required component without a switch', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('sizes');
		const row = [...m.root.querySelectorAll('table')[0]!.querySelectorAll('tbody tr')][1]!;
		expect(row.textContent).toContain('Help file');
		expect(row.querySelector('[role="switch"]')).toBeNull();
		expect(row.children[1]!.textContent).toBe('Required');
	});
});

describe('the label', () => {
	const field = (root: HTMLElement) => root.querySelector<HTMLInputElement>('#media-label')!;

	it('counts as it is typed, shows the rule, and writes nothing while the label is too long', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('label');
		expect(field(m.root).value).toBe('QUILL');
		expect(m.root.textContent).toContain('5 / 11');
		expect(m.root.querySelector('#media-label-rule')!.textContent).toContain(
			'Up to 11 capital letters',
		);
		await m.type(field(m.root), 'QUILL RELEASE');
		expect(m.root.textContent).toContain('13 / 11');
		expect(m.root.querySelector('#media-label-rule')!.textContent).toBe(
			'The label is 13 characters; the most is 11.',
		);
		expect(field(m.root).getAttribute('aria-invalid')).toBe('true');
		await m.key(field(m.root), 'Enter');
		await m.leave(field(m.root));
		expect(m.media.calls.some((c) => c.startsWith('setMedia'))).toBe(false);
	});

	it('turns what is typed into capitals, refuses a character a label may not hold, and writes a good one on Enter', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('label');
		await m.type(field(m.root), 'a.b');
		expect(field(m.root).value).toBe('A.B');
		expect(m.root.querySelector('#media-label-rule')!.textContent).toBe(
			'“.” cannot be in a label.',
		);
		await m.type(field(m.root), 'quill 2');
		expect(field(m.root).value).toBe('QUILL 2');
		expect(field(m.root).getAttribute('aria-invalid')).toBe('false');
		await m.key(field(m.root), 'Enter');
		expect(m.media.edits().at(-1)).toBe('setMedia(floppy, {"label":"QUILL 2"})');
		expect(m.rows()[0]!.textContent).toContain('QUILL 2');
		expect((await m.media.history()).entries.at(-1)!.label).toBe('Set label of medium floppy');
	});

	it('writes a good label when the field is left, puts the old one back on Escape and follows an undo', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('label');
		await m.type(field(m.root), 'DISK ONE');
		await m.leave(field(m.root));
		expect(m.media.edits().at(-1)).toBe('setMedia(floppy, {"label":"DISK ONE"})');
		await m.type(field(m.root), 'OTHER');
		await m.key(field(m.root), 'Escape');
		expect(field(m.root).value).toBe('DISK ONE');
		await m.act(() => m.app.dispatch('edit.undo'));
		expect(field(m.root).value).toBe('QUILL');
	});

	it('allows 16 characters on a CD and shows its ISO volume id and Joliet label', async () => {
		const m = await mountMedia([quillCd()]);
		await m.tab('label');
		expect(m.root.textContent).toContain('5 / 16');
		await m.type(field(m.root), 'QUILL 1.0 DISC');
		expect(m.root.querySelector('#media-label-rule')!.textContent).toBe(
			'“.” cannot be in a label.',
		);
		await m.type(field(m.root), 'QUILL ONE DISC');
		await m.key(field(m.root), 'Enter');
		const facts = [...m.root.querySelectorAll('dl')].map((d) => d.textContent).join(' ');
		expect(facts).toContain('ISO volume idQUILL_ONE_DISC');
		expect(facts).toContain('Joliet labelQUILL ONE DISC');
	});
});

describe('the serial', () => {
	it('is automatic by default, and a fixed one is eight hex digits shown as XXXX-XXXX', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('label');
		const radios = [...m.root.querySelectorAll<HTMLInputElement>('input[type="radio"]')];
		expect(radios.map((r) => r.checked)).toEqual([true, false]);
		expect(m.root.textContent).toContain('Currently 0001-0000');
		await m.click(radios[1]);
		expect(m.media.edits().at(-1)).toBe('setMedia(floppy, {"serial":"0001-0000"})');
		const serial = m.root.querySelector<HTMLInputElement>('input[aria-label="Serial number"]')!;
		await m.type(serial, 'dead-be');
		expect(m.root.textContent).toContain('A serial is eight hexadecimal digits.');
		await m.type(serial, 'dead-beef');
		await m.key(serial, 'Enter');
		expect(m.media.edits().at(-1)).toBe('setMedia(floppy, {"serial":"DEAD-BEEF"})');
		await m.tab('map');
		expect(m.root.textContent).toContain('Serial DEAD-BEEF');
		await m.tab('label');
		await m.click(m.root.querySelectorAll('input[type="radio"]')[0]);
		expect(m.media.edits().at(-1)).toBe('setMedia(floppy, {"serial":"auto"})');
	});
});

describe('the targets', () => {
	const folder = (root: HTMLElement, name: string) =>
		root.querySelector<HTMLInputElement>(`input[aria-label="Folder for ${name}"]`)!;

	it('list each target with its folder, whether its SETUP.EXE was found, and its size', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('targets');
		const rows = [...m.root.querySelectorAll('tbody tr')];
		expect(rows).toHaveLength(3);
		expect(folder(m.root, 'Windows 3.x').value).toBe('WIN31');
		expect(folder(m.root, 'DOS').value).toBe('');
		expect(rows[0]!.textContent).toContain('Found at dos/dist/SETUP.EXE');
		expect(rows[2]!.textContent).toContain('Estimated: no runtime was found');
		expect(rows[2]!.textContent).toContain('140,000');
	});

	it('check a folder as it is typed and write a good one, in capitals', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('targets');
		const win = folder(m.root, 'Windows 95');
		await m.type(win, 'WINDOWS95X');
		expect(m.root.querySelector('[role="alert"]')!.textContent).toBe(
			'“WINDOWS95X” is not an 8.3 name.',
		);
		await m.key(win, 'Enter');
		expect(m.media.calls.some((c) => c.startsWith('setMedia'))).toBe(false);
		await m.type(win, 'win31');
		expect(m.root.querySelector('[role="alert"]')!.textContent).toBe(
			'Another target already uses this folder.',
		);
		await m.type(win, 'a/b');
		expect(m.root.querySelector('[role="alert"]')!.textContent).toBe(
			'A folder is one name, with no slash.',
		);
		await m.type(win, 'win32');
		expect(win.value).toBe('WIN32');
		expect(m.root.querySelector('[role="alert"]')).toBeNull();
		await m.key(win, 'Enter');
		expect(m.media.edits().at(-1)).toContain('"dir":"WIN32"');
		expect(folder(m.root, 'Windows 95').value).toBe('WIN32');
	});

	it('take a target off the medium, which frees its files, and put it back in its usual folder', async () => {
		const m = await mountMedia([quillFloppy()]);
		const before = (await m.media.getMediaPlan('floppy')).free;
		await m.tab('targets');
		const box = m.root.querySelector<HTMLInputElement>(
			'input[aria-label="Put Windows 95 on this medium"]',
		)!;
		expect(box.checked).toBe(true);
		await m.click(box);
		expect(folder(m.root, 'Windows 95').disabled).toBe(true);
		expect((await m.media.getMediaPlan('floppy')).free).toBeGreaterThan(before);
		expect((await m.media.history()).entries.at(-1)!.label).toBe(
			'Take Windows 95 off medium floppy',
		);
		await m.click(m.root.querySelector('input[aria-label="Put Windows 95 on this medium"]'));
		expect(folder(m.root, 'Windows 95').value).toBe('WIN95');
		expect((await m.media.getMediaPlan('floppy')).free).toBe(before);
	});
});

describe('the space card', () => {
	it('warns when less than the reserve stays free, and stops when the reserve is lowered', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('sizes');
		const reserve = m.root.querySelector<HTMLInputElement>('#media-reserve')!;
		expect(reserve.value).toBe('16');
		expect(m.root.textContent).not.toContain('Below the reserve');
		await m.type(reserve, '200');
		await m.key(reserve, 'Enter');
		expect(m.media.edits().at(-1)).toBe('setMedia(floppy, {"reserve":204800})');
		expect(m.rows()[0]!.textContent).toContain('Tight');
		expect(m.root.textContent).toContain('Below the reserve');
		expect(m.root.querySelector('[role="meter"]')!.getAttribute('aria-valuetext')).toContain(
			'Below the reserve',
		);
		await m.type(reserve, '0');
		await m.leave(reserve);
		expect(m.root.textContent).not.toContain('Below the reserve');
	});

	it('refuses a reserve that is not a whole number of KiB', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('sizes');
		const reserve = m.root.querySelector<HTMLInputElement>('#media-reserve')!;
		await m.type(reserve, '1.5');
		expect(m.root.textContent).toContain('The reserve is a whole number of KiB.');
		await m.key(reserve, 'Enter');
		expect(m.media.calls.some((c) => c.startsWith('setMedia'))).toBe(false);
	});

	it('continues on a second disk when asked, and then a medium that did not fit does', async () => {
		const m = await mountMedia([{ ...quillFloppy(), exclude: [] }]);
		await m.tab('sizes');
		expect(m.rows()[0]!.textContent).toContain('Does not fit');
		const overflow = m.root.querySelector<HTMLSelectElement>('#media-overflow')!;
		expect(overflow.value).toBe('error');
		await m.act(async () => {
			overflow.value = 'span';
			overflow.dispatchEvent(new Event('change', { bubbles: true }));
		});
		expect(m.media.edits().at(-1)).toBe('setMedia(floppy, {"overflow":"span"})');
		expect(m.rows()[0]!.textContent).toContain('2 disks');
		expect(m.rows()[0]!.textContent).not.toContain('Does not fit');
		await m.tab('map');
		expect(m.root.querySelectorAll('[role="tree"]')).toHaveLength(2);
	});

	it('has no second disk on a CD', async () => {
		const m = await mountMedia([quillCd()]);
		await m.tab('sizes');
		expect(m.root.querySelector<HTMLSelectElement>('#media-overflow')!.disabled).toBe(true);
		expect(m.root.textContent).toContain('never continues on a second disk');
	});
});

describe('the README', () => {
	it('previews the generated text in a 76-column box and checks it against DOS', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('label');
		const box = m.root.querySelector('pre')!;
		expect(box.getAttribute('aria-label')).toBe('The text of README.TXT');
		expect(box.textContent).toContain('QUILL 0.1.0');
		expect(box.textContent).toContain('Windows 95: choose Run and type A:\\WIN95\\SETUP');
		expect(m.root.textContent).toContain('Written from the targets on this medium.');
		expect(m.root.textContent).toContain('Every line fits in 76 columns.');
		expect(m.root.textContent).toContain('Lines end in CR LF, as DOS expects.');
	});

	it('follows the targets: a target taken off leaves the README', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('targets');
		await m.click(m.root.querySelector('input[aria-label="Put Windows 95 on this medium"]'));
		await m.tab('label');
		expect(m.root.querySelector('pre')!.textContent).not.toContain('Windows 95');
	});

	it('uses a file of one’s own, and goes back to the generated text', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('label');
		const generate = () => m.button('Use the generated text')!;
		expect(generate().disabled).toBe(true);
		await m.click(m.button('Use my own file…'));
		expect(m.media.edits().at(-1)).toBe(
			'setMedia(floppy, {"readme":"/project/installer/media/README.TXT"})',
		);
		expect(m.root.textContent).toContain('From the file /project/installer/media/README.TXT.');
		expect(generate().disabled).toBe(false);
		await m.click(generate());
		expect(m.media.edits().at(-1)).toBe('setMedia(floppy, {"readme":""})');
		expect(m.root.textContent).toContain('Written from the targets on this medium.');
	});

	it('does nothing when the file picker is cancelled', async () => {
		const m = await mountMedia([quillFloppy()]);
		m.media.picked = null;
		await m.tab('label');
		await m.click(m.button('Use my own file…'));
		expect(m.media.calls.some((c) => c.startsWith('setMedia'))).toBe(false);
	});
});

describe('the autorun', () => {
	it('is explained and switched off on a floppy', async () => {
		const m = await mountMedia([quillFloppy()]);
		await m.tab('autorun');
		expect(m.root.textContent).toContain('Only a CD has an AUTORUN.INF');
		expect(m.root.querySelector('select')!.disabled).toBe(true);
		expect(m.root.querySelector('pre')).toBeNull();
	});

	it('shows the AUTORUN.INF of a CD and the target it opens, and lets it be changed', async () => {
		const m = await mountMedia([quillCd()]);
		await m.tab('autorun');
		const select = m.root.querySelector<HTMLSelectElement>('select')!;
		expect(select.value).toBe('windows-x86');
		expect(m.root.querySelector('pre')!.textContent).toBe(
			'[autorun]\nopen=WIN95\\SETUP.EXE\nicon=WIN95\\SETUP.EXE,0\n',
		);
		await m.act(async () => {
			select.value = '';
			select.dispatchEvent(new Event('change', { bubbles: true }));
		});
		expect(m.media.edits().at(-1)).toBe('setMedia(cd, {"autorun":""})');
		expect(m.root.querySelector('pre')).toBeNull();
	});

	it('drops the autorun with the target it opened, in the same undo step', async () => {
		const m = await mountMedia([quillCd()]);
		await m.tab('targets');
		await m.click(m.root.querySelector('input[aria-label="Put Windows 95 on this medium"]'));
		expect(m.media.edits().at(-1)).toContain('"autorun":""');
		await m.tab('autorun');
		expect(m.root.textContent).toContain('Put a Windows 95 target on this medium');
		expect(m.root.querySelector('select')!.disabled).toBe(true);
		await m.act(() => m.app.dispatch('edit.undo'));
		expect(m.root.querySelector('select')!.value).toBe('windows-x86');
	});

	it('warns about an autorun that names a target the medium does not have, and the warning opens the tab', async () => {
		const m = await mountMedia([{ ...quillCd(), autorun: 'windows-x86_64' }]);
		expect(m.root.textContent).toContain('autorun names windows-x86_64');
		await m.click(m.button('Open Autorun'));
		expect(m.root.querySelector('#media-tab-autorun')!.getAttribute('aria-selected')).toBe('true');
		expect(document.activeElement).toBe(m.root.querySelector('#media-tab-autorun'));
	});
});

describe('the warnings', () => {
	it('lead to the tab that fixes them, and the note on 4 KiB chunks leads nowhere', async () => {
		const m = await mountMedia([{ ...quillFloppy(), reserve: 400_000 }]);
		const titles = [...m.root.querySelectorAll('ul li strong')].map((x) => x.textContent);
		expect(titles).toContain('Windows 3.x chunk size');
		expect(titles).toContain('Runtime estimated');
		expect(titles).toContain('Below the reserve');
		const item = (title: string) =>
			[...m.root.querySelectorAll('ul li')].find(
				(li) => li.querySelector('strong')?.textContent === title,
			)!;
		expect(item('Windows 3.x chunk size').querySelector('button')).toBeNull();
		expect(item('Windows 3.x chunk size').textContent).toMatch(
			/costs about [\d,]+ bytes against 32 KiB/,
		);
		await m.click(item('Runtime estimated').querySelector('button'));
		expect(m.root.querySelector('#media-tab-targets')!.getAttribute('aria-selected')).toBe('true');
		await m.tab('map');
		await m.click(item('Below the reserve').querySelector('button'));
		expect(m.root.querySelector('#media-tab-sizes')!.getAttribute('aria-selected')).toBe('true');
	});

	it('shows a bad label as an error that opens the label tab', async () => {
		const m = await mountMedia([{ ...quillFloppy(), label: 'quill release' }]);
		const li = [...m.root.querySelectorAll('ul li')].find((x) => x.textContent!.includes('Label'))!;
		expect(li.className).toContain('error');
		await m.click(li.querySelector('button'));
		expect(m.root.querySelector('#media-tab-label')!.getAttribute('aria-selected')).toBe('true');
	});

	it('keeps the tab and the picked row when the medium is edited', async () => {
		const m = await mountMedia([quillFloppy(), quillCd()]);
		await m.click(m.rows()[1]);
		await m.tab('sizes');
		await m.click(m.root.querySelector('[role="switch"]'));
		expect(m.rows()[1]!.getAttribute('aria-selected')).toBe('true');
		expect(m.root.querySelector('#media-tab-sizes')!.getAttribute('aria-selected')).toBe('true');
	});
});
