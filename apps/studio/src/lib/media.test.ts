// Tests the Media screen's pure helpers: the label, serial and folder rules, the meter's cells and the tree's rows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	checkDir,
	checkLabel,
	checkSerial,
	defaultLabel,
	formatBytes,
	formatSerial,
	isoVolumeId,
	jolietLabel,
	meterCells,
	meterParts,
	nextMediaId,
	sortedBy,
	visibleRows,
	volumeKey,
} from './media';
import { planFromFixture, quillFloppy, quillSet } from './mediaFixture';

describe('the label rule', () => {
	it('allows 11 capital letters, digits, spaces and the FAT punctuation', () => {
		expect(checkLabel('QUILL', 'floppy-1440')).toBeNull();
		expect(checkLabel('ABCDEFGHIJK', 'floppy-1440')).toBeNull();
		expect(checkLabel('', 'floppy-1440')).toBeNull();
		expect(checkLabel("A_B-C!#$%&'", 'floppy-1440')).toBeNull();
		expect(checkLabel('()@^{}~ 1', 'floppy-1440')).toBeNull();
	});

	it('says how long a label is when it is too long, and how long a CD’s may be', () => {
		expect(checkLabel('ABCDEFGHIJKL', 'floppy-1440')).toEqual({
			kind: 'tooLong',
			length: 12,
			max: 11,
		});
		expect(checkLabel('ABCDEFGHIJKL', 'cdrom')).toBeNull();
		expect(checkLabel('A'.repeat(17), 'cdrom')).toEqual({ kind: 'tooLong', length: 17, max: 16 });
	});

	it('names the first character a label may not hold', () => {
		expect(checkLabel('quill', 'floppy-1440')).toEqual({ kind: 'badChar', char: 'q' });
		for (const ch of [
			'.',
			'*',
			'?',
			'/',
			'\\',
			':',
			';',
			'|',
			'=',
			',',
			'"',
			'<',
			'>',
			'[',
			']',
			'+',
		])
			expect(checkLabel(`A${ch}B`, 'floppy-1440')).toEqual({ kind: 'badChar', char: ch });
	});
});

describe('the serial', () => {
	it('is auto, or eight hex digits with an optional dash', () => {
		expect(checkSerial('auto')).toBe('auto');
		expect(checkSerial('AUTO')).toBe('auto');
		expect(checkSerial('4C495253')).toBe(0x4c495253);
		expect(checkSerial('4c49-5253')).toBe(0x4c495253);
		for (const bad of ['', '4C49525', '4C4952530', '4C49-525', '4C49_5253', 'ZZZZZZZZ'])
			expect(checkSerial(bad)).toBeNull();
	});

	it('is shown as XXXX-XXXX and reads back', () => {
		expect(formatSerial(0x4c495253)).toBe('4C49-5253');
		expect(formatSerial(0xab)).toBe('0000-00AB');
		expect(checkSerial(formatSerial(0xdeadbeef))).toBe(0xdeadbeef);
	});
});

describe('the folder rule', () => {
	it('is empty or one 8.3 name', () => {
		for (const ok of ['', 'WIN31', 'WIN95', 'A', 'ABCDEFGH', 'DOS.X86', '_1~'])
			expect(checkDir(ok)).toBeNull();
		for (const bad of ['ABCDEFGHI', 'win31', 'A.BCDE', 'A.B.C', '.EXT', 'A B'])
			expect(checkDir(bad)).toEqual({ kind: 'not83' });
		expect(checkDir('A/B')).toEqual({ kind: 'notOneFolder' });
		expect(checkDir('A\\B')).toEqual({ kind: 'notOneFolder' });
	});
});

describe('the names of a medium', () => {
	it('cuts the slug to a label the kind allows', () => {
		expect(defaultLabel('quill-installer-experiment', 'floppy-1440')).toBe('QUILLINSTAL');
		expect(defaultLabel('quill-installer-experiment', 'cdrom')).toBe('QUILLINSTALLEREX');
	});

	it('finds the next free id', () => {
		const m = quillFloppy();
		expect(nextMediaId('floppy-1440', [])).toBe('floppy');
		expect(nextMediaId('floppy-1440', [m])).toBe('floppy-2');
		expect(nextMediaId('floppy-1440', [m, { ...m, id: 'floppy-2' }])).toBe('floppy-3');
		expect(nextMediaId('cdrom', [m])).toBe('cd');
	});

	it('derives a CD’s ISO volume id and Joliet label from the label', () => {
		expect(isoVolumeId('Quill 1.0')).toBe('QUILL_1_0');
		expect(isoVolumeId('A'.repeat(40))).toHaveLength(32);
		expect(jolietLabel('ABCDEFGHIJKLMNOPQRS')).toBe('ABCDEFGHIJKLMNOP');
	});

	it('groups digits for the locale', () => {
		expect(formatBytes(1286656)).toBe('1,286,656');
	});
});

