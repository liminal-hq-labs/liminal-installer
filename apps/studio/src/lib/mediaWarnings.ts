// Every warning code a medium can have, in the order the specification lists them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MediaWarningCode } from './types';

/** The codes of `li-media`'s `MediaWarningCode`. A test fails if one is missing from `warningTab`. */
export const MediaWarningCodes: readonly MediaWarningCode[] = [
	'does_not_fit',
	'below_reserve',
	'default_on_later_disk',
	'win16_window',
	'label',
	'dir_not_8_3',
	'autorun_target',
	'runtime_estimated',
	'runtime_lacks',
	'name_83',
];
