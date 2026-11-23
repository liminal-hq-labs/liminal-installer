// What the script API is, written once: the modules, hooks and fields a script can use, with their notes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// This follows the `li-script` crate: `li.env`, `li.fs`, `li.ui`, `li.plan` and `li.net` are globals (there is
// no `require`), a capability header comment opens them, and a hook is a global function or a field of the table
// the script returns.

import type { MessageId } from './phrases';

export type DocKind = 'module' | 'function' | 'hook' | 'field' | 'capability' | 'keyword' | 'global' | 'variable';

export interface ApiDoc {
	/** The dotted name, such as `li.env.get`. */
	name: string;
	kind: DocKind;
	/** The text a completion inserts when it differs from the last part of `name`. */
	insert?: string;
	syntax?: string;
	note: string;
	example?: string;
	/** The capability that opens it. */
	capability?: string;
}

export const KIND_MESSAGE: Record<DocKind, MessageId> = {
	module: 'kind.module',
	function: 'kind.function',
	hook: 'kind.hook',
	field: 'kind.field',
	capability: 'kind.capability',
	keyword: 'kind.keyword',
	global: 'kind.global',
	variable: 'kind.variable',
};

/** What each hook receives. */
export const HOOK_PARAMS: Record<string, 'page' | 'plan'> = {
	detect: 'page',
	validate: 'page',
	on_commit: 'plan',
	on_uninstall: 'plan',
};

/** The hook as it is written, such as `on_commit(plan)`. */
export const hookSignature = (name: string): string => `${name}(${HOOK_PARAMS[name] ?? 'page'})`;

/** The capabilities a header comment may name. */
export const CAPABILITY_NAMES = ['env', 'fs.read', 'fs.write', 'net', 'ui', 'plan'] as const;

const CAPABILITY_DOCS: ApiDoc[] = [
	{ name: 'env', kind: 'capability', note: 'Reads the environment and system facts: `li.env.get`, `li.env.os` and `li.env.arch`.' },
	{ name: 'fs.read', kind: 'capability', note: 'Reads files through `li.fs`: `read`, `exists` and `list`.' },
	{ name: 'fs.write', kind: 'capability', note: 'Lets plan operations write files: `mkdir`, `copy`, `write_file`, `write_ini` and `write_json`. A script never writes directly; the plan does, and can undo it. Needs `plan` as well.' },
	{ name: 'net', kind: 'capability', note: 'Network access through `li.net.get`. Not in the retro Lua subset, and a dry run never reaches the network.' },
	{ name: 'ui', kind: 'capability', note: 'Shows notes and dialogs through `li.ui`.' },
	{ name: 'plan', kind: 'capability', note: 'Adds operations to the plan through `li.plan.add` and `plan:add`.' },
];

const HEADER_DOC: ApiDoc = {
	name: 'capabilities',
	kind: 'keyword',
	syntax: '-- capabilities: env, ui',
	note: 'A header comment at the top of the file names what the script may use. Anything not named is refused. A script may instead return a table with a `capabilities` list.',
	example: '-- capabilities: env, plan, fs.write',
};

