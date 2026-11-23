// Joins class names, skipping the ones that are off.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export const cx = (...names: (string | false | null | undefined)[]): string =>
	names.filter(Boolean).join(' ');
