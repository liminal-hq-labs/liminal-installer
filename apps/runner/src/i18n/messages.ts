// The runner's message catalogue: every string it shows itself, keyed by a stable identifier.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { rendererMessages, setRendererCatalogue } from '@liminal-hq/installer-renderer';

/**
 * English (`en-CA`), the source catalogue. The product's own pages are the project's text; these
 * are the screens the runner makes for itself (progress with no flow, the end of a run, an error)
 * and what it tells a screen reader. Keys are `area.thing`; a value is plain text with `{name}` tokens.
 */
export const enMessages = {
	...rendererMessages,
	// translator: Window title while installing. {name} is the product's name.
	'runner.title.install': '{name} Setup',
	// translator: Window title while updating. {name} is the product's name.
	'runner.title.update': '{name} Update',
	// translator: Window title while changing which components are installed. {name} is the product's name.
	'runner.title.modify': '{name} Setup — Modify',
	// translator: Window title while repairing. {name} is the product's name.
	'runner.title.repair': '{name} Repair',
	// translator: Window title while uninstalling. {name} is the product's name.
	'runner.title.uninstall': '{name} Uninstall',
	// translator: Accessible name of the whole runner window. {name} is the product's name.
	'runner.window': 'Setup for {name}',
	// translator: Shown for a moment before the first page arrives.
	'runner.starting': 'Starting…',
	// translator: Shown when the page is opened in a browser instead of the runner's own window.
	'runner.needsShell': 'The runner needs its desktop shell.',
	// translator: Heading of the progress screen while installing. {name} is the product's name.
	'runner.working.install': 'Installing {name}',
	'runner.working.update': 'Updating {name}',
	'runner.working.modify': 'Changing {name}',
	'runner.working.repair': 'Repairing {name}',
	'runner.working.uninstall': 'Removing {name}',
	// translator: Body of the progress screen; the file being worked on follows.
	'runner.working.body': 'Please wait. Setup can be cancelled, and every change is then undone.',
	// translator: Label of the progress bar on the screens the runner makes.
	'runner.progress': 'Overall progress',
	// translator: Heading of the end screen. {name} is the product's name.
	'runner.done.install': '{name} is installed',
	'runner.done.update': '{name} is up to date',
	'runner.done.modify': '{name} has been changed',
	'runner.done.repair': '{name} has been repaired',
	'runner.done.uninstall': '{name} has been removed',
	// translator: Body of the end screen.
	'runner.done.body': 'Setup is complete.',
	// translator: Heading of the error screen.
	'runner.failed.title': 'Setup could not finish',
	// translator: Body of the error screen: what was done about it.
	'runner.failed.body': 'Every change was undone. {message}',
	// translator: Button that closes the runner.
	'runner.close': 'Close',
	// translator: Button that stops the work and undoes it.
	'runner.cancel': 'Cancel',
	// translator: Spoken when the progress passes another tenth. {percent} is a whole number.
	'runner.announce.progress': '{percent}% complete',
	// translator: Spoken when the run is cancelled.
	'runner.announce.cancelled': 'Cancelled. Every change was undone.',
	// translator: Spoken when a page the person cannot move on from, such as a licence not yet accepted, appears.
	'runner.announce.page': '{title}',
	// translator: Spoken after a page asked for something and the person pressed Next.
	'runner.announce.problems': 'Setup cannot go on: {problems}',
	// <end of messages>
} as const;

export type MessageId = keyof typeof enMessages;

let active: Partial<Record<string, string>> = {};

/** Makes a catalogue the one in force, for the runner and the shared renderer. */
export function setCatalogue(catalogue: Partial<Record<string, string>>): void {
	active = catalogue;
	setRendererCatalogue(catalogue);
}

function interpolate(message: string, values: Record<string, string | number> | undefined): string {
	if (!values) return message;
	return message.replace(/\{(\w+)\}/g, (token, name: string) =>
		name in values ? String(values[name]) : token,
	);
}

/** The message for `id` in the active catalogue, or English when that catalogue lacks it. */
export function t(id: MessageId, values?: Record<string, string | number>): string {
	return interpolate(active[id] ?? enMessages[id], values);
}
