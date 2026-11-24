// The edits the Media screen makes: each is the change to send and the words for the history entry it becomes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import type { ComponentSize, MediaEdit, MediaInfo, MediaPlan, MediaWarningCode } from './types';
import type { MediaTab } from '../stores/media';

/** A change and the name of its history entry. */
export interface Change {
	edit: MediaEdit;
	label: string;
}

/** Puts a component on the medium or leaves it off. */
export function toggleChange(info: MediaInfo, component: ComponentSize, on: boolean): Change {
	const rest = info.exclude.filter((x) => x !== component.id);
	return {
		edit: { exclude: on ? rest : [...rest, component.id] },
		label: t(on ? 'history.mediaToggleOn' : 'history.mediaToggleOff', {
			component: component.name,
			id: info.id,
		}),
	};
}

/** Puts a target on the medium, in its usual folder, or takes it off. The order of the targets stays the plan's. */
export function targetChange(
	info: MediaInfo,
	plan: MediaPlan,
	target: string,
	name: string,
	on: boolean,
): Change {
	const keep = info.targets.filter((x) => x.target !== target);
	const dir = plan.targets.find((x) => x.target === target)?.dir ?? '';
	const next = on ? [...keep, { target, dir }] : keep;
	const order = plan.targets.map((x) => x.target);
	next.sort((a, b) => order.indexOf(a.target) - order.indexOf(b.target));
	// A disc that opened the target loses its autorun with it, in the same undo step.
	const orphan = !on && info.autorun === target;
	return {
		edit: { targets: next, ...(orphan ? { autorun: '' } : {}) },
		label: t(on ? 'history.mediaTargetOn' : 'history.mediaTargetOff', {
			target: name,
			id: info.id,
		}),
	};
}

/** Moves a target to another folder. */
export function dirChange(info: MediaInfo, target: string, name: string, dir: string): Change {
	return {
		edit: { targets: info.targets.map((x) => (x.target === target ? { ...x, dir } : x)) },
		label: t('history.mediaDir', { target: name, id: info.id }),
	};
}

/** The tab that fixes a warning, or null when there is nothing to open. */
export function warningTab(code: MediaWarningCode): MediaTab | null {
	switch (code) {
		case 'does_not_fit':
		case 'below_reserve':
		case 'default_on_later_disk':
			return 'sizes';
		case 'label':
			return 'label';
		case 'dir_not_8_3':
		case 'runtime_estimated':
		case 'runtime_lacks':
			return 'targets';
		case 'autorun_target':
			return 'autorun';
		case 'name_83':
			return 'map';
		case 'win16_window':
			return null;
	}
}

/** What a README text looks like against DOS: how wide its lines are and how they end. */
export interface ReadmeCheck {
	tooWide: number;
	lfOnly: boolean;
}

export const README_WIDTH = 76;

export function checkReadme(text: string): ReadmeCheck {
	const lines = text.split('\n');
	if (lines[lines.length - 1] === '') lines.pop();
	const bare = lines.some((l) => !l.endsWith('\r'));
	return {
		tooWide: lines.filter((l) => l.replace(/\r$/, '').length > README_WIDTH).length,
		lfOnly: bare,
	};
}
