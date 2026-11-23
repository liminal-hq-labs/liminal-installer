// @vitest-environment happy-dom
// Tests the keyboard path through the menu bar and the app menu built from the one model.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MenuBar } from '@liminal-hq/studio-chrome/MenuBar/MenuBar';
import { AppMenuButton } from '@liminal-hq/studio-chrome/TitleBar/AppMenuButton';
import { MNEMONICS, buildMenus, initialMenuState } from './menuModel';

const state = { ...initialMenuState, hasProject: true, canUndo: true };

afterEach(cleanup);

function bar(onSelect = vi.fn()) {
	render(
		<MenuBar
			items={buildMenus(state).bar}
			onSelect={onSelect}
			label="Menu bar"
			moreLabel="More"
			mnemonics={MNEMONICS}
		/>,
	);
	return onSelect;
}

describe('menu bar keyboard', () => {
	it('moves along the bar with the arrow keys', () => {
		bar();
		const file = screen.getByRole('menuitem', { name: 'File' });
		file.focus();
		fireEvent.keyDown(file, { key: 'ArrowRight' });
		expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Edit' }));
		fireEvent.keyDown(document.activeElement!, { key: 'End' });
		expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Help' }));
	});

	it('opens a menu with Down, picks an item with Enter, and closes with Escape', () => {
		const onSelect = bar();
		const edit = screen.getByRole('menuitem', { name: 'Edit' });
		edit.focus();
		fireEvent.keyDown(edit, { key: 'ArrowDown' });
		expect(screen.getByRole('menu', { name: 'Edit' })).toBeTruthy();
		fireEvent.keyDown(screen.getByRole('menu', { name: 'Edit' }), { key: 'Enter' });
		expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: 'edit.undo' }));
		expect(screen.queryByRole('menu')).toBeNull();

		fireEvent.keyDown(edit, { key: 'ArrowDown' });
		fireEvent.keyDown(screen.getByRole('menu', { name: 'Edit' }), { key: 'Escape' });
		expect(screen.queryByRole('menu')).toBeNull();
	});

	it('opens a menu with Alt and its letter', () => {
		bar();
		act(() => {
			window.dispatchEvent(
				new KeyboardEvent('keydown', { key: 'v', altKey: true, bubbles: true, cancelable: true }),
			);
		});
		expect(screen.getByRole('menu', { name: 'View' })).toBeTruthy();
	});

	it('focuses the bar on F10 and gives focus back on a second press', () => {
		bar();
		const outside = document.createElement('button');
		document.body.append(outside);
		outside.focus();
		act(() => {
			window.dispatchEvent(
				new KeyboardEvent('keydown', { key: 'F10', bubbles: true, cancelable: true }),
			);
		});
		expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'File' }));
		act(() => {
			window.dispatchEvent(
				new KeyboardEvent('keydown', { key: 'F10', bubbles: true, cancelable: true }),
			);
		});
		expect(document.activeElement).toBe(outside);
		outside.remove();
	});
});

describe('app menu keyboard', () => {
	it('opens on F10 as one flat list and not on F10 when accelerators are off', () => {
		const onSelect = vi.fn();
		const { rerender } = render(
			<AppMenuButton
				label="App"
				items={buildMenus(state).flat}
				onSelect={onSelect}
				acceleratorKeys
			/>,
		);
		act(() => {
			window.dispatchEvent(
				new KeyboardEvent('keydown', { key: 'F10', bubbles: true, cancelable: true }),
			);
		});
		const menu = screen.getByRole('menu', { name: 'App' });
		expect(menu).toBeTruthy();
		// Open recent is the one submenu; every other row is direct.
		expect(
			screen
				.queryAllByRole('menuitem')
				.filter((i) => i.getAttribute('aria-haspopup') === 'menu')
				.map((i) => i.textContent),
		).toEqual([expect.stringContaining('Open recent')]);
		expect(screen.getByRole('menuitem', { name: /Open project/ })).toBeTruthy();
		fireEvent.keyDown(menu, { key: 'Escape' });
		expect(screen.queryByRole('menu')).toBeNull();

		rerender(
			<AppMenuButton
				label="App"
				items={buildMenus(state).flat}
				onSelect={onSelect}
				acceleratorKeys={false}
			/>,
		);
		act(() => {
			window.dispatchEvent(
				new KeyboardEvent('keydown', { key: 'F10', bubbles: true, cancelable: true }),
			);
		});
		expect(screen.queryByRole('menu')).toBeNull();
	});
});
