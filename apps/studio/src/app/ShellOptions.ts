// What the page around the app decides: whether the app draws its own header.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext } from 'react';

export interface ShellOptions {
	/** A browser tab has no title bar, so the app draws a header of its own. */
	header: boolean;
}

export const ShellOptionsContext = createContext<ShellOptions>({ header: false });

export const useShellOptions = (): ShellOptions => useContext(ShellOptionsContext);
