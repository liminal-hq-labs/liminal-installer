// What flipping a component's switch does, worded for a note beside the switch and in the details.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../../i18n/messages';
import { formatBytes } from '../../lib/media';
import type { ComponentSize } from '../../lib/types';

/** "Leaving it off frees 211,456", or "Putting it back needs 420,864 (does not fit)". Nothing for a required component. */
export function toggleNote(c: ComponentSize): string {
	if (c.required) return '';
	const bytes = formatBytes(Math.abs(c.delta_if_toggled));
	if (c.on_medium) return t('media.toggle.frees', { bytes });
	return t(c.fits_if_toggled ? 'media.toggle.needs' : 'media.toggle.needsNoFit', { bytes });
}
