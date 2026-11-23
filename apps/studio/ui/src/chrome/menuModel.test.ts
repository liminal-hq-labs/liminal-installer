// Tests the one menu model behind the app menu and the menu bar.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	ACTIONS,
	MENUS,
	MNEMONICS,
	actionForEvent,
	buildBarMenu,
	buildFlatMenu,
	buildMenus,
	initialMenuState,
	matchesShortcut,
	selectableItems,
	type ActionId,
	type MenuState,
} from './menuModel';

const open: MenuState = { ...initialMenuState, hasProject: true };
const key = (k: string, o: Partial<{ ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean }> = {}) => ({
	key: k,
	ctrlKey: false,
	metaKey: false,
	shiftKey: false,
	altKey: false,
	...o,
});

describe('menu model', () => {
	it('builds the app menu flat, with separators and no submenus', () => {
		const flat = buildFlatMenu(open);
		expect(flat.some((i) => i.type === 'submenu')).toBe(false);
		expect(flat.some((i) => i.type === 'separator')).toBe(true);
		expect(flat[0]?.type).not.toBe('separator');
		expect(flat[flat.length - 1]?.type).not.toBe('separator');
		// No two separators in a row.
		flat.forEach((i, n) => expect(i.type === 'separator' && flat[n + 1]?.type === 'separator').toBe(false));
	});

	it('builds the bar as one submenu per menu', () => {
		const bar = buildBarMenu(open);
		expect(bar.map((m) => m.type)).toEqual(MENUS.map(() => 'submenu'));
		expect(bar.map((m) => m.type === 'submenu' && m.label)).toEqual(['File', 'Edit', 'View', 'Project', 'Help']);
	});

	it('puts the same actions in both, each once', () => {
		const { flat, bar } = buildMenus(open);
		const fromFlat = selectableItems(flat).map((i) => i.id);
		const fromBar = selectableItems(bar).map((i) => i.id);
		expect([...fromFlat].sort()).toEqual([...fromBar].sort());
		expect(new Set(fromFlat).size).toBe(fromFlat.length);
		expect(fromFlat.sort()).toEqual((Object.keys(ACTIONS) as ActionId[]).sort());
	});

	it('keeps labels, shortcuts, ticks and enabled state in step', () => {
		const state: MenuState = { ...open, canUndo: true, scriptDirty: true, screen: 'flow', menuBar: true };
		const { flat, bar } = buildMenus(state);
		const byId = (items: ReturnType<typeof selectableItems>) => Object.fromEntries(items.map((i) => [i.id, i]));
		expect(byId(selectableItems(flat))).toEqual(byId(selectableItems(bar)));
		const items = byId(selectableItems(bar));
		expect(items['edit.undo']).toMatchObject({ disabled: false, shortcut: 'Ctrl+Z' });
		expect(items['edit.redo']).toMatchObject({ disabled: true });
		expect(items['file.save']).toMatchObject({ disabled: false });
		expect(items['view.flow']).toMatchObject({ type: 'checkbox', checked: true });
		expect(items['view.layouts']).toMatchObject({ checked: false });
		expect(items['view.menuBar']).toMatchObject({ type: 'checkbox', checked: true });
	});

	it('disables project actions until a project is open', () => {
		const items = selectableItems(buildFlatMenu(initialMenuState));
		const disabled = items.filter((i) => i.disabled).map((i) => i.id);
		expect(disabled).toContain('file.reload');
		expect(disabled).toContain('project.check');
		expect(disabled).toContain('view.flow');
		expect(disabled).not.toContain('file.open');
		expect(disabled).not.toContain('view.menuBar');
		expect(disabled).not.toContain('file.quit');
		expect(disabled).not.toContain('app.preferences');
	});

	it('ends the app menu with Quit and the bar with Help', () => {
		const flat = buildFlatMenu(open);
		expect(selectableItems(flat).at(-1)?.id).toBe('file.quit');
		expect(selectableItems(buildBarMenu(open)).at(-1)?.id).toBe('help.about');
	});

	it('maps Alt letters to the bar menus', () => {
		expect(MNEMONICS).toEqual({ f: 'menu:file', e: 'menu:edit', v: 'menu:view', p: 'menu:project', h: 'menu:help' });
		const ids = buildBarMenu(open).map((m) => m.id);
		for (const target of Object.values(MNEMONICS)) expect(ids).toContain(target);
	});
});

describe('shortcuts', () => {
	it('matches modifiers exactly, treating Command as Ctrl', () => {
		expect(matchesShortcut(key('z', { ctrlKey: true }), 'Ctrl+Z')).toBe(true);
		expect(matchesShortcut(key('z', { metaKey: true }), 'Ctrl+Z')).toBe(true);
		expect(matchesShortcut(key('z', { ctrlKey: true, shiftKey: true }), 'Ctrl+Z')).toBe(false);
		expect(matchesShortcut(key('Z', { ctrlKey: true, shiftKey: true }), 'Ctrl+Shift+Z')).toBe(true);
		expect(matchesShortcut(key('F7'), 'F7')).toBe(true);
		expect(matchesShortcut(key('F7', { ctrlKey: true }), 'F7')).toBe(false);
	});

	it('runs the enabled action for a key press', () => {
		const state: MenuState = { ...open, canUndo: true, canRedo: true };
		expect(actionForEvent(key('z', { ctrlKey: true }), state, false)).toBe('edit.undo');
		expect(actionForEvent(key('Z', { ctrlKey: true, shiftKey: true }), state, false)).toBe('edit.redo');
		expect(actionForEvent(key('y', { ctrlKey: true }), state, false)).toBe('edit.redo');
		expect(actionForEvent(key('3', { ctrlKey: true }), state, false)).toBe('view.layouts');
		expect(actionForEvent(key('M', { ctrlKey: true, shiftKey: true }), initialMenuState, false)).toBe('view.menuBar');
	});

	it('ignores a disabled action', () => {
		expect(actionForEvent(key('z', { ctrlKey: true }), open, false)).toBeUndefined();
		expect(actionForEvent(key('F7'), initialMenuState, false)).toBeUndefined();
	});

	it('leaves undo and redo to a focused text field but not save', () => {
		const state: MenuState = { ...open, canUndo: true, scriptDirty: true };
		expect(actionForEvent(key('z', { ctrlKey: true }), state, true)).toBeUndefined();
		expect(actionForEvent(key('s', { ctrlKey: true }), state, true)).toBe('file.save');
	});

	it('gives no two actions the same key', () => {
		const seen = new Map<string, string>();
		for (const [id, action] of Object.entries(ACTIONS)) {
			for (const k of action.keys ?? []) {
				expect(seen.get(k), `${k} on ${id}`).toBeUndefined();
				seen.set(k, id);
			}
		}
	});
});
