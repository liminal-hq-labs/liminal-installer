// What the Script screen does to the app's state: the draft, saving, discarding, selecting and test runs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The editor's text lives in `state.scriptDraft`, so a redraw, another screen or File > Save never loses an edit.

import type { State } from '../app';
import { formatLua } from '../editor/format';
import { peekEditor, refreshScriptStatus } from '../views/script';
import { errorMessage, type Api } from './api';

export interface ScriptDeps {
	state: State;
	api: Api;
	/** True when formatting on save is on. */
	formatOnSave(): boolean;
	render(): void;
	syncChrome(): void;
	toast(text: string, error: boolean, ms?: number): void;
	/** Settles unsaved edits before they would be lost: saves or discards them and answers true, or answers false to stay. */
	guard(): Promise<boolean>;
}

export function scriptActions(d: ScriptDeps) {
	const { state, api } = d;

	/** Runs the sample report that fills the hook and capability lists. */
	async function refreshReport(): Promise<void> {
		const target = state.summary?.targets.some((t) => t.full_id === state.scriptTarget)
			? state.scriptTarget
			: '';
		try {
			state.report = await api.testScript(state.scriptPath, undefined, target || undefined);
		} catch {
			state.report = null;
		}
	}

	/** Puts new text in the draft and in the editor. */
	function replaceDraft(text: string): void {
		state.scriptDraft = text;
		state.scriptDirty = text !== state.scriptSource;
		peekEditor(state)?.setText(text);
		refreshScriptStatus(state);
		d.syncChrome();
	}

	const actions = {
		/** Forgets the open script, draft and test run, as a project switch does. */
		clearScript(): void {
			state.scriptPath = '';
			state.scriptSource = '';
			state.scriptDraft = '';
			state.scriptDirty = false;
			state.scriptTarget = '';
			state.testRun = null;
			state.report = null;
			peekEditor(state)?.setText('', true);
		},

		/** The editor's text changed. */
		setDraft(text: string): void {
			state.scriptDraft = text;
			state.scriptDirty = text !== state.scriptSource;
			refreshScriptStatus(state);
			d.syncChrome();
		},

		async loadScript(path: string): Promise<void> {
			const source = await api.readScript(path);
			const same = path === state.scriptPath;
			// Our own save comes back as a change event with the text we hold; there is nothing to reload.
			if (same && source === state.scriptSource) return;
			state.scriptPath = path;
			state.scriptSource = source;
			state.scriptDraft = source;
			state.scriptDirty = false;
			if (!same) state.testRun = null;
			if (!state.scriptTarget && state.previewTarget) state.scriptTarget = state.previewTarget;
			peekEditor(state)?.setText(source, !same);
			await refreshReport();
			d.syncChrome();
		},

		/** Opens another script, asking first when the open one has unsaved edits. */
		async selectScript(path: string): Promise<void> {
			if (path === state.scriptPath) return;
			if (state.scriptDirty && !(await d.guard())) return;
			if (state.scriptDirty) return;
			await actions.loadScript(path);
		},

		/** Saves the draft, formatted first when Format on save is on. */
		async saveScript(): Promise<void> {
			if (!state.scriptPath) return;
			if (d.formatOnSave()) {
				const editor = peekEditor(state);
				if (editor) editor.format();
				else state.scriptDraft = formatLua(state.scriptDraft);
			}
			const text = state.scriptDraft;
			await api.writeScript(state.scriptPath, text, `Edit ${state.scriptPath}`);
			state.scriptSource = text;
			state.scriptDirty = state.scriptDraft !== text;
			await refreshReport();
			refreshScriptStatus(state);
			d.syncChrome();
		},

		/** Throws the unsaved edits away. */
		discardDraft(): void {
			replaceDraft(state.scriptSource);
		},

		/** Formats the draft as an edit that can be undone. */
		formatDraft(): void {
			if (!state.scriptPath) return;
			const before = state.scriptDraft;
			const editor = peekEditor(state);
			if (editor) editor.format();
			else replaceDraft(formatLua(before));
			d.toast(state.scriptDraft === before ? 'Already formatted' : 'Formatted', false, 1800);
		},

		/** Runs the draft, saved or not, against the chosen target and shows the result in the console. */
		async testScript(): Promise<void> {
			if (!state.scriptPath || state.testRun?.status === 'running') return;
			const text = state.scriptDraft;
			const target = state.scriptTarget;
			const unsaved = text !== state.scriptSource;
			state.testRun = { status: 'running', target, text, unsaved };
			d.render();
			const started = performance.now();
			try {
				const report = await api.testScript(state.scriptPath, text, target || undefined);
				const ms = Math.max(1, Math.round(performance.now() - started));
				state.testRun = { status: 'done', target, text, unsaved, at: new Date(), ms, report };
				state.report = report;
				const failed =
					(report.tests ?? []).filter((x) => !x.ok).length ||
					report.diagnostics.filter((x) => x.severity === 'error').length;
				if (report.ok) d.toast(`Test passed${target ? ` on ${target}` : ''}`, false, 3000);
				else d.toast(`Test failed: ${failed} problem${failed === 1 ? '' : 's'}`, true, 5000);
			} catch (e) {
				const error = errorMessage(e);
				state.testRun = {
					status: 'error',
					target,
					text,
					unsaved,
					at: new Date(),
					ms: Math.round(performance.now() - started),
					error,
				};
				d.toast(`Test could not run: ${error}`, true, 6000);
			}
		},
	};
	return actions;
}
