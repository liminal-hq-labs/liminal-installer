// The script editor as a React component: it mounts one CodeMirror view and keeps it in step with the draft.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef } from 'react';
import { createScriptEditor, type EditorSettings, type ScriptEditor } from '../editor/scriptEditor';
import type { ScriptDiagnostic } from '../lib/types';
import type { Studio } from '../stores/studio';
import styles from './ScriptScreen.module.css';

export interface EditorEvents {
	onCursor(line: number, column: number): void;
	onDiagnostics(found: ScriptDiagnostic[]): void;
	onTabFocus(leaves: boolean): void;
}

interface Props extends EditorEvents {
	studio: Studio;
	draft: string;
	settings: EditorSettings;
	label: string;
}

/**
 * Mounts the editor once and leaves it alone while it is on screen. Typing flows out through the
 * studio's `setDraft`; a draft that changes from outside (discard, reload) flows in; the Editor
 * settings and the accessible label are applied without a new view. Give it a `key` per script, so
 * opening another script starts with a fresh undo history.
 */
export function ScriptEditorView({
	studio,
	draft,
	settings,
	label,
	onCursor,
	onDiagnostics,
	onTabFocus,
}: Props) {
	const host = useRef<HTMLDivElement>(null);
	const editor = useRef<ScriptEditor | null>(null);
	const latest = useRef({ draft, settings, label, onCursor, onDiagnostics, onTabFocus });
	latest.current = { draft, settings, label, onCursor, onDiagnostics, onTabFocus };

	useEffect(() => {
		const created = createScriptEditor(
			{
				doc: latest.current.draft,
				settings: latest.current.settings,
				label: latest.current.label,
			},
			{
				onChange: (text) => studio.setDraft(text),
				onSave: () => void studio.run(() => studio.saveScript()),
				onRun: () => void studio.run(() => studio.testScript()),
				onFormat: () => studio.formatDraft(),
				onCursor: (line, column) => latest.current.onCursor(line, column),
				onDiagnostics: (found) => latest.current.onDiagnostics(found),
				onTabFocus: (leaves) => latest.current.onTabFocus(leaves),
				check: (source) => studio.api.checkScriptSyntax(source),
			},
		);
		editor.current = created;
		studio.editor.current = created;
		created.el.classList.add(styles.scriptEditor!);
		host.current?.append(created.el);
		created.configure(latest.current.settings, latest.current.label);
		created.view.requestMeasure();
		created.focus();
		return () => {
			if (studio.editor.current === created) studio.editor.current = null;
			editor.current = null;
			created.destroy();
			created.el.remove();
		};
	}, [studio]);

	useEffect(() => {
		if (editor.current && editor.current.text() !== draft) editor.current.setText(draft);
	}, [draft]);

	const { fontSize, tabWidth, wrap, lineNumbers } = settings;
	useEffect(() => {
		editor.current?.configure({ fontSize, tabWidth, wrap, lineNumbers }, label);
	}, [fontSize, tabWidth, wrap, lineNumbers, label]);

	return <div ref={host} className={styles.editorFrame} />;
}
