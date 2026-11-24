// Tests the edits the Media screen sends and the README's check against DOS.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { checkReadme, dirChange, targetChange, toggleChange, warningTab } from './mediaEdit';
import { generatedReadme, planFromFixture, quillCd, quillFloppy } from './mediaFixture';
import { MediaWarningCodes } from './mediaWarnings';

describe('what a switch sends', () => {
	it('leaves a component off, and puts it back, in the history’s words', () => {
		const info = quillFloppy();
		const plan = planFromFixture(info);
		const dict = plan.components.find((c) => c.id === 'dicts')!;
		const off = toggleChange(info, dict, false);
		expect(off.edit).toEqual({ exclude: ['dicts-more', 'dicts'] });
		expect(off.label).toBe('Leave Canadian dictionary off medium floppy');
		const more = plan.components.find((c) => c.id === 'dicts-more')!;
		const on = toggleChange(info, more, true);
		expect(on.edit).toEqual({ exclude: [] });
		expect(on.label).toBe('Put More dictionaries on medium floppy');
	});
});

describe('what a target sends', () => {
	it('puts a target back in its usual folder and keeps the plan’s order', () => {
		const info = {
			...quillFloppy(),
			targets: quillFloppy().targets.filter((t) => t.target !== 'win16-x86'),
		};
		const plan = planFromFixture(info);
		const change = targetChange(info, plan, 'win16-x86', 'Windows 3.x', true);
		expect(change.edit.targets!.map((t) => [t.target, t.dir])).toEqual([
			['dos-x86', ''],
			['win16-x86', 'WIN31'],
			['windows-x86', 'WIN95'],
		]);
		expect(change.label).toBe('Put Windows 3.x on medium floppy');
		expect(
			targetChange(info, plan, 'dos-x86', 'DOS', false).edit.targets!.map((t) => t.target),
		).toEqual(['windows-x86']);
	});

	it('drops the autorun that opened a target when the target comes off', () => {
		const info = quillCd();
		const plan = planFromFixture(info);
		expect(targetChange(info, plan, 'windows-x86', 'Windows 95', false).edit.autorun).toBe('');
		expect(targetChange(info, plan, 'dos-x86', 'DOS', false).edit.autorun).toBeUndefined();
	});

	it('moves one target and leaves the others', () => {
		const change = dirChange(quillFloppy(), 'windows-x86', 'Windows 95', 'WIN32');
		expect(change.edit.targets!.map((t) => t.dir)).toEqual(['', 'WIN31', 'WIN32']);
		expect(change.label).toBe('Set folder of Windows 95 on medium floppy');
	});
});

describe('the tab a warning opens', () => {
	it('names one for every code but the chunk-size note', () => {
		const tabs = Object.fromEntries(MediaWarningCodes.map((c) => [c, warningTab(c)]));
		expect(tabs).toEqual({
			does_not_fit: 'sizes',
			below_reserve: 'sizes',
			default_on_later_disk: 'sizes',
			win16_window: null,
			label: 'label',
			dir_not_8_3: 'targets',
			autorun_target: 'autorun',
			runtime_estimated: 'targets',
			runtime_lacks: 'targets',
			name_83: 'map',
		});
	});
});

describe('the README check', () => {
	it('passes the generated text: every line within 76 columns and ending CR LF', () => {
		expect(checkReadme(generatedReadme(quillFloppy()))).toEqual({ tooWide: 0, lfOnly: false });
		expect(checkReadme(generatedReadme(quillCd()))).toEqual({ tooWide: 0, lfOnly: false });
	});

	it('counts the lines wider than 76 columns and spots a line that ends in LF alone', () => {
		const wide = 'x'.repeat(77);
		expect(checkReadme(`short\r\n${wide}\r\n${wide}\r\n`)).toEqual({ tooWide: 2, lfOnly: false });
		expect(checkReadme('x'.repeat(76) + '\r\n').tooWide).toBe(0);
		expect(checkReadme('one\r\ntwo\n').lfOnly).toBe(true);
		expect(checkReadme('one\ntwo\n').lfOnly).toBe(true);
	});
});
