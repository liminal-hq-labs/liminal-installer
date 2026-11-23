// Tests the editor's pieces without a browser: the language, completion, hover docs, diagnostics, phrases and the formatter.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { foldable } from '@codemirror/language';
import { EditorState, Text } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { suggest } from './completion';
import { countFindings, toLintDiagnostics } from './diagnostics';
import { formatLua } from './format';
import { docTooltip, nameAt } from './hover';
import { luaSupport } from './luaLanguage';
import { API_DOCS, HOOK_PARAMS, docFor, hookSignature } from './luaDocs';
import { ENGLISH, editorPhrases, setLanguage, t } from './phrases';

/** A state over `text` with the caret at the `|` marker, which is removed. */
function at(text: string): { state: EditorState; pos: number } {
	const pos = text.indexOf('|');
	return { state: EditorState.create({ doc: text.replace('|', ''), extensions: [luaSupport()] }), pos };
}

const labels = (text: string, explicit = false) => {
	const { state, pos } = at(text);
	return suggest(state, pos, explicit)?.options.map((o) => o.label) ?? [];
};

describe('language', () => {
	it('is Lua, and folds a block from its opening line to the line before its end', () => {
		const state = EditorState.create({ doc: 'function a()\n\tif x then\n\t\treturn 1\n\tend\nend\n', extensions: [luaSupport()] });
		expect(state.facet(EditorState.languageData).length).toBeGreaterThan(0);
		const outer = foldable(state, state.doc.line(1).from, state.doc.line(1).to);
		expect(outer).toEqual({ from: state.doc.line(1).to, to: state.doc.line(4).to });
		const inner = foldable(state, state.doc.line(2).from, state.doc.line(2).to);
		expect(inner?.to).toBe(state.doc.line(3).to);
		expect(foldable(state, state.doc.line(3).from, state.doc.line(3).to)).toBeNull();
	});

	it('does not fold a block that opens and closes on one line', () => {
		const state = EditorState.create({ doc: 'if a then b() end\nx = 1\n', extensions: [luaSupport()] });
		expect(foldable(state, 0, state.doc.line(1).to)).toBeNull();
	});
});

describe('documentation', () => {
	it('gives the hooks their real parameters', () => {
		expect(hookSignature('on_commit')).toBe('on_commit(plan)');
		expect(hookSignature('on_uninstall')).toBe('on_uninstall(plan)');
		expect(hookSignature('detect')).toBe('detect(page)');
		expect(hookSignature('validate')).toBe('validate(page)');
		expect(Object.keys(HOOK_PARAMS).sort()).toEqual(['detect', 'on_commit', 'on_uninstall', 'validate']);
		expect(docFor('on_commit')?.syntax).toBe('function on_commit(plan)');
	});

	it('documents every module of the sandbox and nothing it lacks', () => {
		const modules = API_DOCS.filter((d) => d.kind === 'module').map((d) => d.name);
		expect(modules).toEqual(['li.env', 'li.fs', 'li.ui', 'li.plan', 'li.net']);
		expect(docFor('li.retro.sound')).toBeUndefined();
		expect(API_DOCS.some((d) => d.name === 'require')).toBe(false);
	});

	it('treats a colon as a dot', () => {
		expect(docFor('plan:add')?.capability).toBe('plan');
	});
});

describe('completion', () => {
	it('offers the modules after `li.`', () => {
		expect(labels('local x = li.|')).toEqual(['env', 'fs', 'ui', 'plan', 'net']);
		expect(labels('li.e|')).toContain('env');
	});

	it('offers a module\'s functions with the capability they need', () => {
		expect(labels('li.env.|')).toEqual(['get', 'os', 'arch']);
		const { state, pos } = at('li.ui.|');
		const options = suggest(state, pos)!.options;
		expect(options.map((o) => o.label)).toEqual(['note', 'dialog']);
		expect(options[0]!.detail).toContain('ui');
	});

	it('offers page and plan members', () => {
		expect(labels('page.|')).toEqual(['id', 'fields']);
		expect(labels('plan:|')).toEqual(['ops', 'add']);
		expect(labels('page.fields.|')).toEqual([]);
	});

	it('offers the hooks as snippets on a blank line, with their own parameter', () => {
		const names = labels('|', true);
		expect(names).toEqual(expect.arrayContaining(['detect', 'validate', 'on_commit', 'on_uninstall', 'li', 'local', 'string']));
		const { state, pos } = at('on_|');
		const commit = suggest(state, pos)!.options.find((o) => o.label === 'on_commit')!;
		expect(commit.detail).toBe('on_commit(plan)');
		const after = at('local x = d|');
		expect(suggest(after.state, after.pos)!.options.some((o) => o.label === 'detect' && o.detail === 'detect(page)')).toBe(false);
	});

	it('offers capability names in the header and leaves out those already named', () => {
		expect(labels('-- capabilities: env, |')).toEqual(['fs.read', 'fs.write', 'net', 'ui', 'plan']);
		expect(labels('-- capabilities: fs.|')).toEqual(expect.arrayContaining(['fs.read', 'fs.write']));
	});

	it('is quiet inside strings and comments', () => {
		expect(labels('local s = "li.|"')).toEqual([]);
		expect(labels('-- see li.|')).toEqual([]);
	});

	it('reaches the library tables', () => {
		expect(labels('string.|')).toContain('format');
		expect(labels('table.|')).toContain('insert');
	});
});

