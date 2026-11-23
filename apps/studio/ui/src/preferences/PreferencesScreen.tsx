// The Preferences screen: the shared settings shell over the Studio's settings, with the Escape guard.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useMemo, useState } from 'react';
import { SettingsShell } from '@liminal-hq/studio-chrome/SettingsShell/SettingsShell';
import { createApi } from '../lib/api';
import type { Host } from '../lib/host';
import type { SettingsPort } from '../lib/settings';
import { useEscapeGuard } from './escapeGuard';
import { preferenceSections } from './sections';
import { usePreferences } from './usePreferences';

export interface PreferencesScreenProps {
	port: SettingsPort;
	host: Host;
	/** The section shown first. */
	initialId?: string;
	/** Asks for the open project's target ids. Defaults to the Studio's backend. */
	loadTargets?: () => Promise<string[]>;
}

/** The full ids of the open project's targets; none when there is no project or no backend to ask. */
const askForTargets = async (): Promise<string[]> => (await (await createApi()).listTargets()).map((t) => t.full_id);

function useTargets(load: () => Promise<string[]>): string[] {
	const [targets, setTargets] = useState<string[]>([]);
	useEffect(() => {
		let live = true;
		load()
			.then((ids) => live && setTargets(ids))
			.catch(() => undefined);
		return () => {
			live = false;
		};
	}, [load]);
	return targets;
}

/**
 * Changes apply at once and are written as a merge; a refusal puts the value back and says why on its row. Escape
 * puts back text that was typed and not applied before it is left to close the window.
 */
export function PreferencesScreen({ port, host, initialId = 'general', loadTargets = askForTargets }: PreferencesScreenProps) {
	const { settings, failures, update, reset } = usePreferences(port);
	const [active, setActive] = useState(initialId);
	const guard = useEscapeGuard();
	const targets = useTargets(loadTargets);
	const sections = useMemo(
		() => (settings ? preferenceSections({ settings, failures, update, reset, host, targets, epoch: guard.epoch }) : []),
		[settings, failures, update, reset, host, targets, guard.epoch],
	);
	if (!settings) return <div className="prefs-screen" aria-busy="true" />;
	return (
		<div className="prefs-screen" ref={guard.containerRef} {...guard.handlers}>
			<SettingsShell sections={sections} activeId={active} onSelect={setActive} labels={{ navigation: 'Preferences sections' }} />
		</div>
	);
}
