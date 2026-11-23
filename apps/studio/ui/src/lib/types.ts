// Types mirroring the Rust serde types the backend sends.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export type StepType = 'page' | 'action' | 'branch' | 'commit' | 'finish';

export interface Branch {
	when: string;
	next: string;
}

export interface Step {
	id: string;
	type: StepType;
	layout?: string;
	when?: string;
	on_enter?: string;
	next?: string;
	branches?: Branch[];
}

export interface Flow {
	$schema?: string;
	version: number;
	id: string;
	start?: string;
	steps: Step[];
}

export interface LayoutNode {
	kind: string;
	bind?: string;
	props?: Record<string, unknown>;
	children?: LayoutNode[];
	overrides?: Record<string, LayoutNode>;
}

export interface Layout {
	$schema?: string;
	version: number;
	id: string;
	root: LayoutNode;
}

export type Severity = 'error' | 'warning';

export interface Problem {
	severity: Severity;
	scope: string;
	message: string;
	step?: string;
}

export interface TargetInfo {
	full_id: string;
	id: string;
	renderer: string;
	theme: string | null;
	outputs: string[];
	components: string[];
}

export interface ProjectSummary {
	root: string;
	id: string;
	name: string;
	publisher: string;
	version: string;
	default_theme: string | null;
	targets: TargetInfo[];
	components: { id: string; name: string; required: boolean; targets: string[]; files: number }[];
	themes: { id: string; name: string; renderers: string[] }[];
	flows: { id: string; steps: number; errors: number }[];
	layouts: string[];
	scripts: string[];
}

export interface FlowView {
	flow: Flow;
	errors: Problem[];
}

export type Source = 'ui' | 'mcp';

export interface HistoryEntry {
	index: number;
	label: string;
	source: Source;
	doc: { type: 'flow' | 'layout' | 'script'; id: string };
	applied: boolean;
}

export interface HistoryView {
	entries: HistoryEntry[];
	can_undo: boolean;
	can_redo: boolean;
}

export interface ChangeEvent {
	kind: 'open' | 'edit' | 'undo' | 'redo';
	doc: { type: 'flow' | 'layout' | 'script'; id: string } | null;
	label: string;
	source: Source;
	can_undo: boolean;
	can_redo: boolean;
}

export type Operation = { op: string; [key: string]: unknown };

export interface Screenshot {
	mime: string;
	encoding: 'utf8' | 'base64';
	data: string;
}

export interface PreviewStep {
	id: string;
	kind: string;
	layout: string | null;
	skipped: boolean;
	screenshot: Screenshot | null;
}

export interface Preview {
	target: string;
	plan: { ops: Operation[] };
	problems: Problem[];
	steps: PreviewStep[];
}

export interface HookInfo {
	name: string;
	when: string;
	present: boolean;
}

export interface ScriptReport {
	ok: boolean;
	hooks: HookInfo[];
	capabilities: string[];
	diagnostics: Problem[];
	output: string[];
}

export interface McpInfo {
	url: string;
	token: string;
}
