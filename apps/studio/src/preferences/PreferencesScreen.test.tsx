// @vitest-environment happy-dom
// Tests the Preferences screen end to end over a fake port: the sections, applying changes, refusals and the Escape guard.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { PreferencesScreen } from './PreferencesScreen';
import { t } from '../i18n/messages';
import { SECTION_IDS, SECTION_LABELS } from './sections';
import { testHost, testPort, type TestPort } from './testPort';
import type { Host } from '../lib/host';

afterEach(cleanup);

async function open(
	port: TestPort = testPort(),
	host: Host = testHost(),
	targets: string[] = ['dos-x86:tui-retro', 'linux-x64:canvas'],
) {
	const view = render(
		<PreferencesScreen port={port} host={host} loadTargets={async () => targets} />,
	);
	await screen.findByRole('navigation', { name: 'Preferences sections' });
	return { port, host, ...view };
}

const go = async (name: string) => {
	fireEvent.click(
		within(screen.getByRole('navigation', { name: 'Preferences sections' })).getByRole('button', {
			name,
		}),
	);
	await screen.findByRole('heading', { level: 2, name });
};

describe('sections', () => {
	it('lists them in order, each with its label', async () => {
		await open();
		const nav = screen.getByRole('navigation', { name: 'Preferences sections' });
		expect(
			within(nav)
				.getAllByRole('button')
				.map((b) => b.textContent),
		).toEqual(SECTION_IDS.map((id) => t(SECTION_LABELS[id])));
		expect(SECTION_IDS).toEqual([
			'general',
			'appearance',
			'editor',
			'preview',
			'agents',
			'projects',
			'advanced',
		]);
	});

	it('draws every section without error and with a named control per row', async () => {
		await open();
		for (const id of SECTION_IDS) {
			await go(t(SECTION_LABELS[id]));
			const main = screen.getByRole('main');
			expect(within(main).getAllByRole('group').length).toBeGreaterThan(0);
			expect(main.querySelectorAll('input, select, button').length).toBeGreaterThan(1);
			for (const el of [
				...screen.queryAllByRole('switch'),
				...screen.queryAllByRole('spinbutton'),
				...screen.queryAllByRole('textbox'),
				...screen.queryAllByRole('combobox'),
			]) {
				if (el.closest('nav')) continue;
				const named =
					el.getAttribute('aria-labelledby') ||
					el.getAttribute('aria-label') ||
					document.querySelector(`label[for="${el.id}"]`);
				expect(named, `${id}: ${el.outerHTML}`).toBeTruthy();
			}
		}
	});
});

