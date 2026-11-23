// The Studio's one menu model: the app menu and the menu bar are both built from it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem, SelectableMenuItem } from '@liminal-hq/studio-chrome/ContextMenu/types';
import { t, type MessageId } from '../i18n/messages';

export type Screen = 'project' | 'flow' | 'layouts' | 'script' | 'preview';

export type ActionId =
	| 'file.new'
	| 'file.open'
	| 'file.reload'
	| 'file.clearRecent'
	| 'file.save'
	| 'file.quit'
	| 'app.preferences'
	| 'edit.undo'
	| 'edit.redo'
	| 'view.project'
	| 'view.flow'
	| 'view.layouts'
	| 'view.script'
	| 'view.preview'
	| 'view.fullScreenPreview'
	| 'view.menuBar'
	| 'project.check'
	| 'project.preview'
	| 'help.shortcuts'
	| 'help.agent'
	| 'help.about';

/** What the menus need to know to label, tick and enable their items. */
export interface MenuState {
	hasProject: boolean;
	canUndo: boolean;
	canRedo: boolean;
	screen: Screen;
	/** The Script screen has a script with edits that are not saved. */
	scriptDirty: boolean;
	menuBar: boolean;
	/** Paths of the recent projects, newest first. */
	recent: string[];
}

export const initialMenuState: MenuState = {
	hasProject: false,
	canUndo: false,
	canRedo: false,
	screen: 'project',
	scriptDirty: false,
	menuBar: false,
	recent: [],
};

interface Action {
	/** The catalogue message that labels the item. */
	label: MessageId;
	/** Shown beside the item and matched against key presses. The first is the one displayed. */
	keys?: string[];
	enabled?: (s: MenuState) => boolean;
	/** Present for items that show a tick. */
	checked?: (s: MenuState) => boolean;
	/** Leave the key to a focused text field, which has its own meaning for it. */
	textFieldOwnsKey?: boolean;
}

const needsProject = (s: MenuState) => s.hasProject;

export const SCREEN_ACTIONS: Record<Screen, ActionId> = {
	project: 'view.project',
	flow: 'view.flow',
	layouts: 'view.layouts',
	script: 'view.script',
	preview: 'view.preview',
};

const screenAction = (screen: Screen, label: MessageId, key: string): Action => ({
	label,
	keys: [`Ctrl+${key}`],
	enabled: needsProject,
	checked: (s) => s.screen === screen,
});

export const ACTIONS: Record<ActionId, Action> = {
	'file.new': { label: 'menu.newProject', keys: ['Ctrl+N'] },
	'file.open': { label: 'menu.openProject', keys: ['Ctrl+O'] },
	'file.reload': { label: 'menu.reload', keys: ['F5'], enabled: needsProject },
	'file.clearRecent': { label: 'menu.clearRecent', enabled: (s) => s.recent.length > 0 },
	'file.save': {
		label: 'menu.save',
		keys: ['Ctrl+S'],
		enabled: (s) => s.hasProject && s.scriptDirty,
	},
	'app.preferences': { label: 'menu.preferences', keys: ['Ctrl+,'] },
	'file.quit': { label: 'menu.quit', keys: ['Ctrl+Q'] },
	'edit.undo': {
		label: 'menu.undo',
		keys: ['Ctrl+Z'],
		enabled: (s) => s.hasProject && s.canUndo,
		textFieldOwnsKey: true,
	},
	'edit.redo': {
		label: 'menu.redo',
		keys: ['Ctrl+Shift+Z', 'Ctrl+Y'],
		enabled: (s) => s.hasProject && s.canRedo,
		textFieldOwnsKey: true,
	},
	'view.project': screenAction('project', 'menu.viewProject', '1'),
	'view.flow': screenAction('flow', 'menu.viewFlow', '2'),
	'view.layouts': screenAction('layouts', 'menu.viewLayouts', '3'),
	'view.script': screenAction('script', 'menu.viewScripts', '4'),
	'view.preview': screenAction('preview', 'menu.viewDryRun', '5'),
	'view.fullScreenPreview': {
		label: 'menu.fullScreenPreview',
		keys: ['F11'],
		enabled: (s) => s.hasProject && s.screen === 'preview',
	},
	'view.menuBar': { label: 'menu.showMenuBar', keys: ['Ctrl+Shift+M'], checked: (s) => s.menuBar },
	'project.check': { label: 'menu.runCheck', keys: ['F7'], enabled: needsProject },
	'project.preview': { label: 'menu.runPreview', keys: ['F8'], enabled: needsProject },
	'help.shortcuts': { label: 'menu.shortcuts', keys: ['Ctrl+/'] },
	'help.agent': { label: 'menu.connectAgent' },
	'help.about': { label: 'menu.about', keys: ['F1'] },
};

