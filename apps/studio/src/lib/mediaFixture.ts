// The Quill payload as the Media screen's stand-in planner sees it, and the plan it works out for a medium.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// The sizes are the Quill media plan's: the program, help and dictionary files and their packed sizes at
// 4 KiB and 32 KiB windows are measured there, and the layout rounds each file to a 512-byte cluster. The
// per-pack overhead (80 bytes plus 48 per file and 8 per chunk), the `SETUP.PRJ` budgets and the split of the
// sample files are this file's own, chosen so the three totals the plan gives come out exactly:
//
//   all targets, `COMMON.LPK` at 4 KiB, `EN_CA` only ... 1,286,656 bytes used, 171,008 free
//   the same with no shared pack .......................... 1,735,168 bytes
//   the same with `EN_US` and `EN_GB` ..................... does not fit
//
// It stands in for the planner in `li-media` (D83) until that exists, so the screen, its tests and the
// demo run on numbers that behave like the real ones: switching a component recomputes everything from the
// files, so a switch's "frees N" is a real difference between two plans.

import { checkDir, checkLabel, checkSerial, formatSerial, formatBytes, labelMax } from './media';
import type {
	ComponentSize,
	DiskEntry,
	FileSize,
	MediaInfo,
	MediaKind,
	MediaPlan,
	MediaTargetPlan,
	MediaWarning,
	PackEntryPlan,
	PackWhich,
	VolumePlan,
} from './types';

/** The bytes a 1.44 MB floppy has for data, and a 650 MB disc. */
export const CAPACITY: Record<MediaKind, number> = {
	'floppy-1440': 1_457_664,
	cdrom: 681_574_400,
};
/** The bytes of a floppy cluster and a disc sector. */
export const CLUSTER: Record<MediaKind, number> = { 'floppy-1440': 512, cdrom: 2048 };

/** The product the fixture describes, and the version its automatic serial comes from. */
export const FIXTURE_PRODUCT = { slug: 'quill', version: '0.1.0' } as const;

const WINDOW_4K = 4096;
const WINDOW_32K = 32768;
const PACK_HEADER = 80;
const PACK_PER_FILE = 48;
const PACK_PER_CHUNK = 8;
const DEFAULT_RESERVE = 16 * 1024;

interface FixtureTarget {
	name: string;
	/** The folder a new medium puts the target in. */
	dir: string;
	/** The size of `SETUP.EXE`, found or budgeted. */
	setup: number;
	/** Where `SETUP.EXE` was found; absent when only its budget is known. */
	found?: string;
	exe: { name: string; raw: number; packed: number };
	/** The window of the target's own pack. */
	window: number;
	/** The budget for `SETUP.PRJ`. */
	project: number;
}

/** The retro targets of the project, in the order a new medium lists them. */
export const FIXTURE_TARGETS: Record<string, FixtureTarget> = {
	'dos-x86': {
		name: 'DOS',
		dir: '',
		setup: 165_000,
		found: 'dos/dist/SETUP.EXE',
		exe: { name: 'QUILLDOS.EXE', raw: 196_608, packed: 112_885 },
		window: WINDOW_32K,
		project: 12_000,
	},
	'win16-x86': {
		name: 'Windows 3.x',
		dir: 'WIN31',
		setup: 206_179,
		found: 'retro/win16/out/SETUP.EXE',
		exe: { name: 'QUILL.EXE', raw: 326_528, packed: 157_507 },
		window: WINDOW_4K,
		project: 16_000,
	},
	'windows-x86': {
		name: 'Windows 95',
		dir: 'WIN95',
		setup: 140_000,
		exe: { name: 'QUILL32.EXE', raw: 361_472, packed: 216_035 },
		window: WINDOW_32K,
		project: 16_000,
	},
};

interface FixtureFile {
	name: string;
	raw: number;
	/** Packed at a 4 KiB window and at a 32 KiB one. */
	p4: number;
	p32: number;
}

interface FixtureComponent {
	id: string;
	name: string;
	required: boolean;
	/** Ticked by default in the installer, so leaving it for a later disk is worth a warning. */
	defaultOn: boolean;
	files: FixtureFile[];
}

