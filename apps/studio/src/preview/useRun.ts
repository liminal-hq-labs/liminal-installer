// The state of an interactive dry run: choices, the folder, the step on show and the copy animation.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useMemo, useState } from 'react';
import type { ComponentInfo, Preview } from '../lib/types';
import type { Live } from './model';
import { copyFrame, derive, nextFolder, stepFrom, type Derived } from './sim';
import { usePlayback, type Playback } from './usePlayback';

export interface Run {
	/** The preview's values with the ones the chosen components and folder change. */
	vars: Record<string, string>;
	/** The page's state with the choices made so far over it. */
	state: Record<string, unknown>;
	live: Live;
	derived: Derived;
	selected: string[];
	playback: Playback;
	onChange(key: string, value: unknown): void;
	onAction(action: string): void;
	/** Forgets the choices and goes back to the first step. */
	resetChoices(): void;
}

const NAMES = ['data/level-01.lvl', 'data/level-02.lvl', 'sound/sb16.drv', 'music/theme.mod'];

/**
 * Runs a dry run's screens as a person would: Next and Back step through the flow without landing
 * on skipped steps, controls change the page, and the numbers follow (space required, free space,
 * the plan summary). `index` and `setIndex` are the step on show, held by the caller.
 */
export function useRun(
	pv: Preview | null,
	components: ComponentInfo[],
	index: number,
	setIndex: (i: number) => void,
): Run {
	const [values, setValues] = useState<Record<string, unknown>>({});
	const playback = usePlayback();
	const { reset } = playback;
	useEffect(() => {
		setValues({});
		reset();
	}, [pv, reset]);

	const step = pv?.steps[index];
	const firstState = pv?.steps.find((s) => s.state?.['components.selected'])?.state;
	const pick = (key: string): unknown => values[key] ?? step?.state?.[key] ?? firstState?.[key];
	const selectedRaw = pick('components.selected');
	const selected = useMemo(
		() => (Array.isArray(selectedRaw) ? (selectedRaw as string[]) : components.map((c) => c.id)),
		[selectedRaw, components],
	);
	const pathRaw = pick('install.path');
	const path = typeof pathRaw === 'string' ? pathRaw : (pv?.vars['install.path'] ?? '');
	const derived = useMemo(
		() => derive(components, selected, path, pv?.vars ?? {}),
		[components, selected, path, pv],
	);

	const moving = playback.playing || playback.t > 0;
	const frame = moving ? copyFrame(playback.t, derived.files || 1) : null;
	const vars: Record<string, string> = { ...derived.vars, 'install.path': path };
	if (frame) {
		vars['progress.file'] = NAMES[frame.fileIndex % NAMES.length]!;
		vars['progress.percent'] = String(Math.round(frame.total * 100));
	}
	const live: Live = {
		freeSpace: derived.freeFraction,
		...(frame ? { file: frame.file, total: frame.total, slide: frame.slide } : {}),
	};

	const steps = pv?.steps ?? [];
	return {
		vars,
		state: { ...(step?.state ?? {}), ...values, 'install.path': path },
		live,
		derived,
		selected,
		playback,
		onChange: (key, value) => setValues((v) => ({ ...v, [key]: value })),
		onAction: (action) => {
			if (action === 'next' || action === 'commit' || action === 'skip')
				setIndex(stepFrom(steps, index, 1));
			else if (action === 'back') setIndex(stepFrom(steps, index, -1));
			else if (action === 'cancel') {
				const first = steps.findIndex((s) => !s.skipped);
				setIndex(Math.max(0, first));
			} else if (action === 'finish')
				setIndex(
					Math.max(
						0,
						steps.findIndex((s) => !s.skipped),
					),
				);
			else if (action === 'browse') setValues((v) => ({ ...v, 'install.path': nextFolder(path) }));
		},
		resetChoices: () => {
			setValues({});
			playback.reset();
			setIndex(
				Math.max(
					0,
					steps.findIndex((s) => !s.skipped),
				),
			);
		},
	};
}
