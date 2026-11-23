// The runner window: the project's current page drawn by the shared renderer, with keys and announcements.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { PreviewTree, fill, type LayoutNode, type Live } from '@liminal-hq/installer-renderer';
import { useEffect, useMemo, useRef } from 'react';
import { useStore } from 'zustand';
import { t } from '../i18n/messages';
import { backAction, enterAction, escapeAction, FIRST_FOCUS, pathsOf } from '../lib/keys';
import type { Runner } from '../lib/runner';
import { synthScreen, type SynthKind } from '../lib/synth';
import styles from './App.module.css';

/** The heading a screen reader hears when a page appears: the page window's label or title. */
function pageTitle(root: LayoutNode, vars: Record<string, string>): string {
	const stack = [root];
	while (stack.length > 0) {
		const node = stack.shift()!;
		if (node.kind === 'window' || node.kind === 'dialog') {
			const p = node.props ?? {};
			const title = p.label ?? p.title;
			if (typeof title === 'string' && title) return fill(title, vars);
		}
		stack.push(...(node.children ?? []));
	}
	return '';
}

export function App({ runner }: { runner: Runner }) {
	const s = useStore(runner.store);
	const stage = useRef<HTMLDivElement>(null);
	const { info, page } = s;

	const synth: SynthKind | null = !info
		? null
		: s.phase === 'failed'
			? 'failed'
			: s.phase === 'done'
				? 'done'
				: page
					? null
					: 'status';
	const tree = useMemo<LayoutNode | null>(() => {
		if (!info) return null;
		if (synth) return synthScreen(synth, info, s.message);
		return page?.layout ?? null;
	}, [info, synth, page, s.message]);
	const paths = useMemo(() => (tree ? pathsOf(tree) : new Map<LayoutNode, string>()), [tree]);
	const off = useMemo(() => new Set(s.disabled), [s.disabled]);

	// Before the commit reports anything the meters read nothing used, not the renderer's samples.
	const live: Live = {
		freeSpace: s.progress?.free_fraction ?? 1,
		file: s.progress?.file_fraction ?? 0,
		total: s.progress?.total ?? 0,
	};
	const announced = useMemo(() => {
		if (!tree) return '';
		const title = pageTitle(tree, s.vars);
		const problems = page?.problems ?? [];
		return problems.length > 0
			? t('runner.announce.problems', { problems: problems.join(', ') })
			: title
				? t('runner.announce.page', { title })
				: '';
	}, [tree, s.vars, page]);
	const tenth = s.progress ? Math.floor(s.progress.total * 10) * 10 : null;

	// A new page takes focus on its first control, so the keyboard starts where the page does.
	const key = page ? `${page.step}:${page.index}` : (synth ?? 'none');
	useEffect(() => {
		const el = stage.current?.querySelector<HTMLElement>(FIRST_FOCUS);
		el?.focus({ preventScroll: true });
	}, [key]);

	useEffect(() => {
		if (!tree) return;
		const onKey = (e: KeyboardEvent) => {
			// A control that took the key (Enter on a choice) has already acted; Ctrl+Enter is ours anyway.
			if (e.defaultPrevented && !(e.key === 'Enter' && e.ctrlKey)) return;
			const target = e.target instanceof Element ? e.target : null;
			if (e.key === 'Enter' && !e.altKey && !e.metaKey) {
				// A focused control acts for itself; Enter elsewhere, and Ctrl+Enter anywhere, presses the default button.
				if (
					!e.ctrlKey &&
					target?.closest('button, a, select, textarea, [role=radio], [role=checkbox]')
				)
					return;
				const action = enterAction(tree, off);
				if (action) {
					e.preventDefault();
					runner.act(action);
				}
			} else if (e.key === 'Escape' && page?.kind !== 'commit') {
				const action = escapeAction(tree, off);
				if (action) {
					e.preventDefault();
					runner.act(action);
				}
			} else if (e.key === 'ArrowLeft' && e.altKey) {
				const action = backAction(tree, off);
				if (action) {
					e.preventDefault();
					runner.act(action);
				}
			}
		};
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	}, [tree, off, page, runner]);

	if (!info || !tree) {
		return (
			<main className={styles.root} aria-busy="true">
				<p role="status" className={styles.starting}>
					{s.phase === 'failed' ? s.message : t('runner.starting')}
				</p>
			</main>
		);
	}
	return (
		<main className={styles.root} aria-label={t('runner.window', { name: info.product.name })}>
			<div className={styles.stage} ref={stage}>
				<PreviewTree
					root={tree}
					options={{
						renderer: info.renderer,
						theme: info.theme,
						vars: s.vars,
						state: s.values,
						components: info.components,
						live,
						fit: 'contain',
						scheme: 'system',
						interact: {
							mode: 'run',
							onChange: runner.change,
							onAction: runner.act,
							isDisabled: (node) => {
								const path = paths.get(node);
								return path !== undefined && off.has(path);
							},
						},
					}}
				/>
			</div>
			<div className={styles.live} role="status" aria-live="polite" aria-atomic="true">
				{s.phase === 'cancelled'
					? t('runner.announce.cancelled')
					: tenth !== null && page?.kind === 'commit'
						? t('runner.announce.progress', { percent: tenth })
						: announced}
			</div>
		</main>
	);
}
