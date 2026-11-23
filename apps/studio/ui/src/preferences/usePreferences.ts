// The settings as Preferences shows them: read once, followed when they change elsewhere, written with a reason on refusal.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useRef, useState } from 'react';
import type { SettingKey, Settings, SettingsPort } from '../lib/settings';

/** The reason the last write to each setting was refused, until the next change to it. */
export type Failures = Partial<Record<SettingKey, string>>;

export interface Preferences {
	/** The settings in force, or null until they are read. */
	settings: Settings | null;
	failures: Failures;
	/**
	 * Shows a change at once and writes it. When the write is refused the settings go back to the ones in force and
	 * the reason is kept for the setting. Resolves to whether it was saved.
	 */
	update(patch: Partial<Settings>): Promise<boolean>;
	/** Puts every setting back to its default. Resolves to whether it was done. */
	reset(): Promise<boolean>;
}

const messageOf = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** `Not saved` followed by the reason, as a row's error; nothing when the setting was not refused. */
export function refusal(failures: Failures, key: SettingKey): { error?: string } {
	const reason = failures[key];
	return reason ? { error: `Not saved: ${reason}` } : {};
}

export function usePreferences(port: SettingsPort): Preferences {
	const [settings, setSettings] = useState<Settings | null>(null);
	const [failures, setFailures] = useState<Failures>({});
	const latest = useRef<Settings | null>(null);
	const show = useCallback((next: Settings) => {
		latest.current = next;
		setSettings(next);
	}, []);

	useEffect(() => {
		let live = true;
		let stop: (() => void) | undefined;
		void port.read().then((read) => live && show(read), () => undefined);
		// A change made in another window, or by the app itself, reaches this one too.
		void port
			.onChange((changed) => live && show(changed))
			.then((off) => {
				if (live) stop = off;
				else off();
			});
		return () => {
			live = false;
			stop?.();
		};
	}, [port, show]);

	const restore = useCallback(async () => {
		try {
			show(await port.read());
		} catch {
			// The refusal is the news; the next change to the settings brings the value back.
		}
	}, [port, show]);

	const update = useCallback(
		async (patch: Partial<Settings>) => {
			if (!latest.current) return false;
			const keys = Object.keys(patch) as SettingKey[];
			setFailures((now) => {
				const next = { ...now };
				for (const k of keys) delete next[k];
				return next;
			});
			show({ ...latest.current, ...patch });
			try {
				show(await port.write(patch));
				return true;
			} catch (error) {
				const reason = messageOf(error);
				setFailures((now) => ({ ...now, ...Object.fromEntries(keys.map((k) => [k, reason])) }));
				await restore();
				return false;
			}
		},
		[port, show, restore],
	);

	const reset = useCallback(async () => {
		try {
			show(await port.reset());
			setFailures({});
			return true;
		} catch (error) {
			setFailures((now) => ({ ...now, version: messageOf(error) }));
			return false;
		}
	}, [port, show]);

	return { settings, failures, update, reset };
}
