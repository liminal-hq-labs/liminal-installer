// The 16 by 16 stroke drawings shared by the sidebar and the menus, as SVG path data.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** One entry per drawing: a single path, or several joined, in a 16 by 16 box. */
export const ICON_PATHS = {
	folder: 'M2 4.5h4l1.5 1.5H14v6.5H2z',
	flow: 'M3 3h4v3H3zM9 10h4v3H9zM7 4.5h2.5V10',
	layouts: 'M2 3h12v10H2zM2 6h12M6 6v7',
	code: 'M5 4L2 8l3 4M11 4l3 4-3 4M9 3l-2 10',
	floppy: 'M3 2.5h8.5l1.5 1.5v9.5H3zM5.5 2.5v3.5h5V2.5M5 13.5V9h6v4.5M9 3.8v1.2',
	package: 'M8 2l5 2.5v7L8 14l-5-2.5v-7zM3 4.5L8 7l5-2.5M8 7v7',
	eye: 'M8 3C4 3 2 8 2 8s2 5 6 5 6-5 6-5-2-5-6-5zM8 6a2 2 0 100 4 2 2 0 000-4z',
	newProject: 'M4 2.5h5l3 3v8H4zM9 2.5v3h3M8 8v4M6 10h4',
	recent: 'M8 2.5a5.5 5.5 0 100 11 5.5 5.5 0 000-11zM8 5v3l2 1.5',
	reload: 'M13 8a5 5 0 11-1.6-3.7M13 3v3h-3',
	clear: 'M3.5 4.5h9M6 4.5V3h4v1.5M5 4.5l.5 8h5l.5-8',
	save: 'M3 3h8l2 2v8H3zM5.5 3v3h4V3M5.5 13V9h5v4',
	preferences:
		'M3 5h10M3 11h10M6 3.5a1.5 1.5 0 100 3 1.5 1.5 0 000-3zM10 9.5a1.5 1.5 0 100 3 1.5 1.5 0 000-3z',
	quit: 'M7 3H4v10h3M7 8h6M11 6l2 2-2 2',
	undo: 'M5.5 3.5L3 6l2.5 2.5M3 6h6.5a3.5 3.5 0 010 7H7',
	redo: 'M10.5 3.5L13 6l-2.5 2.5M13 6H6.5a3.5 3.5 0 000 7H9',
	fullScreen: 'M3 6V3h3M10 3h3v3M13 10v3h-3M6 13H3v-3',
	check: 'M8 2.5a5.5 5.5 0 100 11 5.5 5.5 0 000-11zM5.5 8.2l1.8 1.8 3.2-3.5',
	play: 'M5 3.5v9l7-4.5z',
	keyboard: 'M2 4.5h12v7H2zM4.5 7h.01M7 7h.01M9.5 7h.01M12 7h.01M5 9.5h6',
	agent: 'M6 2.5v3M10 2.5v3M4 5.5h8v3a4 4 0 01-8 0zM8 12.5v1.5',
	info: 'M8 2.5a5.5 5.5 0 100 11 5.5 5.5 0 000-11zM8 7.5v3.5M8 5.2v.01',
} as const;

export type IconName = keyof typeof ICON_PATHS;
