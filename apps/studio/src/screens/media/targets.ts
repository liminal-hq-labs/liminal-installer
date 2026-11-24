// The names a person knows the targets of a medium by.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, type MessageId } from '../../i18n/messages';

const NAMES: Record<string, MessageId> = {
	'dos-x86': 'media.target.dos-x86',
	'win16-x86': 'media.target.win16-x86',
	'windows-x86': 'media.target.windows-x86',
};

/** A target's name in words, or its id when it has none. */
export const targetName = (id: string): string => (NAMES[id] ? t(NAMES[id]) : id);

/** Several targets as a list: `DOS, Windows 3.x`. */
export const targetNames = (ids: readonly string[]): string => ids.map(targetName).join(', ');