/** The components whose files may be shared. The program is each target's own and is not listed here. */
export const FIXTURE_COMPONENTS: FixtureComponent[] = [
	{
		id: 'help',
		name: 'Help file',
		required: true,
		defaultOn: true,
		files: [{ name: 'QUILL.HLP', raw: 6_518, p4: 2_152, p32: 1_980 }],
	},
	{
		id: 'samples',
		name: 'Sample documents',
		required: false,
		defaultOn: true,
		files: [
			{ name: 'SAMPLE1.QLD', raw: 18_000, p4: 8_100, p32: 7_900 },
			{ name: 'SAMPLE2.QLD', raw: 12_400, p4: 5_600, p32: 5_450 },
			{ name: 'SAMPLE3.QLD', raw: 9_800, p4: 4_300, p32: 4_180 },
			{ name: 'SAMPLE4.QLD', raw: 7_100, p4: 3_300, p32: 3_200 },
			{ name: 'README.TXT', raw: 4_300, p4: 2_100, p32: 2_050 },
			{ name: 'LICENCE.TXT', raw: 3_100, p4: 1_628, p32: 1_600 },
		],
	},
	{
		id: 'dicts',
		name: 'Canadian dictionary',
		required: false,
		defaultOn: true,
		files: [{ name: 'EN_CA.DIC', raw: 300_086, p4: 210_547, p32: 197_518 }],
	},
	{
		id: 'dicts-more',
		name: 'More dictionaries',
		required: false,
		defaultOn: false,
		files: [
			{ name: 'EN_US.DIC', raw: 298_454, p4: 209_556, p32: 197_000 },
			{ name: 'EN_GB.DIC', raw: 298_270, p4: 209_261, p32: 196_900 },
		],
	},
];

const ceilTo = (n: number, unit: number) => Math.ceil(n / unit) * unit;
const chunksOf = (raw: number, window: number) => Math.ceil(raw / window);
const overhead = (files: number, chunks: number) =>
	PACK_HEADER + PACK_PER_FILE * files + PACK_PER_CHUNK * chunks;

/** The label of the second disk of a floppy set: `QUILL 2`, cut to fit. */
export const secondDiskLabel = (label: string): string =>
	`${label.slice(0, labelMax('floppy-1440') - 2)} 2`;

/** An automatic serial worked out from a version, as `XXXX-XXXX`. The demo's rule, not the engine's. */
export function serialFromVersion(version: string): string {
	const [major = 0, minor = 0, patch = 0] = version.split('.').map((n) => Number(n) || 0);
	return formatSerial((((major & 0xff) << 24) | ((minor & 0xff) << 16) | (patch & 0xffff)) >>> 0);
}

/** A new floppy medium with every retro target, as Add floppy makes it. */
export function newMedia(kind: MediaKind, id: string, label: string): MediaInfo {
	return {
		id,
		kind,
		label,
		serial: 'auto',
		common: 'auto',
		exclude: kind === 'cdrom' ? [] : ['dicts-more'],
		overflow: 'error',
		reserve: DEFAULT_RESERVE,
		extra: [],
		...(kind === 'cdrom' ? { autorun: 'windows-x86' } : {}),
		targets: Object.entries(FIXTURE_TARGETS).map(([target, t]) => ({ target, dir: t.dir })),
	};
}

/** The floppy of the Quill plan: all three targets, one dictionary, a shared pack. */
export const quillFloppy = (): MediaInfo => newMedia('floppy-1440', 'floppy', 'QUILL');
/** The CD of the Quill plan: everything, with an `AUTORUN.INF` for Windows 95. */
export const quillCd = (): MediaInfo => newMedia('cdrom', 'cd', 'QUILL');
/** A two-disk floppy set: every dictionary, the later ones on disk 2. */
export const quillSet = (): MediaInfo => ({
	...newMedia('floppy-1440', 'set', 'QUILL'),
	exclude: [],
	overflow: 'span',
});

interface Placed {
	component: FixtureComponent;
	volume: number;
}

interface Layout {
	volumes: VolumePlan[];
	files: FileSize[];
	components: {
		id: string;
		raw: number;
		packed: number;
		pack: PackWhich;
		volume: number;
		users: string[];
	}[];
	used: number;
	capacity: number;
	cluster: number;
}

