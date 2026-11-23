// @vitest-environment happy-dom
// Tests the Keyboard shortcuts, Connect an agent and New project dialogs over a fake host.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { defaultSettings } from '../lib/settings';
import { testHost } from '../preferences/testPort';
import { AgentBody, NewProjectBody, ShortcutsBody, clientConfig } from './DialogPages';
import { CREATED_PREFIX } from './dialogWindow';
import { ACTIONS, shortcutTable } from './menuModel';

afterEach(cleanup);

describe('Keyboard shortcuts', () => {
	it('lists every command that has a key, from the one menu model', () => {
		const listed = shortcutTable().flatMap((g) => g.rows.map((r) => r.id));
		const keyed = (Object.keys(ACTIONS) as (keyof typeof ACTIONS)[]).filter(
			(id) => ACTIONS[id].keys?.length,
		);
		expect(listed.sort()).toEqual(keyed.sort());
		expect(listed).toContain('file.new');
		render(<ShortcutsBody />);
		expect(screen.getByText('New project…')).toBeTruthy();
		expect(screen.getAllByText('Ctrl+N').length).toBe(1);
	});
});

describe('Connect an agent', () => {
	it('shows the endpoint and a sample configuration without the token', async () => {
		render(<AgentBody host={testHost()} />);
		expect(await screen.findByDisplayValue('http://127.0.0.1:5000/mcp')).toBeTruthy();
		const snippet = document.querySelector('.snippet')!.textContent!;
		expect(snippet).toContain('"url": "http://127.0.0.1:5000/mcp"');
		expect(snippet).toContain('Bearer YOUR_TOKEN');
		expect(snippet).not.toContain('secret-token');
	});

	it('copies the URL, the token and a configuration holding the real token', async () => {
		const copy = vi.fn(async () => undefined);
		render(<AgentBody host={testHost({ copy })} />);
		await screen.findByDisplayValue('http://127.0.0.1:5000/mcp');
		fireEvent.click(screen.getByRole('button', { name: 'Copy URL' }));
		await waitFor(() => expect(copy).toHaveBeenLastCalledWith('http://127.0.0.1:5000/mcp'));
		fireEvent.click(screen.getByRole('button', { name: 'Copy token' }));
		await waitFor(() => expect(copy).toHaveBeenLastCalledWith('secret-token'));
		fireEvent.click(screen.getByRole('button', { name: 'Copy configuration' }));
		await waitFor(() =>
			expect(copy).toHaveBeenLastCalledWith(
				clientConfig('http://127.0.0.1:5000/mcp', 'secret-token'),
			),
		);
	});

	it('says why the server is not running', async () => {
		render(
			<AgentBody
				host={testHost({ agent: async () => null, agentError: async () => 'port 4000 is in use' })}
			/>,
		);
		expect(await screen.findByText(/port 4000 is in use/)).toBeTruthy();
	});
});

describe('New project', () => {
	const fill = (label: string, value: string) =>
		fireEvent.change(screen.getByLabelText(label), { target: { value } });

	it('suggests an id, creates the project in a folder named after it and answers with the folder', async () => {
		const newProject = vi.fn(async (folder: string) => folder);
		const onAnswer = vi.fn();
		render(
			<NewProjectBody
				host={testHost({ newProject })}
				settings={{ ...defaultSettings, projectsFolder: '/home/me/projects' }}
				onAnswer={onAnswer}
			/>,
		);
		fill('Name', 'Fizz Level');
		expect((screen.getByLabelText('Id') as HTMLInputElement).value).toBe('example.fizzlevel');
		fill('Publisher', 'Liminal HQ');
		fireEvent.click(screen.getByRole('button', { name: 'Create project' }));
		await waitFor(() =>
			expect(onAnswer).toHaveBeenCalledWith(`${CREATED_PREFIX}/home/me/projects/fizz-level`),
		);
		expect(newProject).toHaveBeenCalledWith(
			'/home/me/projects/fizz-level',
			'Fizz Level',
			'example.fizzlevel',
			'Liminal HQ',
		);
	});

	it('keeps the form open and shows the reason when the shell refuses', async () => {
		const onAnswer = vi.fn();
		render(
			<NewProjectBody
				host={testHost({
					newProject: async () => {
						throw new Error('id: use reverse-DNS form');
					},
				})}
				settings={{ ...defaultSettings, projectsFolder: '/p' }}
				onAnswer={onAnswer}
			/>,
		);
		fill('Name', 'Demo');
		fireEvent.click(screen.getByRole('button', { name: 'Create project' }));
		expect((await screen.findByRole('alert')).textContent).toContain('reverse-DNS');
		expect(onAnswer).not.toHaveBeenCalled();
	});

	it('cannot create until it has a name and a folder, and cancel answers none', () => {
		const onAnswer = vi.fn();
		render(<NewProjectBody host={testHost()} settings={defaultSettings} onAnswer={onAnswer} />);
		expect(
			(screen.getByRole('button', { name: 'Create project' }) as HTMLButtonElement).disabled,
		).toBe(true);
		fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(onAnswer).toHaveBeenCalledWith(null);
	});
});
