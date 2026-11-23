// The Preferences screen: the shared settings shell over the Studio's settings.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useMemo, useState, useSyncExternalStore } from 'react';
import { SelectRow } from '@liminal-hq/studio-chrome/SettingsShell/SelectRow';
import { SettingsGroup } from '@liminal-hq/studio-chrome/SettingsShell/SettingsGroup';
import { SettingsShell } from '@liminal-hq/studio-chrome/SettingsShell/SettingsShell';
import type { SettingsSectionDef } from '@liminal-hq/studio-chrome/SettingsShell/types';
import { ToggleRow } from '@liminal-hq/studio-chrome/SettingsShell/ToggleRow';
import { THEMES, type Settings, type ThemeId } from '../lib/settings';
import type { ChromeStore } from './chromeStore';

const THEME_LABELS: Record<ThemeId, string> = {
	afterglow: 'Afterglow',
	'afterglow-dark': 'Afterglow dark',
	'afterglow-light': 'Afterglow light',
};

export function PreferencesScreen({ store }: { store: ChromeStore }) {
	const { settings } = useSyncExternalStore(store.subscribe, store.get);
	const [active, setActive] = useState('general');
	// The reason the last change was refused, kept by setting until the next change to it.
	const [errors, setErrors] = useState<Partial<Record<keyof Settings, string>>>({});

	const change = <K extends keyof Settings>(key: K, value: Settings[K]) => {
		setErrors((e) => ({ ...e, [key]: undefined }));
		store.updateSettings({ [key]: value }).catch((reason) => setErrors((e) => ({ ...e, [key]: String(reason instanceof Error ? reason.message : reason) })));
	};

	const sections = useMemo<SettingsSectionDef[]>(
		() => [
			{
				id: 'general',
				label: 'General',
				render: () => (
					<SettingsGroup title="Window">
						<ToggleRow
							label="Menu bar"
							description="Show File, Edit, View, Project and Help under the title bar. The app menu stays on the wordmark."
							checked={settings.menuBar}
							onChange={(v) => change('menuBar', v)}
							{...(errors.menuBar ? { error: errors.menuBar } : {})}
						/>
					</SettingsGroup>
				),
			},
			{
				id: 'appearance',
				label: 'Appearance',
				render: () => (
					<SettingsGroup title="Theme">
						<SelectRow
							label="Colour theme"
							description="Applies to every Studio window."
							value={settings.theme}
							options={THEMES.map((t) => ({ value: t, label: THEME_LABELS[t] }))}
							onChange={(v) => change('theme', v)}
							{...(errors.theme ? { error: errors.theme } : {})}
						/>
					</SettingsGroup>
				),
			},
		],
		// `change` only closes over the store, which does not change.
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[settings, errors],
	);

	return <SettingsShell sections={sections} activeId={active} onSelect={setActive} />;
}
