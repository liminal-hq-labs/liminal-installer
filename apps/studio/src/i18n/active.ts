// The locale in force: its catalogue and the `Intl` tags that go with it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { setRendererCatalogue } from '@liminal-hq/installer-renderer';
import { FALLBACK_LOCALE, intlTagFor, pluralTagFor, type Locale } from './locales';
import type { MessageId, PluralId } from './messages';

/** A plural form of a group, such as `problems.count.one`. */
export type PluralFormId = `${PluralId}.${'zero' | 'one' | 'two' | 'few' | 'many' | 'other'}`;

/** A catalogue: some or all of the messages in one locale. A missing one falls back to English. */
export type Catalogue = Partial<Record<MessageId | PluralFormId, string>>;

let current: { locale: Locale; messages: Catalogue } = { locale: FALLBACK_LOCALE, messages: {} };

export function activeLocale(): Locale {
	return current.locale;
}

/** The messages `t` reads before it falls back to English. */
export function activeMessages(): Catalogue {
	return current.messages;
}

/** The tag to hand `Intl.NumberFormat`, `Intl.DateTimeFormat` and the rest. */
export function formatLocale(): string {
	return intlTagFor(current.locale);
}

/** The tag to hand `Intl.PluralRules`. */
export function pluralLocale(): string {
	return pluralTagFor(current.locale);
}

/** Makes an already loaded catalogue the one in force. */
export function setActiveLocale(locale: Locale, messages: Catalogue): void {
	current = { locale, messages };
	setRendererCatalogue(messages);
}

/** Puts the English catalogue back; for tests. */
export function resetActiveLocale(): void {
	setActiveLocale(FALLBACK_LOCALE, {});
}
