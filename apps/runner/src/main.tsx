// Entry point: connects the page to the shell and mounts the window.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import './app/global.css';
import { setCatalogue, t } from './i18n/messages';
import { inTauri, tauriApi } from './lib/api';
import { createRunner } from './lib/runner';

async function boot(): Promise<void> {
	setCatalogue({});
	const container = document.getElementById('root');
	if (!container) return;
	if (!inTauri()) {
		container.textContent = t('runner.needsShell');
		return;
	}
	const runner = createRunner(await tauriApi());
	createRoot(container).render(<App runner={runner} />);
	await runner.start();
}

void boot();