const upper = (s: string) => s.toUpperCase();
/** Where a file is installed: 8.3 names on DOS and Windows 3.x, the long name on Windows 95. */
const destOf = (target: string, name: string): string => {
	if (target !== 'windows-x86') return upper(name);
	const [stem = '', ext = ''] = name.split('.');
	const long = stem.charAt(0) + stem.slice(1).toLowerCase();
	return ext ? `${long}.${ext.toLowerCase()}` : long;
};

function layoutOf(info: MediaInfo, shared: boolean): Layout {
	const cluster = CLUSTER[info.kind];
	const capacity = CAPACITY[info.kind];
	const onMedium = info.targets.filter((t) => t.target in FIXTURE_TARGETS);
	const ids = onMedium.map((t) => t.target);
	const reserve = info.reserve;
	const isOn = (c: FixtureComponent) => c.required || !info.exclude.includes(c.id);

	// Which components go in the shared pack: those two or more targets on the medium install.
	const used = FIXTURE_COMPONENTS.filter(isOn);
	const sharing = shared && info.common === 'auto' && ids.length >= 2;
	const commonComponents = sharing ? used : [];
	const sharedWindow = ids.some((t) => FIXTURE_TARGETS[t]!.window === WINDOW_4K)
		? WINDOW_4K
		: WINDOW_32K;
	const packedAt = (f: FixtureFile, window: number) => (window === WINDOW_4K ? f.p4 : f.p32);

	// Place the shared pack's components on disks, whole, in order: required, ticked by default, then the rest.
	const order = [...commonComponents].sort(
		(a, b) =>
			Number(!a.required) - Number(!b.required) || Number(!a.defaultOn) - Number(!b.defaultOn),
	);
	const packCost = (list: FixtureComponent[]): number => {
		const files = list.flatMap((c) => c.files);
		const chunks = files.reduce((n, f) => n + chunksOf(f.raw, sharedWindow), 0);
		return files.length === 0
			? 0
			: ceilTo(
					files.reduce((n, f) => n + packedAt(f, sharedWindow), 0) + overhead(files.length, chunks),
					cluster,
				);
	};

	// What every disk carries beside the shared pack.
	const readmeRaw = 1_900;
	const autorunRaw = 80;
	const folders = new Set(onMedium.map((t) => t.dir).filter((d) => d !== ''));
	const extras = info.extra.map((p) => ({ name: p.split('/').pop() ?? p, raw: 2_048 }));
	const ownPacks = onMedium.map((t) => {
		const meta = FIXTURE_TARGETS[t.target]!;
		const carried = sharing ? [] : used;
		const carriedFiles = carried.flatMap((c) => c.files);
		const chunks =
			chunksOf(meta.exe.raw, meta.window) +
			carriedFiles.reduce((n, f) => n + chunksOf(f.raw, meta.window), 0);
		const raw =
			meta.exe.packed +
			meta.project +
			carriedFiles.reduce((n, f) => n + packedAt(f, meta.window), 0) +
			overhead(2 + carriedFiles.length, chunks);
		return { target: t, meta, carried, raw };
	});
	const fixedBytes =
		ownPacks.reduce((n, p) => n + ceilTo(p.meta.setup, cluster) + ceilTo(p.raw, cluster), 0) +
		ceilTo(readmeRaw, cluster) +
		(info.kind === 'cdrom' && info.autorun ? ceilTo(autorunRaw, cluster) : 0) +
		extras.reduce((n, e) => n + ceilTo(e.raw, cluster), 0) +
		folders.size * cluster;

	// Greedy placement of whole components: the first that does not fit starts disk 2, and so does everything after it.
	const placed: Placed[] = [];
	let first: FixtureComponent[] = order;
	let second: FixtureComponent[] = [];
	if (info.overflow === 'span' && info.kind === 'floppy-1440') {
		const limit = capacity - Math.max(0, reserve);
		let take = order.length;
		while (take > 0 && fixedBytes + packCost(order.slice(0, take)) > limit) take--;
		first = order.slice(0, take);
		second = order.slice(take);
	}
	for (const c of first) placed.push({ component: c, volume: 1 });
	for (const c of second) placed.push({ component: c, volume: 2 });

	const entryFor = (target: string, f: FixtureFile) => ({
		target,
		path: destOf(target, f.name),
	});
	/** Stored names count on across the disks of a set, so each is unique. */
	const stored = new Map<FixtureFile, string>();
	for (const f of order.flatMap((c) => c.files))
		stored.set(f, `C${String(stored.size + 1).padStart(7, '0')}`);
	const packEntries = (list: FixtureComponent[]): PackEntryPlan[] =>
		list.flatMap((c) =>
			c.files.map((f) => ({
				stored: stored.get(f)!,
				name: destOf('windows-x86', f.name),
				dest: ids.map((t) => entryFor(t, f)),
				raw: f.raw,
				packed: packedAt(f, sharedWindow),
				targets: ids,
				component: c.id,
			})),
		);

	const entryOf = (
		partial: Partial<DiskEntry> & Pick<DiskEntry, 'name' | 'role' | 'raw'>,
	): DiskEntry => ({
		on_disk: ceilTo(partial.raw, cluster),
		targets: [],
		entries: [],
		children: [],
		...partial,
	});

	const volume = (number: number): VolumePlan => ({
		number,
		label: number === 1 ? info.label : secondDiskLabel(info.label),
		capacity,
		used: 0,
		free: capacity,
		common: 0,
		root: [],
	});
	const volumes = [volume(1)];
	if (second.length > 0) volumes.push(volume(2));
	const v1 = volumes[0]!;

	const root = v1.root;
	const folderEntry = new Map<string, DiskEntry>();
	for (const dir of [...folders]) {
		const f = entryOf({ name: dir, role: 'folder', raw: cluster });
		folderEntry.set(dir, f);
		root.push(f);
	}
	const home = (dir: string): DiskEntry[] => (dir === '' ? root : folderEntry.get(dir)!.children);

	const filesOut: FileSize[] = [];
	for (const p of ownPacks) {
		const exeRaw = p.meta.setup;
		home(p.target.dir).push(
			entryOf({
				name: 'SETUP.EXE',
				role: 'setup',
				raw: exeRaw,
				targets: [p.target.target],
			}),
			entryOf({
				name: 'SETUP.LPK',
				role: 'own_pack',
				raw: p.raw,
				targets: [p.target.target],
				pack: { which: 'own', chunk: p.meta.window },
				entries: [
					{
						stored: 'PROJECT',
						name: 'SETUP.PRJ',
						dest: [],
						raw: p.meta.project,
						packed: p.meta.project,
						targets: [p.target.target],
						component: 'program',
					},
					{
						stored: 'PROGRAM',
						name: p.meta.exe.name,
						dest: [{ target: p.target.target, path: p.meta.exe.name }],
						raw: p.meta.exe.raw,
						packed: p.meta.exe.packed,
						targets: [p.target.target],
						component: 'program',
					},
					...p.carried.flatMap((c) =>
						c.files.map((f) => ({
							stored: f.name,
							name: destOf(p.target.target, f.name),
							dest: [entryFor(p.target.target, f)],
							raw: f.raw,
							packed: packedAt(f, p.meta.window),
							targets: [p.target.target],
							component: c.id,
						})),
					),
				],
			}),
		);
		filesOut.push({
			path: `${p.meta.exe.name}`,
			component: 'program',
			raw: p.meta.exe.raw,
			packed: p.meta.exe.packed,
			pack: 'own',
			targets: [p.target.target],
			volume: 1,
		});
		for (const c of p.carried)
			for (const f of c.files)
				filesOut.push({
					path: destOf(p.target.target, f.name),
					component: c.id,
					raw: f.raw,
					packed: packedAt(f, p.meta.window),
					pack: 'own',
					targets: [p.target.target],
					volume: 1,
				});
	}

	const addCommon = (vol: VolumePlan, list: FixtureComponent[], name: string) => {
		if (list.length === 0) return;
		const files = list.flatMap((c) => c.files);
		const chunks = files.reduce((n, f) => n + chunksOf(f.raw, sharedWindow), 0);
		const raw =
			files.reduce((n, f) => n + packedAt(f, sharedWindow), 0) + overhead(files.length, chunks);
		vol.root.push(
			entryOf({
				name,
				role: 'common_pack',
				raw,
				targets: ids,
				pack: { which: 'common', chunk: sharedWindow, set_id: setIdOf(info) },
				entries: packEntries(list),
			}),
		);
		vol.common += ceilTo(raw, cluster);
		for (const c of list)
			for (const f of c.files)
				filesOut.push({
					path: destOf('windows-x86', f.name),
					component: c.id,
					raw: f.raw,
					packed: packedAt(f, sharedWindow),
					pack: 'common',
					targets: ids,
					volume: vol.number,
				});
	};
	addCommon(v1, first, 'COMMON.LPK');
	if (volumes[1]) addCommon(volumes[1], second, 'COMMON.L02');

	root.push(
		entryOf({ name: 'README.TXT', role: 'readme', raw: readmeRaw, targets: ids }),
		...extras.map((e) => entryOf({ name: e.name, role: 'extra', raw: e.raw })),
	);
	if (info.kind === 'cdrom' && info.autorun)
		root.push(entryOf({ name: 'AUTORUN.INF', role: 'autorun', raw: autorunRaw }));
	const rank = {
		folder: 0,
		setup: 1,
		own_pack: 2,
		common_pack: 3,
		readme: 4,
		autorun: 5,
		extra: 6,
	};
	const byRole = (a: DiskEntry, b: DiskEntry) =>
		rank[a.role] - rank[b.role] || a.name.localeCompare(b.name);
	root.sort(byRole);
	for (const d of folders) folderEntry.get(d)!.children.sort(byRole);

	const total = (e: DiskEntry): number => e.on_disk + e.children.reduce((n, c) => n + total(c), 0);
	for (const v of volumes) {
		v.used = v.root.reduce((n, e) => n + total(e), 0);
		v.free = capacity - v.used;
	}
	v1.serial = serialOf(info);

	const components = [
		{
			id: 'program',
			raw: ownPacks.reduce((n, p) => n + p.meta.exe.raw, 0),
			packed: ownPacks.reduce((n, p) => n + p.meta.exe.packed, 0),
			pack: 'own' as PackWhich,
			volume: 1,
			users: ids,
		},
		...FIXTURE_COMPONENTS.map((c) => {
			const place = placed.find((p) => p.component === c);
			const on = isOn(c);
			const per = (w: number) => c.files.reduce((n, f) => n + packedAt(f, w), 0);
			const raw = c.files.reduce((n, f) => n + f.raw, 0);
			return {
				id: c.id,
				raw: !on ? 0 : raw * (place ? 1 : ownPacks.length),
				packed: !on
					? 0
					: place
						? per(sharedWindow)
						: ownPacks.reduce((n, p) => n + per(p.meta.window), 0),
				pack: (place ? 'common' : 'own') as PackWhich,
				volume: place ? place.volume : on ? 1 : 0,
				users: on ? ids : [],
			};
		}),
	];
	return {
		volumes,
		files: filesOut,
		components,
		used: volumes.reduce((n, v) => n + v.used, 0),
		capacity,
		cluster,
	};
}

