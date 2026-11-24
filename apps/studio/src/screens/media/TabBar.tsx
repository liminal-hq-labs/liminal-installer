// The tabs of a medium's detail pane, as a `tablist` the arrow keys move along.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { KeyboardEvent } from 'react';
import { t, type MessageId } from '../../i18n/messages';
import type { MediaTab } from '../../stores/media';
import { cx } from '../../ui/cx';
import ui from '../../ui/ui.module.css';

export interface TabSpec {
	id: MediaTab;
	label: MessageId;
}

export const tabId = (id: MediaTab): string => `media-tab-${id}`;
export const panelId = (id: MediaTab): string => `media-panel-${id}`;

export interface TabBarProps {
	tabs: TabSpec[];
	current: MediaTab;
	onSelect(tab: MediaTab): void;
}

export function TabBar({ tabs, current, onSelect }: TabBarProps) {
	const keys = (e: KeyboardEvent<HTMLElement>) => {
		const at = tabs.findIndex((x) => x.id === current);
		// In right-to-left text the next tab is on the left.
		const rtl = document.documentElement.dir === 'rtl';
		const step: Record<string, number> = {
			ArrowRight: rtl ? -1 : 1,
			ArrowLeft: rtl ? 1 : -1,
		};
		let next = step[e.key] === undefined ? -1 : (at + step[e.key]! + tabs.length) % tabs.length;
		if (e.key === 'Home') next = 0;
		if (e.key === 'End') next = tabs.length - 1;
		if (next < 0) return;
		e.preventDefault();
		const target = tabs[next]!;
		onSelect(target.id);
		queueMicrotask(() => document.getElementById(tabId(target.id))?.focus());
	};
	return (
		<div role="tablist" aria-label={t('media.tabs.label')} className={ui.tabs} onKeyDown={keys}>
			{tabs.map((x) => (
				<button
					key={x.id}
					type="button"
					role="tab"
					id={tabId(x.id)}
					aria-selected={x.id === current}
					aria-controls={panelId(x.id)}
					tabIndex={x.id === current ? 0 : -1}
					className={cx(ui.tab, x.id === current && ui.active)}
					onClick={() => onSelect(x.id)}
				>
					{t(x.label)}
				</button>
			))}
		</div>
	);
}
