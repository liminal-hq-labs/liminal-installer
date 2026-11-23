// The renderer's own messages, in the same catalogue pattern as the apps that host it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/**
 * English (`en-CA`), the source. A host spreads these into its own `enMessages`, so its pseudo-locales
 * and parity checks cover them, and hands its active catalogue to {@link setRendererCatalogue}.
 * Keys are `renderer.thing`; a value is plain text with `{name}` tokens.
 */
export const rendererMessages = {
	// translator: Marks a meter that is nearly full or nearly empty, such as free disk space. Drawn in capitals in the text look.
	'renderer.low': 'Low',
	// translator: Sample caption of a step progress bar. {value} is the current step, {max} the number of steps.
	'renderer.stepOf': 'Step {value} of {max}',
	'renderer.required': 'required',
	'renderer.image': 'image',
	'renderer.customContent': 'custom content',
	'renderer.slot': 'The page’s layout is drawn here',
	// translator: Default credit line on the classic backdrop.
	'renderer.credit': 'Liminal Installer',
	// translator: Accessible name of the group of status meters in the Windows 11 look.
	'renderer.setupStatus': 'Setup status',
	// translator: Accessible name of the close button in a drawn window title bar.
	'renderer.close': 'Close',
	// translator: Label of the sample Start button in the drawn Windows taskbar.
	'renderer.start': 'Start',
	// translator: Default taskbar caption of the drawn Windows setup window.
	'renderer.setup': 'Setup',
	// <end of renderer messages>
} as const;

export type RendererMessageId = keyof typeof rendererMessages;

let active: Partial<Record<string, string>> = {};

/** The messages `t` reads before it falls back to English: the host's active catalogue. */
export function setRendererCatalogue(catalogue: Partial<Record<string, string>>): void {
	active = catalogue;
}

/** The message for `id` in the host's catalogue, or English when it lacks it. */
export function t(id: RendererMessageId, values?: Record<string, string | number>): string {
	const message = active[id] ?? rendererMessages[id];
	if (!values) return message;
	return message.replace(/\{(\w+)\}/g, (token, name: string) =>
		name in values ? String(values[name]) : token,
	);
}