describe('applying changes', () => {
	it('writes a toggle as a patch at once', async () => {
		const { port } = await open();
		fireEvent.click(screen.getByRole('switch', { name: 'Show menu bar' }));
		await waitFor(() => expect(port.current.menuBar).toBe(true));
		expect(port.writes).toEqual([{ menuBar: true }]);
	});

	it('follows a change made in another window', async () => {
		const { port } = await open();
		expect(screen.getByRole('switch', { name: 'Show menu bar' }).getAttribute('aria-checked')).toBe(
			'false',
		);
		act(() => port.external({ menuBar: true }));
		expect(screen.getByRole('switch', { name: 'Show menu bar' }).getAttribute('aria-checked')).toBe(
			'true',
		);
	});

	it('applies a number when the box is left, and not while it is typed', async () => {
		const { port } = await open();
		await go('Editor');
		const box = screen.getByRole('spinbutton', { name: 'Font size' });
		fireEvent.change(box, { target: { value: '1' } });
		fireEvent.change(box, { target: { value: '18' } });
		expect(port.writes).toEqual([]);
		fireEvent.blur(box);
		await waitFor(() => expect(port.current.editorFontSize).toBe(18));
		expect(port.writes).toEqual([{ editorFontSize: 18 }]);
	});

	it('keeps a number within its range', async () => {
		const { port } = await open();
		await go('Appearance');
		const box = screen.getByRole('spinbutton', { name: 'Interface text size' });
		fireEvent.change(box, { target: { value: '500' } });
		fireEvent.keyDown(box, { key: 'Enter' });
		await waitFor(() => expect(port.current.textSize).toBe(130));
	});

	it('restores the value and shows the reason on the row when a write is refused', async () => {
		const { port } = await open();
		await go('Agents');
		port.refuseNext('port 80 is in use');
		const box = screen.getByRole('spinbutton', { name: 'Port' });
		fireEvent.change(box, { target: { value: '80' } });
		fireEvent.blur(box);
		const alert = await screen.findByRole('alert');
		expect(alert.textContent).toBe('Not saved: port 80 is in use');
		expect((screen.getByRole('spinbutton', { name: 'Port' }) as HTMLInputElement).value).toBe('0');
		expect(screen.getByRole('spinbutton', { name: 'Port' }).getAttribute('aria-invalid')).toBe(
			'true',
		);
	});

	it('applies a text box when it is left and trims it', async () => {
		const { port } = await open(testPort(), testHost());
		await go('Projects');
		const box = screen.getByRole('textbox', { name: 'Projects folder' });
		fireEvent.change(box, { target: { value: ' /work ' } });
		fireEvent.blur(box);
		await waitFor(() => expect(port.current.projectsFolder).toBe('/work'));
	});

	it('shows a refused path with its reason', async () => {
		const { port } = await open();
		await go('Projects');
		port.refuseNext('the runner did not answer `--version`: No such file');
		const box = screen.getByRole('textbox', { name: 'Runner' });
		fireEvent.change(box, { target: { value: '/nope' } });
		fireEvent.keyDown(box, { key: 'Enter' });
		expect((await screen.findByRole('alert')).textContent).toContain('the runner did not answer');
		expect((screen.getByRole('textbox', { name: 'Runner' }) as HTMLInputElement).value).toBe('');
	});
});

describe('Escape', () => {
	it('puts back unapplied text and leaves the next Escape to close the window', async () => {
		const { port } = await open();
		await go('Projects');
		const onWindowKey = vi.fn((e: KeyboardEvent) => e.key === 'Escape' && !e.defaultPrevented);
		window.addEventListener('keydown', onWindowKey);
		const box = screen.getByRole('textbox', { name: 'Projects folder' }) as HTMLInputElement;
		box.focus();
		fireEvent.change(box, { target: { value: '/typed' } });
		fireEvent.input(box, { target: { value: '/typed' } });
		fireEvent.keyDown(box, { key: 'Escape' });
		expect(onWindowKey).toHaveBeenCalled();
		expect(onWindowKey.mock.results[0]!.value).toBe(false);
		expect(
			(screen.getByRole('textbox', { name: 'Projects folder' }) as HTMLInputElement).value,
		).toBe('');
		expect(port.writes).toEqual([]);
		fireEvent.keyDown(screen.getByRole('textbox', { name: 'Projects folder' }), { key: 'Escape' });
		expect(onWindowKey.mock.results[1]!.value).toBe(true);
		window.removeEventListener('keydown', onWindowKey);
	});

	it('drops an unapplied number the same way', async () => {
		const { port } = await open();
		await go('Editor');
		const box = screen.getByRole('spinbutton', { name: 'Tab width' }) as HTMLInputElement;
		box.focus();
		fireEvent.change(box, { target: { value: '7' } });
		fireEvent.input(box, { target: { value: '7' } });
		fireEvent.keyDown(box, { key: 'Escape' });
		expect((screen.getByRole('spinbutton', { name: 'Tab width' }) as HTMLInputElement).value).toBe(
			'4',
		);
		expect(port.writes).toEqual([]);
	});
});

