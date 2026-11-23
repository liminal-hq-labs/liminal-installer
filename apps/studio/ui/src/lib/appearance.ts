// Appearance settings turned into what the page shows: the theme, density, text size and motion, with the system's choice where asked.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MotionSetting, Settings, ThemeId } from './settings';

/** What the operating system prefers. */
export interface SystemAppearance {
	dark: boolean;
	reducedMotion: boolean;
}

export const defaultSystem: SystemAppearance = { dark: true, reducedMotion: false };

/** The theme id the design tokens define, with `system` resolved to the dark or light Afterglow. */
export function resolveTheme(theme: ThemeId, system: SystemAppearance): Exclude<ThemeId, 'system'> {
	if (theme !== 'system') return theme;
	return system.dark ? 'afterglow' : 'afterglow-light';
}

/** Whether motion is reduced: the system's choice for `system`, otherwise the setting. */
export function resolveMotion(motion: MotionSetting, system: SystemAppearance): boolean {
	return motion === 'system' ? system.reducedMotion : motion === 'on';
}

/** The parts of the document element the settings change. */
export interface AppearanceTarget {
	dataset: DOMStringMap;
	style: { setProperty(name: string, value: string): void };
}

/** Sets the theme, density, text scale and motion on `root`. */
export function applyAppearance(
	root: AppearanceTarget,
	settings: Pick<Settings, 'theme' | 'density' | 'textSize' | 'reduceMotion'>,
	system: SystemAppearance,
): void {
	root.dataset.theme = resolveTheme(settings.theme, system);
	root.dataset.density = settings.density;
	root.dataset.motion = resolveMotion(settings.reduceMotion, system) ? 'reduced' : 'full';
	root.style.setProperty('--text-scale', String(settings.textSize / 100));
}

/** The browser's own answer, used outside the desktop shell and when the system gives none. */
export function browserSystem(
	media: (query: string) => { matches: boolean } = (q) => window.matchMedia(q),
): SystemAppearance {
	return {
		dark: !media('(prefers-color-scheme: light)').matches,
		reducedMotion: media('(prefers-reduced-motion: reduce)').matches,
	};
}

/**
 * Follows the system's preferences: the desktop shell's appearance plugin where there is one, the browser's media
 * queries otherwise and where the plugin has no answer. Calls `listener` at once and on every change.
 */
export async function followSystem(
	listener: (system: SystemAppearance) => void,
	inTauri: boolean,
): Promise<() => void> {
	const stops: (() => void)[] = [];
	let plugin: SystemAppearance | null = null;
	const emit = () => listener(plugin ?? browserSystem());
	if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
		for (const q of ['(prefers-color-scheme: light)', '(prefers-reduced-motion: reduce)']) {
			const m = window.matchMedia(q);
			m.addEventListener('change', emit);
			stops.push(() => m.removeEventListener('change', emit));
		}
	}
	if (inTauri) {
		try {
			const api = await import('@liminal-hq/plugin-system-appearance');
			const take = (a: { colourScheme: string; reducedMotion: boolean }) => {
				const fallback = browserSystem();
				plugin = {
					dark: a.colourScheme === 'noPreference' ? fallback.dark : a.colourScheme !== 'light',
					reducedMotion: a.reducedMotion,
				};
				emit();
			};
			take(await api.getAppearance());
			stops.push(await api.onAppearanceChanged(take));
		} catch {
			// The plugin has no answer here; the browser's takes over.
		}
	}
	emit();
	return () => stops.forEach((s) => s());
}