describe('hover', () => {
	it('names the qualified word under the pointer', () => {
		const { state, pos } = at('local x = li.env.g|et("A")');
		expect(nameAt(state, pos)).toMatchObject({ name: 'li.env.get' });
	});

	it('documents module functions, hooks, page fields and capability names', () => {
		const tip = (text: string) => {
			const { state, pos } = at(text);
			return docTooltip(state, pos);
		};
		expect(tip('x = li.env.g|et("A")')).not.toBeNull();
		expect(tip('function on_comm|it(plan)')).not.toBeNull();
		expect(tip('page.fie|lds.x = 1')).not.toBeNull();
		expect(tip('-- capabilities: en|v, ui')).not.toBeNull();
		expect(tip('-- capabilities: env, ui|')).not.toBeNull();
		expect(tip('local foo|bar = 1')).toBeNull();
		expect(tip('-- li.env.g|et in a comment')).toBeNull();
	});
});

describe('diagnostics', () => {
	const doc = Text.of(['local a = 1', '  local b = * 2', 'end']);
	const base = { severity: 'error' as const, code: 'syntax', message: 'bad' };

	it('maps lines and columns to the span they mark', () => {
		const [d] = toLintDiagnostics(doc, [{ ...base, line: 2, column: 13, end_line: 2, end_column: 14 }]);
		expect(doc.sliceString(d!.from, d!.to)).toBe('*');
		expect(d).toMatchObject({ severity: 'error', source: 'syntax', message: 'bad' });
	});

	it('keeps a warning a warning and clamps what is out of range', () => {
		const [w, far] = toLintDiagnostics(doc, [
			{ ...base, severity: 'warning', line: 1, column: 1, end_line: 1, end_column: 6 },
			{ ...base, line: 99, column: 99, end_line: 99, end_column: 120 },
		]);
		expect(w!.severity).toBe('warning');
		expect(doc.sliceString(w!.from, w!.to)).toBe('local');
		expect(far!.from).toBeLessThanOrEqual(doc.length);
		expect(far!.to).toBeLessThanOrEqual(doc.length);
		expect(far!.to).toBeGreaterThanOrEqual(far!.from);
	});

	it('counts errors and warnings', () => {
		expect(countFindings([{ ...base, line: 1, column: 1, end_line: 1, end_column: 2 }, { ...base, severity: 'warning', line: 1, column: 1, end_line: 1, end_column: 2 }])).toEqual({ errors: 1, warnings: 1 });
	});
});

describe('phrases', () => {
	it('gives the editor library every phrase it shows, from one catalogue', () => {
		const phrases = editorPhrases();
		expect(phrases['Find']).toBe('Find');
		expect(phrases['Go to line']).toBe('Go to line');
		expect(Object.keys(phrases).length).toBe(Object.keys(ENGLISH).filter((k) => k.startsWith('cm.')).length);
	});

	it('fills placeholders and lets a language replace a word', () => {
		expect(t('status.position', { line: 4, column: 9 })).toBe('Ln 4, Col 9');
		expect(t('status.lua')).toBe('Lua 5.4 · retro subset checked');
		setLanguage({ 'cm.Find': 'Chercher', 'status.position': 'Lg {line}, Col {column}' });
		try {
			expect(editorPhrases()['Find']).toBe('Chercher');
			expect(t('status.position', { line: 1, column: 2 })).toBe('Lg 1, Col 2');
			expect(t('cm.Replace')).toBe('Replace');
		} finally {
			setLanguage({});
		}
	});
});

describe('formatter', () => {
	it('indents by block and bracket depth with tabs', () => {
		const src = 'function detect(page)\nlocal t = {\n1,\n2,\n}\nif a then\nx()\nelseif b then\ny()\nelse\nz()\nend\nfor i = 1, 3 do\nprint(i)\nend\nend\n';
		expect(formatLua(src)).toBe('function detect(page)\n\tlocal t = {\n\t\t1,\n\t\t2,\n\t}\n\tif a then\n\t\tx()\n\telseif b then\n\t\ty()\n\telse\n\t\tz()\n\tend\n\tfor i = 1, 3 do\n\t\tprint(i)\n\tend\nend\n');
	});

	it('handles repeat, one-line blocks and strings or comments that hold keywords', () => {
		const src = 'repeat\nlocal s = "end"  \n-- end\nuntil x\nif a then b() end\nc()\n';
		expect(formatLua(src)).toBe('repeat\n\tlocal s = "end"\n\t-- end\nuntil x\nif a then b() end\nc()\n');
	});

	it('leaves the inside of long strings and comments alone', () => {
		const src = 'local s = [[\n   keep   \n  end\n]]\n--[[\n  note\n]]\nx()\n';
		expect(formatLua(src)).toBe('local s = [[\n   keep   \n  end\n]]\n--[[\n  note\n]]\nx()\n');
	});

	it('trims trailing space, settles line endings and ends with one newline', () => {
		expect(formatLua('a()  \r\n\r\n\r\nb()\r\n\r\n')).toBe('a()\n\n\nb()\n');
		expect(formatLua('')).toBe('');
	});

	it('is stable: formatting twice changes nothing', () => {
		const once = formatLua('function f()\nif a then\nreturn {\n1,\n}\nend\nend\n');
		expect(formatLua(once)).toBe(once);
	});

	it('never changes the code, only the space around it', () => {
		const src = 'local x   =  { a=1 ,b = 2 }\nfunction  f( a )\nreturn a+1\nend\n';
		const squash = (s: string) => s.replace(/\s+/g, '');
		expect(squash(formatLua(src))).toBe(squash(src));
	});
});