/** The id that ties each target's project to the shared pack of one medium. */
const setIdOf = (info: MediaInfo): string =>
	[...`${info.id}:${info.label}`]
		.reduce((h, c) => (Math.imul(h, 31) + c.charCodeAt(0)) >>> 0, 7)
		.toString(16)
		.toUpperCase()
		.padStart(8, '0');

function serialOf(info: MediaInfo): string {
	const s = checkSerial(info.serial);
	return typeof s === 'number' ? formatSerial(s) : serialFromVersion(FIXTURE_PRODUCT.version);
}

const readmeLines = (info: MediaInfo): string[] => {
	const drive = info.kind === 'cdrom' ? 'D:' : 'A:';
	const lines = [
		`${FIXTURE_PRODUCT.slug.toUpperCase()} ${FIXTURE_PRODUCT.version}`,
		'',
		`This ${info.kind === 'cdrom' ? 'disc' : 'disk'} installs Quill on DOS, Windows 3.x and Windows 95.`,
		'',
	];
	for (const t of info.targets) {
		const meta = FIXTURE_TARGETS[t.target];
		if (!meta) continue;
		const path = `${drive}\\${t.dir ? `${t.dir}\\` : ''}SETUP`;
		lines.push(
			`${meta.name}: ${t.target === 'dos-x86' ? 'at the DOS prompt' : 'choose Run'} and type ${path}`,
		);
	}
	return lines;
};

