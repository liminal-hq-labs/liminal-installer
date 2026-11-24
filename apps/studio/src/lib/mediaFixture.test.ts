// Checks that the fixture's stand-in planner lands on the totals the Quill media plan gives.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { planFromFixture, quillCd, quillFloppy, quillSet, serialFromVersion } from './mediaFixture';

describe('the fixture planner', () => {
	it('puts the Quill floppy at the plan’s 1,286,656 bytes used and 171,008 free', () => {
		const plan = planFromFixture(quillFloppy());
		expect(plan.capacity).toBe(1_457_664);
		expect(plan.used).toBe(1_286_656);
		expect(plan.free).toBe(171_008);
		expect(plan.volumes).toHaveLength(1);
		expect(plan.volumes[0]!.used + plan.volumes[0]!.free).toBe(plan.capacity);
	});

	it('needs the plan’s 1,735,168 bytes without sharing, which is 277,504 over', () => {
		const plan = planFromFixture(quillFloppy());
		expect(plan.without_sharing).toBe(1_735_168);
		expect(plan.without_sharing - plan.capacity).toBe(277_504);
	});

	it('rounds every file up to whole clusters', () => {
		const all = (es: ReturnType<typeof planFromFixture>['volumes'][number]['root']): number[] =>
			es.flatMap((e) => [e.on_disk, ...all(e.children)]);
		for (const kind of [quillFloppy(), quillCd()]) {
			const plan = planFromFixture(kind);
			for (const n of all(plan.volumes[0]!.root)) expect(n % plan.cluster).toBe(0);
		}
	});

	it('lists the shared files as C0000001 and on in a common pack at 4 KiB for every target', () => {
		const plan = planFromFixture(quillFloppy());
		const common = plan.volumes[0]!.root.find((e) => e.name === 'COMMON.LPK')!;
		expect(common.pack).toMatchObject({ which: 'common', chunk: 4096 });
		expect(common.entries[0]).toMatchObject({ stored: 'C0000001', name: 'Quill.hlp' });
		expect(common.entries.every((e) => e.targets.length === 3)).toBe(true);
		expect(new Set(common.entries.map((e) => e.stored)).size).toBe(common.entries.length);
	});

	it('leaves a switch off by default for the second dictionaries and says it would not fit', () => {
		const more = planFromFixture(quillFloppy()).components.find((c) => c.id === 'dicts-more')!;
		expect(more.on_medium).toBe(false);
		expect(more.volume).toBe(0);
		expect(more.delta_if_toggled).toBeLessThan(-400_000);
		expect(more.fits_if_toggled).toBe(false);
	});

	it('says what leaving the Canadian dictionary off frees, in whole clusters', () => {
		const dict = planFromFixture(quillFloppy()).components.find((c) => c.id === 'dicts')!;
		expect(dict.on_medium).toBe(true);
		expect(dict.delta_if_toggled).toBeGreaterThan(210_000);
		expect(dict.delta_if_toggled % 512).toBe(0);
		expect(dict.fits_if_toggled).toBe(true);
	});

	it('makes a delta the real difference between two plans', () => {
		const info = quillFloppy();
		const before = planFromFixture(info);
		const dict = before.components.find((c) => c.id === 'dicts')!;
		const after = planFromFixture({ ...info, exclude: [...info.exclude, 'dicts'] });
		expect(after.free - before.free).toBe(dict.delta_if_toggled);
	});

	it('does not fit with every dictionary, and names the overrun and the largest files', () => {
		const plan = planFromFixture({ ...quillFloppy(), exclude: [] });
		expect(plan.free).toBeLessThan(0);
		const warning = plan.warnings.find((w) => w.code === 'does_not_fit')!;
		expect(warning.severity).toBe('error');
		expect(warning.message).toMatch(/QUILL32.EXE \(216,035\)/);
	});

	it('warns that 4 KiB chunks cost bytes against 32 KiB, and that the Windows 95 runtime is estimated', () => {
		const codes = planFromFixture(quillFloppy()).warnings.map((w) => w.code);
		expect(codes).toContain('win16_window');
		expect(codes).toContain('runtime_estimated');
		expect(codes).not.toContain('does_not_fit');
	});

	it('warns below the reserve and stops warning when the reserve is lowered', () => {
		const tight = planFromFixture({ ...quillFloppy(), reserve: 200_000 });
		expect(tight.warnings.some((w) => w.code === 'below_reserve')).toBe(true);
		const fine = planFromFixture({ ...quillFloppy(), reserve: 0 });
		expect(fine.warnings.some((w) => w.code === 'below_reserve')).toBe(false);
	});

	it('spans: puts the second dictionaries on disk 2, in COMMON.L02, with a label of its own', () => {
		const plan = planFromFixture(quillSet());
		expect(plan.volumes).toHaveLength(2);
		expect(plan.volumes[1]!.label).toBe('QUILL 2');
		expect(plan.volumes[1]!.root.map((e) => e.name)).toEqual(['COMMON.L02']);
		expect(plan.components.find((c) => c.id === 'dicts-more')!.volume).toBe(2);
		expect(plan.components.find((c) => c.id === 'dicts')!.volume).toBe(1);
		expect(plan.warnings.some((w) => w.code === 'does_not_fit')).toBe(false);
		expect(plan.warnings.some((w) => w.code === 'runtime_lacks')).toBe(true);
		const stored = plan.volumes.flatMap((v) =>
			v.root.flatMap((e) => e.entries.map((x) => x.stored)),
		);
		expect(new Set(stored).size).toBe(stored.length);
	});

	it('puts everything on a CD with its autorun text and a 2,048-byte sector', () => {
		const plan = planFromFixture(quillCd());
		expect(plan.cluster).toBe(2048);
		expect(plan.capacity).toBe(681_574_400);
		expect(plan.components.every((c) => c.on_medium)).toBe(true);
		expect(plan.autorun_text).toBe(
			'[autorun]\r\nopen=WIN95\\SETUP.EXE\r\nicon=WIN95\\SETUP.EXE,0\r\n',
		);
		expect(plan.free).toBeGreaterThan(600_000_000);
	});

	it('works the automatic serial out of the version', () => {
		expect(serialFromVersion('0.1.0')).toBe('0001-0000');
		expect(planFromFixture(quillFloppy()).serial).toBe('0001-0000');
		expect(planFromFixture({ ...quillFloppy(), serial: '4c49-5253' }).serial).toBe('4C49-5253');
	});

	it('reports a bad label and folder as errors the way li-media’s validate does', () => {
		const info = quillFloppy();
		info.label = 'quill release';
		info.targets[1]!.dir = 'WINDOWS31';
		const codes = planFromFixture(info).warnings.map((w) => w.code);
		expect(codes).toContain('label');
		expect(codes).toContain('dir_not_8_3');
	});
});
