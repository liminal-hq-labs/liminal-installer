// The script store: the open script, the draft in the editor and the last test run.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { ScriptReport } from '../lib/types';

export interface TestRun {
	status: 'running' | 'done' | 'error';
	target: string;
	/** The text that was run. */
	text: string;
	/** True when it differed from the saved file. */
	unsaved: boolean;
	at?: Date;
	ms?: number;
	report?: ScriptReport;
	error?: string;
}

export interface ScriptState {
	scriptPath: string;
	scriptSource: string;
	/** The text in the editor. It equals `scriptSource` until there are unsaved edits, and `scriptDirty` is whether it does not. */
	scriptDraft: string;
	scriptDirty: boolean;
	/** The target a test runs against (`os-arch:renderer`), or empty for the sample machine. */
	scriptTarget: string;
	/** The last test run on this script, which goes out of date as the draft changes. */
	testRun: TestRun | null;
	/** The hooks and capabilities the script has, from the last report. */
	report: ScriptReport | null;
}

export const initialScriptState: ScriptState = {
	scriptPath: '',
	scriptSource: '',
	scriptDraft: '',
	scriptDirty: false,
	scriptTarget: '',
	testRun: null,
	report: null,
};

export type ScriptStore = StoreApi<ScriptState>;

export const createScriptStore = (): ScriptStore =>
	createStore<ScriptState>(() => ({ ...initialScriptState }));

/** True when the run on screen no longer matches what is in the editor or the chosen target. */
export function isStale(s: Pick<ScriptState, 'testRun' | 'scriptDraft' | 'scriptTarget'>): boolean {
	const run = s.testRun;
	return (
		!!run &&
		run.status !== 'running' &&
		(run.text !== s.scriptDraft || run.target !== s.scriptTarget)
	);
}
