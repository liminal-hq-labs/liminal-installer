// The sections of Preferences, in the order they are listed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SettingsSectionDef } from '@liminal-hq/studio-chrome/SettingsShell/types';
import { AdvancedPage, AgentsPage, AppearancePage, EditorPage, GeneralPage, PreviewPage, ProjectsPage, type PageProps } from './pages';

export const SECTION_IDS = ['general', 'appearance', 'editor', 'preview', 'agents', 'projects', 'advanced'] as const;
export type SectionId = (typeof SECTION_IDS)[number];

export const SECTION_LABELS: Record<SectionId, string> = {
	general: 'General',
	appearance: 'Appearance',
	editor: 'Editor',
	preview: 'Preview',
	agents: 'Agents',
	projects: 'Projects',
	advanced: 'Advanced',
};

const PAGES: Record<SectionId, (props: PageProps) => React.JSX.Element> = {
	general: GeneralPage,
	appearance: AppearancePage,
	editor: EditorPage,
	preview: PreviewPage,
	agents: AgentsPage,
	projects: ProjectsPage,
	advanced: AdvancedPage,
};

/** The sections for the shell, each drawing its page over `props`. */
export function preferenceSections(props: PageProps): SettingsSectionDef[] {
	return SECTION_IDS.map((id) => {
		const Page = PAGES[id];
		return { id, label: SECTION_LABELS[id], render: () => <Page {...props} /> };
	});
}