describe('the meter’s cells', () => {
	it('always add up to the count, and the floppy’s 16 KiB cells make 89', () => {
		const volume = planFromFixture(quillFloppy()).volumes[0]!;
		const parts = meterParts(volume, 16384);
		expect(parts.own + parts.common + parts.reserve + parts.free).toBe(volume.capacity);
		const cells = meterCells(parts, 89);
		expect(cells.own + cells.common + cells.reserve + cells.free).toBe(89);
		expect(cells.common).toBeGreaterThan(0);
		expect(cells.reserve).toBe(1);
	});

	it('gives a part with any bytes at least one cell, even a tiny reserve', () => {
		const cells = meterCells(
			{ own: 1_000_000, common: 0, reserve: 1, free: 1_000_000, over: 0 },
			89,
		);
		expect(cells.reserve).toBe(1);
		expect(cells.own + cells.reserve + cells.free).toBe(89);
	});

	it('draws a disk with nothing on it as all free, and an over-full disk as all used', () => {
		expect(meterCells({ own: 0, common: 0, reserve: 0, free: 0, over: 0 }, 10).free).toBe(10);
		const full = planFromFixture({ ...quillFloppy(), exclude: [] }).volumes[0]!;
		const parts = meterParts(full, 16384);
		expect(parts.over).toBeGreaterThan(0);
		expect(parts.free + parts.reserve).toBe(0);
		const cells = meterCells(parts, 89);
		expect(cells.own + cells.common).toBe(89);
	});
});

describe('the disk map’s rows', () => {
	it('shows the disk, its folders open, and a pack’s files only once the pack is open', () => {
		const volume = planFromFixture(quillFloppy()).volumes[0]!;
		const open = new Set(['1', '1/WIN31', '1/WIN95']);
		const rows = visibleRows(volume, open);
		expect(rows.map((r) => r.key)).toEqual([
			'1',
			'1/WIN31',
			'1/WIN31/SETUP.EXE',
			'1/WIN31/SETUP.LPK',
			'1/WIN95',
			'1/WIN95/SETUP.EXE',
			'1/WIN95/SETUP.LPK',
			'1/SETUP.EXE',
			'1/SETUP.LPK',
			'1/COMMON.LPK',
			'1/README.TXT',
		]);
		expect(rows.map((r) => r.level)).toEqual([1, 2, 3, 3, 2, 3, 3, 2, 2, 2, 2]);
		const withPack = visibleRows(volume, new Set([...open, '1/COMMON.LPK']));
		const entries = withPack.filter((r) => r.kind === 'pack');
		expect(entries[0]).toMatchObject({ level: 3, parent: '1/COMMON.LPK' });
		expect(entries.map((r) => r.packEntry!.stored)[0]).toBe('C0000001');
		expect(visibleRows(volume, new Set())).toHaveLength(1);
		expect(volumeKey(volume)).toBe('1');
	});

	it('numbers the disks of a set apart so their keys never meet', () => {
		const keys = planFromFixture(quillSet()).volumes.map((v) => volumeKey(v));
		expect(keys).toEqual(['1', '2']);
	});
});

describe('sorting', () => {
	it('sorts text and numbers, either way, and keeps ties in order', () => {
		const rows = [
			{ n: 'b', v: 2 },
			{ n: 'a', v: 2 },
			{ n: 'c', v: 1 },
		];
		expect(sortedBy(rows, (r) => r.v, 'ascending').map((r) => r.n)).toEqual(['c', 'b', 'a']);
		expect(sortedBy(rows, (r) => r.v, 'descending').map((r) => r.n)).toEqual(['b', 'a', 'c']);
		expect(sortedBy(rows, (r) => r.n, 'ascending').map((r) => r.n)).toEqual(['a', 'b', 'c']);
		expect(rows.map((r) => r.n)).toEqual(['b', 'a', 'c']);
	});
});