describe('pages', () => {
	it('clears the recent projects', async () => {
		const { port } = await open(testPort({ recentProjects: ['/a', '/b'] }));
		expect(screen.getByText('2 projects in File > Open recent.')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Clear recent projects' }));
		await waitFor(() => expect(port.current.recentProjects).toEqual([]));
		await waitFor(() =>
			expect(
				(screen.getByRole('button', { name: 'Clear recent projects' }) as HTMLButtonElement)
					.disabled,
			).toBe(true),
		);
	});

	it('offers the theme choices, including following the system', async () => {
		const { port } = await open();
		await go('Appearance');
		const select = screen.getByRole('combobox', { name: 'Theme' });
		expect(
			within(select)
				.getAllByRole('option')
				.map((o) => o.textContent),
		).toEqual(['Afterglow dark', 'Afterglow light', 'Follow system', 'Afterglow dark, neutral']);
		fireEvent.change(select, { target: { value: 'system' } });
		await waitFor(() => expect(port.current.theme).toBe('system'));
	});

	it('lists the project targets for the dry-run default', async () => {
		const { port } = await open();
		await go('Preview');
		const select = await screen.findByRole('combobox', { name: 'Default target' });
		await waitFor(() => expect(within(select).getAllByRole('option')).toHaveLength(3));
		fireEvent.change(select, { target: { value: 'linux-x64:canvas' } });
		await waitFor(() => expect(port.current.previewTarget).toBe('linux-x64:canvas'));
	});

	it('keeps the token hidden until asked, copies it and regenerates it after a confirmation', async () => {
		const copy = vi.fn(async () => undefined);
		const confirm = vi.fn(async () => 'regenerate');
		const host = testHost({ copy, confirm });
		await open(testPort(), host);
		await go('Agents');
		await screen.findByText('http://127.0.0.1:5000/mcp');
		expect(screen.queryByText('secret-token')).toBeNull();
		fireEvent.click(screen.getByRole('button', { name: 'Show token' }));
		expect(await screen.findByText('secret-token')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Copy' }));
		await waitFor(() => expect(copy).toHaveBeenCalledWith('secret-token'));
		fireEvent.click(screen.getByRole('button', { name: 'Regenerate' }));
		expect(await screen.findByText('fresh-token')).toBeTruthy();
		expect(confirm).toHaveBeenCalledTimes(1);
	});

	it('does not regenerate when the confirmation is declined', async () => {
		const regenerateToken = vi.fn(async () => 'fresh-token');
		await open(testPort(), testHost({ confirm: async () => 'cancel', regenerateToken }));
		await go('Agents');
		await screen.findByText('http://127.0.0.1:5000/mcp');
		fireEvent.click(screen.getByRole('button', { name: 'Regenerate' }));
		await act(async () => undefined);
		expect(regenerateToken).not.toHaveBeenCalled();
	});

	it('disables the token buttons while the server is off', async () => {
		await open(testPort(), testHost({ agent: async () => null, agentToken: async () => null }));
		await go('Agents');
		await screen.findByText('The server is off.');
		expect((screen.getByRole('button', { name: 'Show token' }) as HTMLButtonElement).disabled).toBe(
			true,
		);
	});

	it('browses for a folder and applies the pick', async () => {
		const browse = vi.fn(async () => '/picked');
		const { port } = await open(testPort(), testHost({ browse }));
		await go('Projects');
		fireEvent.click(screen.getAllByRole('button', { name: 'Browse…' })[0]!);
		await waitFor(() => expect(port.current.projectsFolder).toBe('/picked'));
		expect(browse).toHaveBeenCalledWith('folder', 'Choose the projects folder', undefined);
	});

	it('resets everything only after a confirmation', async () => {
		const confirm = vi.fn(async () => 'cancel');
		const { port } = await open(testPort({ tabWidth: 2 }), testHost({ confirm }));
		await go('Advanced');
		fireEvent.click(screen.getByRole('button', { name: 'Reset all…' }));
		await waitFor(() => expect(confirm).toHaveBeenCalled());
		expect(port.current.tabWidth).toBe(2);
		confirm.mockResolvedValue('reset');
		fireEvent.click(screen.getByRole('button', { name: 'Reset all…' }));
		await waitFor(() => expect(port.current.tabWidth).toBe(4));
	});

	it('reveals the settings file and shows a refusal on its row', async () => {
		const reveal = vi.fn(async (what: string) => {
			if (what === 'logs') throw new Error('no file manager');
		});
		await open(testPort(), testHost({ reveal }));
		await go('Advanced');
		expect(await screen.findByText('/cfg/settings.json')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Open location' }));
		expect(reveal).toHaveBeenCalledWith('settings');
		fireEvent.click(screen.getByRole('button', { name: 'Show location' }));
		expect((await screen.findByRole('alert')).textContent).toBe('no file manager');
		expect(screen.getByText('Liminal Installer Studio 9.9.9')).toBeTruthy();
	});
});
