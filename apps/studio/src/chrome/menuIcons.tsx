// The icons the menus show beside their items. Not every item has one.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode } from 'react';
import { ICON_PATHS, type IconName } from '../ui/iconPaths';

/** The chrome's menu rows reserve a 16px slot, so these draw at 16px in the text colour. */
export function MenuIcon({ name }: { name: IconName }) {
	return (
		<svg
			viewBox="0 0 16 16"
			width="16"
			height="16"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.25"
			strokeLinecap="round"
			strokeLinejoin="round"
			aria-hidden="true"
			focusable="false"
		>
			<path d={ICON_PATHS[name]} />
		</svg>
	);
}

export function menuIcon(name: IconName): ReactNode {
	return <MenuIcon name={name} />;
}
