// Draws one API entry as DOM, for hover text and the note beside a completion.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { KIND_MESSAGE, type ApiDoc } from './luaDocs';
import { t } from './phrases';

function add(parent: HTMLElement, tag: string, text: string, className?: string): HTMLElement {
	const el = parent.ownerDocument.createElement(tag);
	el.textContent = text;
	if (className) el.className = className;
	parent.append(el);
	return el;
}

/** A name, its kind, what it does, how to write it and what it needs. */
export function docElement(entry: ApiDoc, owner: Document = document): HTMLElement {
	const root = owner.createElement('div');
	root.className = 'ed-doc';
	const head = add(root, 'div', '');
	add(head, 'span', entry.name.replace(/^capability:/, ''), 'ed-doc-name');
	add(head, 'span', t(KIND_MESSAGE[entry.kind]), 'ed-doc-kind');
	add(root, 'p', entry.note, 'ed-doc-note');
	if (entry.syntax) {
		add(root, 'span', t('doc.syntax'), 'ed-doc-label');
		add(root, 'code', entry.syntax);
	}
	if (entry.example) {
		add(root, 'span', t('doc.example'), 'ed-doc-label');
		add(root, 'code', entry.example);
	}
	if (entry.capability)
		add(root, 'span', t('doc.needs', { capability: entry.capability }), 'ed-doc-needs');
	return root;
}