/** In a group, this stands for the submenu of recent projects. */
export const RECENT = 'recent' as const;

interface Group {
	items: (ActionId | typeof RECENT)[];
	/** In the app menu, which has no menus to order the groups, this one goes last. */
	last?: boolean;
}

interface Menu {
	id: string;
	label: MessageId;
	/** Alt plus this letter opens the menu in the bar. */
	mnemonic: string;
	groups: Group[];
}

/** The menus of the bar, in order. Each group of items is set off from the next by a separator. */
export const MENUS: Menu[] = [
	{
		id: 'file',
		label: 'menu.file',
		mnemonic: 'f',
		groups: [
			{ items: ['file.new', 'file.open', RECENT, 'file.reload'] },
			{ items: ['file.save'] },
			{ items: ['app.preferences'] },
			{ items: ['file.quit'], last: true },
		],
	},
	{
		id: 'edit',
		label: 'menu.edit',
		mnemonic: 'e',
		groups: [{ items: ['edit.undo', 'edit.redo'] }],
	},
	{
		id: 'view',
		label: 'menu.view',
		mnemonic: 'v',
		groups: [
			{ items: ['view.project', 'view.flow', 'view.layouts', 'view.script', 'view.preview'] },
			{ items: ['view.fullScreenPreview', 'view.menuBar'] },
		],
	},
	{
		id: 'project',
		label: 'menu.project',
		mnemonic: 'p',
		groups: [{ items: ['project.check', 'project.preview'] }],
	},
	{
		id: 'help',
		label: 'menu.help',
		mnemonic: 'h',
		groups: [{ items: ['help.shortcuts', 'help.agent'] }, { items: ['help.about'] }],
	},
];

/** Alt plus a letter, for the bar: letters to the ids of the menus the bar builds. */
export const MNEMONICS: Readonly<Record<string, string>> = Object.fromEntries(
	MENUS.map((m) => [m.mnemonic, menuId(m.id)]),
);

function menuId(id: string): string {
	return `menu:${id}`;
}

export function isEnabled(id: ActionId, state: MenuState): boolean {
	return ACTIONS[id].enabled?.(state) ?? true;
}

function item(id: ActionId, state: MenuState): SelectableMenuItem {
	const action = ACTIONS[id];
	const shortcut = action.keys?.[0];
	const base = {
		id,
		label: t(action.label),
		disabled: !isEnabled(id, state),
		...(shortcut ? { shortcut } : {}),
	};
	return action.checked
		? { type: 'checkbox', ...base, checked: action.checked(state) }
		: { type: 'action', ...base };
}

const RECENT_PREFIX = 'recent:';

/** The id of the menu row for the recent project at `index`. */
export const recentId = (index: number): string => `${RECENT_PREFIX}${index}`;

/** The path a recent-project row stands for, or undefined for any other id, a placeholder or a stale index. */
export function recentPath(id: string, state: Pick<MenuState, 'recent'>): string | undefined {
	if (!id.startsWith(RECENT_PREFIX)) return undefined;
	return state.recent[Number(id.slice(RECENT_PREFIX.length))];
}

/** The last two parts of a path, which tell projects apart better than the folder name alone. */
export function shortPath(path: string): string {
	return path.split(/[\\/]/).filter(Boolean).slice(-2).join('/') || path;
}

