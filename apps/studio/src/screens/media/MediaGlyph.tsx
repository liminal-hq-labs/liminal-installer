// The pixel-art drawing of a floppy disk or a CD that marks a medium in the list and the detail heading.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MediaKind } from '../../lib/types';

/** `#` is a filled pixel. */
const FLOPPY = [
	'###########.',
	'#.####.....#',
	'#.####.....#',
	'#.####.....#',
	'#..........#',
	'#..........#',
	'#.########.#',
	'#.#......#.#',
	'#.#......#.#',
	'#.#......#.#',
	'#.########.#',
	'############',
];

const SIZE = 12;

/** A disc: a ring around a hole, drawn from the distance of each pixel to the middle. */
const CD = Array.from({ length: SIZE }, (_, y) =>
	Array.from({ length: SIZE }, (_, x) => {
		const d = Math.hypot(x - (SIZE - 1) / 2, y - (SIZE - 1) / 2);
		return (d <= 5.6 && d >= 2.4) || d <= 0.8 ? '#' : '.';
	}).join(''),
);

/** A medium's kind, drawn on a 12 by 12 grid of square pixels. Decorative: the row names the kind in words. */
export function MediaGlyph({ kind, size = 24 }: { kind: MediaKind; size?: number }) {
	const rows = kind === 'cdrom' ? CD : FLOPPY;
	return (
		<svg
			viewBox={`0 0 ${SIZE} ${SIZE}`}
			width={size}
			height={size}
			fill="currentColor"
			shapeRendering="crispEdges"
			aria-hidden="true"
			focusable="false"
		>
			{rows.flatMap((row, y) =>
				[...row].map((c, x) =>
					c === '#' ? <rect key={`${x},${y}`} x={x} y={y} width="1" height="1" /> : null,
				),
			)}
		</svg>
	);
}