export const API_DOCS: ApiDoc[] = [
	{ name: 'li', kind: 'global', note: 'The host modules: `li.env`, `li.fs`, `li.ui`, `li.plan` and `li.net`. They are globals; there is no `require`.' },
	{ name: 'li.env', kind: 'module', capability: 'env', note: 'The environment and system facts of the machine the installer runs on.' },
	{ name: 'li.env.get', kind: 'function', capability: 'env', syntax: 'li.env.get(name)', note: 'The value of an environment variable or installer fact (`target`, `install`, `version`), or nil.', example: 'local blaster = li.env.get("BLASTER")' },
	{ name: 'li.env.os', kind: 'function', capability: 'env', syntax: 'li.env.os()', note: 'The operating system of the target, such as `dos`, `windows` or `linux`.', example: 'if li.env.os() == "dos" then' },
	{ name: 'li.env.arch', kind: 'function', capability: 'env', syntax: 'li.env.arch()', note: 'The architecture of the target, such as `x86` or `x86_64`.' },
	{ name: 'li.fs', kind: 'module', capability: 'fs.read', note: 'Reads files anywhere the host allows. Writing goes through the plan.' },
	{ name: 'li.fs.read', kind: 'function', capability: 'fs.read', syntax: 'li.fs.read(path)', note: 'The bytes of a file as a string. Raises an error if it cannot be read.' },
	{ name: 'li.fs.exists', kind: 'function', capability: 'fs.read', syntax: 'li.fs.exists(path)', note: 'True when the path exists.', example: 'if li.fs.exists("C:\\\\GAMES") then' },
	{ name: 'li.fs.list', kind: 'function', capability: 'fs.read', syntax: 'li.fs.list(path)', note: 'The names in a directory, as a list of strings.' },
	{ name: 'li.ui', kind: 'module', capability: 'ui', note: 'Notes and dialogs shown to the person installing.' },
	{ name: 'li.ui.note', kind: 'function', capability: 'ui', syntax: 'li.ui.note(text)', note: 'Shows a short note on the current page.', example: 'li.ui.note("Found a Sound Blaster.")' },
	{ name: 'li.ui.dialog', kind: 'function', capability: 'ui', syntax: 'li.ui.dialog{ title = "…", text = "…" }', note: 'Shows a dialog. Takes a string or a table with `title` and `text`.' },
	{ name: 'li.plan', kind: 'module', capability: 'plan', note: 'Adds operations to the plan being built.' },
	{ name: 'li.plan.add', kind: 'function', capability: 'plan', syntax: 'li.plan.add(op)', note: 'Adds an operation, such as `{ op = "mkdir", path = "{install}/saves" }`. File operations (`mkdir`, `copy`, `write_file`, `write_ini`, `write_json`) also need `fs.write`.', example: 'li.plan.add({ op = "mkdir", path = "{install}/saves" })' },
	{ name: 'li.net', kind: 'module', capability: 'net', note: 'Network access. Not in the retro Lua subset.' },
	{ name: 'li.net.get', kind: 'function', capability: 'net', syntax: 'li.net.get(url)', note: 'Fetches a URL and returns the body as a string.' },
	{ name: 'detect', kind: 'hook', insert: 'detect', syntax: 'function detect(page)', note: 'Runs when a page opens. May change `page.fields`, which the page shows.', example: 'function detect(page)\n\tpage.fields["sound.card"] = "none"\nend' },
	{ name: 'validate', kind: 'hook', syntax: 'function validate(page)', note: 'Runs before Next. Changes nothing; returns a string or a list of strings describing problems, or nil.', example: 'function validate(page)\n\treturn {}\nend' },
	{ name: 'on_commit', kind: 'hook', syntax: 'function on_commit(plan)', note: 'Runs when the install plan is built. Takes the plan, not a page; add operations with `plan:add`.', example: 'function on_commit(plan)\n\tplan:add({ op = "mkdir", path = "{install}/saves" })\nend' },
	{ name: 'on_uninstall', kind: 'hook', syntax: 'function on_uninstall(plan)', note: 'Runs when the uninstall plan is built. Takes the plan, not a page.' },
	{ name: 'page', kind: 'variable', note: 'What `detect` and `validate` receive: the page being shown.' },
	{ name: 'page.id', kind: 'field', note: 'The id of the flow step the page belongs to.' },
	{ name: 'page.fields', kind: 'field', note: 'The page\'s state as a table. `detect` may change it; `validate` sees a copy. Keys such as `install.path` hold dotted names, so index them with brackets.', example: 'local path = page.fields["install.path"]' },
	{ name: 'plan', kind: 'variable', note: 'What `on_commit` and `on_uninstall` receive: the plan built so far.' },
	{ name: 'plan.ops', kind: 'field', note: 'The operations already in the plan, as a list of tables.' },
	{ name: 'plan.add', kind: 'function', capability: 'plan', syntax: 'plan:add(op)', note: 'Adds an operation to the plan. Same as `li.plan.add`. Needs the `plan` capability, and `fs.write` for file operations.', example: 'plan:add({ op = "write_ini", path = "{install}/GAME.INI" })' },
	...stdDocs(),
	...CAPABILITY_DOCS.map((d) => ({ ...d, name: `capability:${d.name}` })),
];

function stdDocs(): ApiDoc[] {
	const f = (name: string, syntax: string, note: string): ApiDoc => ({ name, kind: 'function', syntax, note });
	return [
		f('string.format', 'string.format(format, ...)', 'Formats values as `printf` does.'),
		f('string.match', 'string.match(s, pattern)', 'The first match of a Lua pattern, or nil. Captures are returned.'),
		f('string.find', 'string.find(s, pattern)', 'The start and end of the first match of a pattern, or nil.'),
		f('string.gsub', 'string.gsub(s, pattern, replacement)', 'Replaces matches of a pattern; returns the new string and the count.'),
		f('string.sub', 'string.sub(s, i, j)', 'The part of a string from `i` to `j`.'),
		f('string.lower', 'string.lower(s)', 'The string in lower case.'),
		f('string.upper', 'string.upper(s)', 'The string in upper case.'),
		f('string.rep', 'string.rep(s, n)', 'The string repeated `n` times.'),
		f('string.len', 'string.len(s)', 'The length of the string in bytes.'),
		f('table.insert', 'table.insert(list, value)', 'Appends a value to a list, or inserts at a position.'),
		f('table.remove', 'table.remove(list, pos)', 'Removes and returns an element; the last by default.'),
		f('table.concat', 'table.concat(list, sep)', 'Joins a list of strings.'),
		f('table.sort', 'table.sort(list, comp)', 'Sorts a list in place.'),
		f('math.floor', 'math.floor(x)', 'The largest integer not above `x`.'),
		f('math.max', 'math.max(x, ...)', 'The largest argument.'),
		f('math.min', 'math.min(x, ...)', 'The smallest argument.'),
		f('tostring', 'tostring(v)', 'Converts a value to a string.'),
		f('tonumber', 'tonumber(v, base)', 'Converts a string to a number, or nil.'),
		f('type', 'type(v)', 'The name of a value\'s type: `nil`, `number`, `string`, `boolean`, `table` or `function`.'),
		f('ipairs', 'ipairs(list)', 'Iterates a list in order.'),
		f('pairs', 'pairs(table)', 'Iterates every key and value of a table.'),
		f('select', 'select(n, ...)', 'The arguments from the n-th, or their count with `"#"`.'),
		f('assert', 'assert(v, message)', 'Raises an error unless `v` is true.'),
		f('error', 'error(message)', 'Stops the script with an error.'),
		f('setmetatable', 'setmetatable(table, meta)', 'Sets a table\'s metatable.'),
	];
}

const byName = new Map(API_DOCS.map((d) => [d.name, d]));

/** The documentation for a dotted name; `page:set`-style colons count as dots. */
export function docFor(name: string): ApiDoc | undefined {
	return byName.get(name.replace(/:/g, '.'));
}

/** The documentation for a capability named in the header. */
export function capabilityDoc(name: string): ApiDoc | undefined {
	return byName.get(`capability:${name}`);
}

export { HEADER_DOC };