/** Open recent: a row per project and, after a separator, Clear recent projects. */
function recentMenu(state: MenuState): MenuItem {
	const rows: MenuItem[] =
		state.recent.length === 0
			? [
					{
						type: 'action',
						id: `${RECENT_PREFIX}none`,
						label: t('menu.noRecent'),
						disabled: true,
					},
				]
			: [
					...state.recent.map((path, i): MenuItem => ({
						type: 'action',
						id: recentId(i),
						label: shortPath(path),
						title: path,
					})),
					{ type: 'separator', id: 'separator:recent' },
					item('file.clearRecent', state),
				];
	return { type: 'submenu', id: 'menu:recent', label: t('menu.openRecent'), items: rows };
}

/** Items of several groups, one separator between neighbouring groups. */
function joinGroups(groups: Group[], state: MenuState): MenuItem[] {
	return groups.flatMap((group, index): MenuItem[] => [
		...(index > 0 ? [{ type: 'separator' as const, id: `separator:${index}` }] : []),
		...group.items.map((id) => (id === RECENT ? recentMenu(state) : item(id, state))),
	]);
}

/** The menu bar: one submenu per menu. */
export function buildBarMenu(state: MenuState): MenuItem[] {
	return MENUS.map((menu) => ({
		type: 'submenu',
		id: menuId(menu.id),
		label: t(menu.label),
		items: joinGroups(menu.groups, state),
	}));
}

/** The app menu: one flat menu with no submenus, made of the same groups in the bar's order. */
export function buildFlatMenu(state: MenuState): MenuItem[] {
	const groups = MENUS.flatMap((menu) => menu.groups);
	return joinGroups([...groups.filter((g) => !g.last), ...groups.filter((g) => g.last)], state);
}

/** Both menus from the one model. */
export function buildMenus(state: MenuState): { flat: MenuItem[]; bar: MenuItem[] } {
	return { flat: buildFlatMenu(state), bar: buildBarMenu(state) };
}

/** True for an id of an action, as opposed to a menu or separator. */
export function isActionId(id: string): id is ActionId {
	return id in ACTIONS;
}

/** Every selectable item in a menu, however deeply nested. */
export function selectableItems(items: MenuItem[]): SelectableMenuItem[] {
	return items.flatMap((i) =>
		i.type === 'submenu'
			? selectableItems(i.items)
			: i.type === 'action' || i.type === 'checkbox'
				? [i]
				: [],
	);
}

interface KeyEventLike {
	key: string;
	ctrlKey: boolean;
	metaKey: boolean;
	shiftKey: boolean;
	altKey: boolean;
}

/** True when the key press is the shortcut, such as `Ctrl+Shift+Z`. Command counts as Ctrl. */
export function matchesShortcut(event: KeyEventLike, shortcut: string): boolean {
	const parts = shortcut.split('+');
	const key = parts[parts.length - 1]!.toLowerCase();
	const mods = new Set(parts.slice(0, -1).map((m) => m.toLowerCase()));
	return (
		event.key.toLowerCase() === key &&
		(event.ctrlKey || event.metaKey) === mods.has('ctrl') &&
		event.shiftKey === mods.has('shift') &&
		event.altKey === mods.has('alt')
	);
}

/** The enabled action a key press runs, if any. `inTextField` skips the keys such a field keeps for itself. */
export function actionForEvent(
	event: KeyEventLike,
	state: MenuState,
	inTextField: boolean,
): ActionId | undefined {
	for (const id of Object.keys(ACTIONS) as ActionId[]) {
		const action = ACTIONS[id];
		if (inTextField && action.textFieldOwnsKey) continue;
		if (action.keys?.some((k) => matchesShortcut(event, k)) && isEnabled(id, state)) return id;
	}
	return undefined;
}

/** The shortcut table for the Keyboard shortcuts window: every command that has a key, grouped by its menu. */
export function shortcutTable(): {
	menu: string;
	rows: { id: ActionId; label: string; keys: string[] }[];
}[] {
	return MENUS.map((menu) => ({
		menu: t(menu.label),
		rows: menu.groups
			.flatMap((g) => g.items)
			.filter((id): id is ActionId => id !== RECENT && (ACTIONS[id].keys?.length ?? 0) > 0)
			.map((id) => ({ id, label: t(ACTIONS[id].label), keys: ACTIONS[id].keys! })),
	})).filter((g) => g.rows.length > 0);
}
