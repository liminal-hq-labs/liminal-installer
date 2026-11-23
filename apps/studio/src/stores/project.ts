// The project store: the open project and the documents and selections tied to it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { FlowView, HistoryView, Layout, McpInfo, Problem, ProjectSummary } from '../lib/types';

export interface ProjectState {
	summary: ProjectSummary | null;
	history: HistoryView;
	mcp: McpInfo | null;
	/** Why the agent server is off although it is switched on. */
	mcpError: string | null;
	problems: Problem[];
	flowId: string;
	flow: FlowView | null;
	selectedStep: string | null;
	connectFrom: string | null;
	layoutId: string;
	layout: Layout | null;
	layoutPath: number[];
	overrideRenderer: string | null;
	/** The renderer the Layouts screen previews with. */
	layoutRenderer: string;
	showHistory: boolean;
	/** Counts requests to bring the Overview's checks list into view. */
	checksRequest: number;
}

export const initialProjectState: ProjectState = {
	summary: null,
	history: { entries: [], can_undo: false, can_redo: false },
	mcp: null,
	mcpError: null,
	problems: [],
	flowId: 'install',
	flow: null,
	selectedStep: null,
	connectFrom: null,
	layoutId: 'welcome',
	layout: null,
	layoutPath: [],
	overrideRenderer: null,
	layoutRenderer: 'canvas',
	showHistory: false,
	checksRequest: 0,
};

export type ProjectStore = StoreApi<ProjectState>;

export const createProjectStore = (): ProjectStore =>
	createStore<ProjectState>(() => ({ ...initialProjectState }));