/** A README for the medium: one line for each target. Lines end CR LF, as DOS expects. */
export const generatedReadme = (info: MediaInfo): string => `${readmeLines(info).join('\r\n')}\r\n`;

/** The text of `AUTORUN.INF` for a disc, or undefined when the medium has no autorun target. */
export function autorunText(info: MediaInfo): string | undefined {
	if (info.kind !== 'cdrom' || !info.autorun) return undefined;
	const t = info.targets.find((x) => x.target === info.autorun);
	if (!t) return undefined;
	const exe = `${t.dir ? `${t.dir}\\` : ''}SETUP.EXE`;
	return `[autorun]\r\nopen=${exe}\r\nicon=${exe},0\r\n`;
}

/** What is wrong with the entry itself, as the engine words it. `li-media`'s `validate` is the real one. */
export function validateInfo(info: MediaInfo): MediaWarning[] {
	const out: MediaWarning[] = [];
	const label = checkLabel(info.label, info.kind);
	if (label)
		out.push({
			code: 'label',
			severity: 'error',
			subject: 'label',
			message:
				label.kind === 'tooLong'
					? `the label is ${label.length} characters; the most is ${label.max}`
					: `the label has \`${label.char}\`, which a label may not hold`,
		});
	if (checkSerial(info.serial) === null)
		out.push({
			code: 'label',
			severity: 'error',
			subject: 'serial',
			message: 'a serial is `auto` or eight hexadecimal digits',
		});
	info.targets.forEach((t, i) => {
		if (checkDir(t.dir))
			out.push({
				code: 'dir_not_8_3',
				severity: 'error',
				subject: t.target,
				message: `\`${t.dir}\` is not an 8.3 folder name`,
			});
		else if (info.targets.slice(0, i).some((o) => o.dir.toUpperCase() === t.dir.toUpperCase()))
			out.push({
				code: 'dir_not_8_3',
				severity: 'error',
				subject: t.target,
				message: `${t.dir === '' ? 'the root' : t.dir} is used by more than one target`,
			});
	});
	if (info.autorun) {
		if (info.kind !== 'cdrom')
			out.push({
				code: 'autorun_target',
				severity: 'warning',
				subject: info.autorun,
				message: 'only a CD has an AUTORUN.INF',
			});
		else if (!info.targets.some((t) => t.target === info.autorun))
			out.push({
				code: 'autorun_target',
				severity: 'error',
				subject: info.autorun,
				message: `autorun names ${info.autorun}, which is not a target of this medium`,
			});
	}
	return out;
}

