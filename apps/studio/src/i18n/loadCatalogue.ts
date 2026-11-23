// Loads the catalogue for the locale a page asks for, and sets the document's language and direction.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { setActiveLocale, type Catalogue } from './active';
import { FALLBACK_LOCALE, isRightToLeftLocale, resolveLocale, type Locale } from './locales';

/** The catalogue for `locale`. English is the source and needs none; a pseudo-locale is generated. */
async function catalogueFor(locale: Locale): Promise<Catalogue> {
	if (locale === 'en-XA') return (await import('./catalogues/en-XA')).default;
	if (locale === 'ar-XB') return (await import('./catalogues/ar-XB')).default;
	return {};
}

/**
 * Puts the locale in force. The development build accepts `?locale=en-XA` and `?locale=ar-XB` in the
 * address; anything else, and every release build, is English.
 */
export async function loadLocale(
	search: string,
	developer: boolean,
	root?: HTMLElement,
): Promise<Locale> {
	const requested = new URLSearchParams(search).get('locale');
	const locale = resolveLocale(requested, developer);
	setActiveLocale(locale, await catalogueFor(locale));
	if (root) {
		root.lang = locale === 'en-XA' ? FALLBACK_LOCALE : locale;
		root.dir = isRightToLeftLocale(locale) ? 'rtl' : 'ltr';
	}
	return locale;
}
