// The Studio's one menu model: the app menu and the menu bar are both built from it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem, SelectableMenuItem } from '@liminal-hq/studio-chrome/ContextMenu/types';

export type Screen = 'project' | 'flow' | 'layouts' | 'script' | 'preview';

export type ActionId =
	| 'file.open'
	| 'file.reload'
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
	| 'view.menuBar'
	| 'project.check'
	| 'project.preview'
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
}

export const initialMenuState: MenuState = {
	hasProject: false,
	canUndo: false,
	canRedo: false,
	screen: 'project',
	scriptDirty: false,
	menuBar: false,
};

interface Action {
	label: string;
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

const screenAction = (screen: Screen, label: string, key: string): Action => ({
	label,
	keys: [`Ctrl+${key}`],
	enabled: needsProject,
	checked: (s) => s.screen === screen,
});

export const ACTIONS: Record<ActionId, Action> = {
	'file.open': { label: 'Open project…', keys: ['Ctrl+O'] },
	'file.reload': { label: 'Reload', keys: ['F5'], enabled: needsProject },
	'file.save': { label: 'Save', keys: ['Ctrl+S'], enabled: (s) => s.hasProject && s.scriptDirty },
	'app.preferences': { label: 'Preferences…', keys: ['Ctrl+,'] },
	'file.quit': { label: 'Quit', keys: ['Ctrl+Q'] },
	'edit.undo': { label: 'Undo', keys: ['Ctrl+Z'], enabled: (s) => s.hasProject && s.canUndo, textFieldOwnsKey: true },
	'edit.redo': { label: 'Redo', keys: ['Ctrl+Shift+Z', 'Ctrl+Y'], enabled: (s) => s.hasProject && s.canRedo, textFieldOwnsKey: true },
	'view.project': screenAction('project', 'Project', '1'),
	'view.flow': screenAction('flow', 'Flow', '2'),
	'view.layouts': screenAction('layouts', 'Layouts', '3'),
	'view.script': screenAction('script', 'Scripts', '4'),
	'view.preview': screenAction('preview', 'Dry run', '5'),
	'view.menuBar': { label: 'Show menu bar', keys: ['Ctrl+Shift+M'], checked: (s) => s.menuBar },
	'project.check': { label: 'Run check', keys: ['F7'], enabled: needsProject },
	'project.preview': { label: 'Run dry-run preview', keys: ['F8'], enabled: needsProject },
	'help.about': { label: 'About Liminal Installer Studio', keys: ['F1'] },
};

interface Group {
	items: ActionId[];
	/** In the app menu, which has no menus to order the groups, this one goes last. */
	last?: boolean;
}

interface Menu {
	id: string;
	label: string;
	/** Alt plus this letter opens the menu in the bar. */
	mnemonic: string;
	groups: Group[];
}

/** The menus of the bar, in order. Each group of items is set off from the next by a separator. */
export const MENUS: Menu[] = [
	{ id: 'file', label: 'File', mnemonic: 'f', groups: [{ items: ['file.open', 'file.reload'] }, { items: ['file.save'] }, { items: ['app.preferences'] }, { items: ['file.quit'], last: true }] },
	{ id: 'edit', label: 'Edit', mnemonic: 'e', groups: [{ items: ['edit.undo', 'edit.redo'] }] },
	{
		id: 'view',
		label: 'View',
		mnemonic: 'v',
		groups: [{ items: ['view.project', 'view.flow', 'view.layouts', 'view.script', 'view.preview'] }, { items: ['view.menuBar'] }],
	},
	{ id: 'project', label: 'Project', mnemonic: 'p', groups: [{ items: ['project.check', 'project.preview'] }] },
	{ id: 'help', label: 'Help', mnemonic: 'h', groups: [{ items: ['help.about'] }] },
];

/** Alt plus a letter, for the bar: letters to the ids of the menus the bar builds. */
export const MNEMONICS: Readonly<Record<string, string>> = Object.fromEntries(MENUS.map((m) => [m.mnemonic, menuId(m.id)]));

function menuId(id: string): string {
	return `menu:${id}`;
}

export function isEnabled(id: ActionId, state: MenuState): boolean {
	return ACTIONS[id].enabled?.(state) ?? true;
}

function item(id: ActionId, state: MenuState): SelectableMenuItem {
	const action = ACTIONS[id];
	const shortcut = action.keys?.[0];
	const base = { id, label: action.label, disabled: !isEnabled(id, state), ...(shortcut ? { shortcut } : {}) };
	return action.checked ? { type: 'checkbox', ...base, checked: action.checked(state) } : { type: 'action', ...base };
}

/** Items of several groups, one separator between neighbouring groups. */
function joinGroups(groups: Group[], state: MenuState): MenuItem[] {
	return groups.flatMap((group, index): MenuItem[] => [
		...(index > 0 ? [{ type: 'separator' as const, id: `separator:${index}` }] : []),
		...group.items.map((id) => item(id, state)),
	]);
}

/** The menu bar: one submenu per menu. */
export function buildBarMenu(state: MenuState): MenuItem[] {
	return MENUS.map((menu) => ({ type: 'submenu', id: menuId(menu.id), label: menu.label, items: joinGroups(menu.groups, state) }));
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
	return items.flatMap((i) => (i.type === 'submenu' ? selectableItems(i.items) : i.type === 'action' || i.type === 'checkbox' ? [i] : []));
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
export function actionForEvent(event: KeyEventLike, state: MenuState, inTextField: boolean): ActionId | undefined {
	for (const id of Object.keys(ACTIONS) as ActionId[]) {
		const action = ACTIONS[id];
		if (inTextField && action.textFieldOwnsKey) continue;
		if (action.keys?.some((k) => matchesShortcut(event, k)) && isEnabled(id, state)) return id;
	}
	return undefined;
}