/** The free bytes over every disk of a layout: negative when the payload is larger than the disks. */
const spare = (l: Layout): number => l.volumes.length * l.capacity - l.used;

/** Works out the plan of a medium from the fixture. A new plan comes out of every call. */
export function planFromFixture(info: MediaInfo): MediaPlan {
	const layout = layoutOf(info, true);
	const free = layout.volumes[layout.volumes.length - 1]!.free;
	const cluster = layout.cluster;
	const reserve = info.reserve;
	const isOn = (c: FixtureComponent) => c.required || !info.exclude.includes(c.id);

	// What a switch does: plan the medium again with the component flipped and compare.
	const flip = (id: string): MediaInfo => ({
		...info,
		exclude: info.exclude.includes(id)
			? info.exclude.filter((x) => x !== id)
			: [...info.exclude, id],
	});
	const components: ComponentSize[] = layout.components.map((c) => {
		const meta = c.id === 'program' ? undefined : FIXTURE_COMPONENTS.find((x) => x.id === c.id)!;
		const required = meta ? meta.required : true;
		const after = required ? layout : layoutOf(flip(c.id), true);
		const afterFree = after.volumes[after.volumes.length - 1]!.free;
		return {
			id: c.id,
			name: meta ? meta.name : 'Quill program',
			required,
			default_on: meta ? meta.defaultOn : true,
			on_medium: meta ? isOn(meta) : true,
			raw: c.raw,
			packed: c.packed,
			pack: c.pack,
			targets: c.users,
			volume: c.volume,
			delta_if_toggled: required ? 0 : spare(after) - spare(layout),
			fits_if_toggled: required
				? true
				: info.overflow === 'span' && info.kind === 'floppy-1440'
					? true
					: afterFree >= Math.max(0, reserve),
		};
	});

	const targets: MediaTargetPlan[] = Object.entries(FIXTURE_TARGETS).map(([id, meta]) => {
		const on = info.targets.find((t) => t.target === id);
		return {
			target: id,
			dir: on ? on.dir : meta.dir,
			on_medium: on !== undefined,
			setup_bytes: meta.setup,
			...(meta.found ? { runtime: meta.found } : {}),
			estimated: meta.found === undefined,
		};
	});

	const warnings: MediaWarning[] = [...validateInfo(info)];
	const spans = layout.volumes.length > 1;
	if (info.overflow === 'error' && free < 0) {
		const largest = [...layout.files].sort((a, b) => b.packed - a.packed).slice(0, 3);
		warnings.push({
			code: 'does_not_fit',
			severity: 'error',
			subject: info.id,
			message: `the payload is ${formatBytes(-free)} bytes over; the largest files are ${largest
				.map((f) => `${f.path} (${formatBytes(f.packed)})`)
				.join(', ')}`,
		});
	} else if (free >= 0 && free < reserve) {
		warnings.push({
			code: 'below_reserve',
			severity: 'warning',
			subject: info.id,
			message: `${formatBytes(free)} bytes are free, which is less than the reserve of ${formatBytes(reserve)}`,
		});
	}
	for (const p of layout.components.filter((c) => c.volume > 1)) {
		const meta = FIXTURE_COMPONENTS.find((x) => x.id === p.id);
		if (meta?.defaultOn)
			warnings.push({
				code: 'default_on_later_disk',
				severity: 'warning',
				subject: p.id,
				message: `${meta.name} is ticked by default but is on disk ${p.volume}`,
			});
	}
	const winTargets = info.targets.filter((t) => FIXTURE_TARGETS[t.target]?.window === WINDOW_4K);
	if (winTargets.length > 0 && info.common === 'auto' && info.targets.length >= 2)
		warnings.push({
			code: 'win16_window',
			severity: 'warning',
			subject: 'COMMON.LPK',
			message: `COMMON.LPK uses 4 KiB chunks because Windows 3.x reads it; that costs about ${formatBytes(
				windowCost(info),
			)} bytes against 32 KiB`,
		});
	for (const t of targets.filter((x) => x.on_medium && x.estimated))
		warnings.push({
			code: 'runtime_estimated',
			severity: 'warning',
			subject: t.target,
			message: `no SETUP.EXE was found for ${t.target}, so its budgeted ${formatBytes(t.setup_bytes)} bytes are used`,
		});
	if (spans)
		warnings.push({
			code: 'runtime_lacks',
			severity: 'warning',
			subject: info.id,
			message:
				'no runtime can ask for the second disk yet, so a set of disks cannot be installed from',
		});

	const without = layoutOf(info, false);
	const readme = info.readme
		? { generated: false, text: generatedReadme(info) }
		: { generated: true, text: generatedReadme(info) };
	const autorun = autorunText(info);

	return {
		media_id: info.id,
		kind: info.kind,
		label: info.label,
		serial: serialOf(info),
		capacity: layout.capacity,
		cluster,
		used: layout.used,
		free,
		reserve,
		without_sharing: without.used,
		volumes: layout.volumes,
		targets,
		components,
		files: layout.files,
		warnings,
		readme,
		...(autorun === undefined ? {} : { autorun_text: autorun }),
	};
}

/** The bytes the shared pack would save with a 32 KiB window instead of 4 KiB: what Windows 3.x costs. */
function windowCost(info: MediaInfo): number {
	const on = FIXTURE_COMPONENTS.filter((c) => c.required || !info.exclude.includes(c.id));
	const files = on.flatMap((c) => c.files);
	const at = (window: number) => {
		const chunks = files.reduce((n, f) => n + chunksOf(f.raw, window), 0);
		const packed = files.reduce((n, f) => n + (window === WINDOW_4K ? f.p4 : f.p32), 0);
		return ceilTo(packed + overhead(files.length, chunks), CLUSTER[info.kind]);
	};
	return Math.max(0, at(WINDOW_4K) - at(WINDOW_32K));
}
