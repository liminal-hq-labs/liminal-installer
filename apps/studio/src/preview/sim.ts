// The numbers behind an interactive preview: space for the chosen components, the drive behind a folder and the copy animation.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** What a component adds to the sample space estimate: each file is this many megabytes. */
export const MB_PER_FILE = 16;

export interface SizedComponent {
	id: string;
	files: number;
}

export interface Drive {
	/** Capacity and free space before the install, in megabytes. */
	capacity: number;
	free: number;
}

/** Sample folders the Browse button steps through, for a Windows or a Unix style path. */
export const SAMPLE_FOLDERS = {
	windows: ['C:\\FIZZ', 'D:\\GAMES\\FIZZ', 'A:\\FIZZ'],
	unix: ['/home/you/Sample', '/mnt/usb/Sample', '/opt/Sample'],
} as const;

/** The sample drive behind a folder: `C:` roomy, `D:` nearly full, `A:` a floppy, `/mnt` a small card. */
export function driveOf(path: string): Drive {
	const letter = /^([A-Za-z]):/.exec(path)?.[1]?.toUpperCase();
	if (letter === 'A' || letter === 'B') return { capacity: 1.4, free: 1.4 };
	if (letter === 'D') return { capacity: 2048, free: 60 };
	if (letter === 'E') return { capacity: 8192, free: 640 };
	if (path.startsWith('/mnt/')) return { capacity: 4096, free: 70 };
	return { capacity: 16384, free: 12700 };
}

export const nextFolder = (current: string): string => {
	const list = /^[A-Za-z]:/.test(current) ? SAMPLE_FOLDERS.windows : SAMPLE_FOLDERS.unix;
	const at = list.findIndex((f) => f === current);
	return list[(at + 1) % list.length]!;
};

/** A size in megabytes as the setup would write it: KB, MB or GB. */
export function formatSize(mb: number): string {
	const v = Math.max(0, mb);
	if (v < 1) return `${Math.round(v * 1024)} KB`;
	if (v < 1000) return `${v < 10 ? v.toFixed(1) : Math.round(v)} MB`;
	return `${(v / 1024).toFixed(1)} GB`;
}

export interface Derived {
	/** The values the screens show, over the run's own: size, free space, the plan summary. */
	vars: Record<string, string>;
	requiredMb: number;
	/** Free space left after the install, as a fraction of the drive: what the free-space meter shows. */
	freeFraction: number;
	/** False when the choice does not fit on the drive. */
	fits: boolean;
	files: number;
}

/** What the chosen components and the chosen folder come to. */
export function derive(
	components: SizedComponent[],
	selected: string[],
	path: string,
	base: Record<string, string> = {},
): Derived {
	const chosen = components.filter((c) => selected.includes(c.id));
	const files = chosen.reduce((n, c) => n + c.files, 0);
	const requiredMb = chosen.reduce((n, c) => n + Math.max(1, c.files) * MB_PER_FILE, 0);
	const drive = driveOf(path);
	const after = drive.free - requiredMb;
	return {
		vars: {
			...base,
			'install.size': formatSize(requiredMb),
			'system.free_space': formatSize(drive.free),
			'system.free_space_after': formatSize(after),
			'plan.summary': `${chosen.length} components · ${files} files · ${formatSize(requiredMb)}`,
		},
		requiredMb,
		freeFraction: Math.max(0, Math.min(1, after / drive.capacity)),
		fits: after >= 0,
		files,
	};
}

/** The step to go to from `index` by `delta`, skipping steps that do not apply, or `index` at either end. */
export function stepFrom(steps: { skipped: boolean }[], index: number, delta: -1 | 1): number {
	let i = index;
	do i += delta;
	while (steps[i]?.skipped);
	return i >= 0 && i < steps.length ? i : index;
}

/** Seconds a file takes to copy at normal speed, and how long a billboard slide stays. */
export const FILE_SECONDS = 1.6;
export const SLIDE_SECONDS = 3.5;

export interface Frame {
	/** 0 to 1: the file being copied, and the whole install. */
	file: number;
	total: number;
	fileIndex: number;
	slide: number;
}

/** The copy animation at `t` seconds over `files` files: each file fills in turn, then a pause and again. */
export function copyFrame(t: number, files: number): Frame {
	const n = Math.max(1, files);
	const run = n * FILE_SECONDS;
	const u = t % (run + FILE_SECONDS);
	const done = u >= run;
	const fileIndex = done ? n - 1 : Math.min(n - 1, Math.floor(u / FILE_SECONDS));
	return {
		file: done ? 1 : (u % FILE_SECONDS) / FILE_SECONDS,
		total: Math.min(1, u / run),
		fileIndex,
		slide: Math.floor(t / SLIDE_SECONDS),
	};
}
