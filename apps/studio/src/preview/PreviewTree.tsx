// Draws a layout tree as themed React elements: DOS text mode, Windows 95 classic, Afterglow and Neutral.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useLayoutEffect, useMemo, useRef, type CSSProperties, type ReactNode } from 'react';
import { t } from '../i18n/messages';
import { resolveForRenderer } from '../lib/layoutEdit';
import { composeScreen, isStageId, refKey, type StepInfo } from '../lib/stage';
import type { Layout, LayoutNode } from '../lib/types';
import {
	SAMPLE,
	STAGE,
	fill,
	fraction,
	isLow,
	lookFor,
	lookup,
	meterSource,
	num,
	options,
	props,
	str,
	type Ctx,
	type Look,
	type Option,
	type RenderOptions,
} from './model';
import styles from './preview.module.css';

/** A sample clock for the Windows taskbar. It is part of the artwork, not a message. */
const SAMPLE_CLOCK = '4:15 PM';

const camel = (name: string): string =>
	name.replace(/-([a-z0-9])/g, (_, c: string) => c.toUpperCase());

/** The module classes for `pv-` style names, such as `pv-stack`, skipping any the stylesheet lacks. */
const pv = (...names: (string | false | null | undefined)[]): string =>
	names
		.filter((n): n is string => !!n)
		.map((n) => styles[camel(n)])
		.filter(Boolean)
		.join(' ');

const text = (cx: Ctx, s: string): string => fill(s, cx.vars);

/** The attributes that tie an element to its node: the key a click reads and the selection outline. */
function at(cx: Ctx, node: LayoutNode | null | undefined): Record<string, string> {
	const key = node ? cx.pathOf?.(node) : undefined;
	if (key === undefined) return {};
	return {
		'data-node-path': key,
		...(cx.interact?.selected === key ? { 'data-selected': 'true' } : {}),
	};
}

const running = (cx: Ctx): boolean => cx.interact?.mode === 'run';
/** The key a control's value is stored under: its `bind`, else its place in the tree. */
const keyOf = (cx: Ctx, node: LayoutNode): string | undefined => node.bind ?? cx.pathOf?.(node);

// ---------------------------------------------------------------------------------------------
// Pixel and vector art

const colour = (c: unknown): string =>
	typeof c === 'string' && /^(#[0-9a-fA-F]{3,8}|[a-zA-Z]{3,20})$/.test(c) ? c : 'none';
const n2 = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? v : 0);

/** Draws an `art` property as inline SVG: `rects` of `[x, y, w, h, colour]` and free `shapes`. */
export function ArtSvg({ art, label = '' }: { art: unknown; label?: string }): ReactNode {
	if (typeof art !== 'object' || art === null) return null;
	const a = art as Record<string, unknown>;
	const vb = Array.isArray(a.viewBox) ? a.viewBox : [a.width, a.height];
	const w = n2(vb[0]) || 16;
	const h = n2(vb[1]) || 16;
	const scale = n2(a.scale) || 1;
	const rects = Array.isArray(a.rects) ? a.rects : [];
	const shapes = Array.isArray(a.shapes) ? a.shapes : [];
	return (
		<svg
			className={pv('pv-art')}
			width={w * scale}
			height={h * scale}
			viewBox={`0 0 ${w} ${h}`}
			shapeRendering={rects.length > 0 ? 'crispEdges' : undefined}
			{...(label ? { role: 'img', 'aria-label': label } : { 'aria-hidden': true })}
		>
			{rects.map((r, i) =>
				Array.isArray(r) ? (
					<rect
						key={`r${i}`}
						x={n2(r[0])}
						y={n2(r[1])}
						width={n2(r[2])}
						height={n2(r[3])}
						fill={colour(r[4])}
					/>
				) : null,
			)}
			{shapes.map((sh, i) => {
				if (typeof sh !== 'object' || sh === null) return null;
				const s = sh as Record<string, unknown>;
				const paint = {
					fill: colour(s.fill),
					stroke: colour(s.stroke),
					strokeWidth: n2(s.width) || 1,
					strokeLinecap: s.cap ? (String(s.cap) as 'butt' | 'round' | 'square') : undefined,
					strokeLinejoin: s.join ? (String(s.join) as 'miter' | 'round' | 'bevel') : undefined,
				};
				if (Array.isArray(s.rect))
					return (
						<rect
							key={`s${i}`}
							x={n2(s.rect[0])}
							y={n2(s.rect[1])}
							width={n2(s.rect[2])}
							height={n2(s.rect[3])}
							rx={n2(s.rx)}
							{...paint}
						/>
					);
				if (typeof s.path === 'string') return <path key={`s${i}`} d={s.path} {...paint} />;
				if (Array.isArray(s.circle))
					return (
						<circle
							key={`s${i}`}
							cx={n2(s.circle[0])}
							cy={n2(s.circle[1])}
							r={n2(s.circle[2])}
							{...paint}
						/>
					);
				return null;
			})}
		</svg>
	);
}

// ---------------------------------------------------------------------------------------------
// Nodes

/** A label with its hot key marked: underlined in Windows, coloured in DOS. */
function HotLabel({ cx, label, hot }: { cx: Ctx; label: string; hot: string }): ReactNode {
	const s = fill(label, cx.vars);
	const i = hot ? s.toLowerCase().indexOf(hot.toLowerCase()) : -1;
	if (i < 0) return s;
	const mark = s[i]!;
	return (
		<>
			{s.slice(0, i)}
			{cx.look === 'win95' ? <u>{mark}</u> : <span className={pv('pv-hot')}>{mark}</span>}
			{s.slice(i + 1)}
		</>
	);
}

function Kids({
	cx,
	node,
	skip = () => false,
}: {
	cx: Ctx;
	node: LayoutNode;
	skip?: (n: LayoutNode) => boolean;
}): ReactNode {
	return (node.children ?? [])
		.filter((c) => !skip(c))
		.map((c, i) => <Node key={i} cx={cx} node={c} />);
}

const List = ({ cx, nodes }: { cx: Ctx; nodes: LayoutNode[] }): ReactNode =>
	nodes.map((n, i) => <Node key={i} cx={cx} node={n} />);

const regionOf = (n: LayoutNode): string => str(n, 'region');

function Bar({ cx, node, pct }: { cx: Ctx; node: LayoutNode; pct: number }): ReactNode {
	const warn = isLow(node, pct);
	if (cx.look === 'dos') {
		const cells = 24;
		const on = Math.round((cells * pct) / 100);
		return (
			<span className={pv('pv-bar-text', warn && 'pv-warn')}>
				<span className={pv('pv-on')}>{'█'.repeat(on)}</span>
				<span className={pv('pv-off')}>{'░'.repeat(cells - on)}</span> {pct}%
				{warn ? ` ${t('preview.low').toUpperCase()}` : ''}
			</span>
		);
	}
	if (cx.look === 'win95') {
		const blocks = 24;
		const on = Math.round((blocks * pct) / 100);
		return (
			<>
				<span className={pv('pv-bar', 'pv-sunken')}>
					{Array.from({ length: blocks }, (_, i) => (
						<i key={i} className={pv('pv-blk', i < on && 'pv-lit')} />
					))}
				</span>
				<span className={pv('pv-pct')}>{pct}%</span>
			</>
		);
	}
	return (
		<span className={pv('pv-bar')}>
			<span className={pv('pv-bar-fill', warn && 'pv-warn')} style={{ width: `${pct}%` }} />
		</span>
	);
}

function Node({ cx, node }: { cx: Ctx; node: LayoutNode }): ReactNode {
	const p = props(node);
	const label = text(cx, str(node, 'label'));
	switch (node.kind) {
		case 'backdrop':
		case 'window':
		case 'dialog':
			// Containers nested inside content draw as plain groups; the frame is drawn from the root.
			return (
				<div className={pv('pv-group')} data-kind={node.kind} {...at(cx, node)}>
					<Kids cx={cx} node={node} />
				</div>
			);
		case 'stack': {
			const row = str(node, 'direction') === 'row' || str(node, 'direction') === 'horizontal';
			const group = str(node, 'group');
			const align = str(node, 'align');
			return (
				<div
					className={pv(
						'pv-stack',
						row ? 'pv-row' : 'pv-col',
						group && `pv-g-${group}`,
						align && `pv-align-${align}`,
					)}
					data-kind="stack"
					{...at(cx, node)}
				>
					<Kids cx={cx} node={node} />
				</div>
			);
		}
		case 'grid':
			return (
				<div className={pv('pv-grid')} data-kind="grid" {...at(cx, node)}>
					<Kids cx={cx} node={node} />
				</div>
			);
		case 'text':
			return <TextNode cx={cx} node={node} />;
		case 'image': {
			const alt = text(cx, str(node, 'alt') || str(node, 'label') || t('preview.image'));
			const art = <ArtSvg art={p.art} label={str(node, 'alt') || str(node, 'label')} />;
			const hasArt = typeof p.art === 'object' && p.art !== null;
			const panel = typeof p.panel === 'string';
			const style = panel
				? ({
						background: colour(p.panel),
						...(typeof p.ground === 'string' ? { '--ground': colour(p.ground) } : {}),
					} as CSSProperties)
				: undefined;
			return (
				<div
					className={pv('pv-image', panel && 'pv-panel')}
					data-kind="image"
					style={style}
					{...at(cx, node)}
				>
					{hasArt ? (
						art
					) : (
						<span className={pv('pv-noart')} role="img" aria-label={alt}>
							{alt}
						</span>
					)}
				</div>
			);
		}
		case 'billboard':
			return <Billboard cx={cx} node={node} label={label} />;
		case 'meter':
		case 'progress':
			return <Meter cx={cx} node={node} label={label} />;
		case 'button': {
			const name = <HotLabel cx={cx} label={str(node, 'label')} hot={str(node, 'hotkey')} />;
			const plain = str(node, 'label');
			return (
				<button
					className={pv('pv-button', p.default === true && 'pv-default')}
					data-kind="button"
					data-action={str(node, 'action')}
					type="button"
					tabIndex={running(cx) ? 0 : -1}
					onClick={
						running(cx) ? () => cx.interact?.onAction?.(str(node, 'action') || 'next') : undefined
					}
					{...at(cx, node)}
				>
					{cx.look === 'dos' ? (
						<>
							{'< '}
							{name}
							{' >'}
						</>
					) : cx.look === 'win95' && plain === 'Next' ? (
						<>
							{name}
							{' >'}
						</>
					) : cx.look === 'win95' && plain === 'Back' ? (
						<>
							{'< '}
							{name}
						</>
					) : (
						name
					)}
				</button>
			);
		}
		case 'checklist':
		case 'radio':
			return <Choices cx={cx} node={node} label={label} />;
		case 'field':
		case 'path':
			return <FieldNode cx={cx} node={node} label={label} />;
		case 'custom':
			return cx.renderer === 'canvas' || cx.renderer === 'web' ? (
				<div className={pv('pv-custom')} data-kind="custom" {...at(cx, node)}>
					{t('preview.customContent')}
				</div>
			) : null;
		case 'slot':
			return (
				<div className={pv('pv-unknown')} data-kind="slot" {...at(cx, node)}>
					{t('preview.slot')}
				</div>
			);
		default:
			return (
				<div className={pv('pv-unknown')} data-kind={node.kind} {...at(cx, node)}>
					{node.kind}
					<Kids cx={cx} node={node} />
				</div>
			);
	}
}

function FieldNode({ cx, node, label }: { cx: Ctx; node: LayoutNode; label: string }): ReactNode {
	const p = props(node);
	const v = lookup(cx, node.bind);
	const value =
		typeof v === 'string' || typeof v === 'number'
			? String(v)
			: typeof p.default === 'string'
				? p.default
				: '';
	const key = keyOf(cx, node);
	const choices = Array.isArray(p.options) ? (p.options as unknown[]).map(toChoice) : null;
	const input = !running(cx) ? (
		<span className={pv('pv-input')}>{fill(value, cx.vars)}</span>
	) : choices ? (
		<select
			className={pv('pv-input', 'pv-control')}
			aria-label={label}
			value={value}
			onChange={(e) => key && cx.interact?.onChange?.(key, e.target.value)}
		>
			{choices.map((c) => (
				<option key={c.id} value={c.id}>
					{c.label}
				</option>
			))}
		</select>
	) : (
		<input
			className={pv('pv-input', 'pv-control')}
			type="text"
			aria-label={label}
			value={fill(value, cx.vars)}
			onChange={(e) => key && cx.interact?.onChange?.(key, e.target.value)}
		/>
	);
	return (
		<div
			className={pv(node.kind === 'path' ? 'pv-path' : 'pv-field')}
			data-kind={node.kind}
			{...at(cx, node)}
		>
			<span className={pv('pv-caption')}>{label}</span>
			{input}
			{node.kind === 'path' && str(node, 'browse') ? (
				<button
					className={pv('pv-button')}
					type="button"
					tabIndex={running(cx) ? 0 : -1}
					onClick={running(cx) ? () => cx.interact?.onAction?.('browse') : undefined}
				>
					<HotLabel cx={cx} label={str(node, 'browse')} hot={str(node, 'hotkey')} />
				</button>
			) : null}
		</div>
	);
}

const toChoice = (o: unknown): { id: string; label: string } =>
	typeof o === 'object' && o !== null
		? {
				id: String((o as Record<string, unknown>).id ?? ''),
				label: String(
					(o as Record<string, unknown>).label ?? (o as Record<string, unknown>).id ?? '',
				),
			}
		: { id: String(o), label: String(o) };

function TextNode({ cx, node }: { cx: Ctx; node: LayoutNode }): ReactNode {
	const role = str(node, 'role') || 'body';
	const cls = pv('pv-text', `pv-${role}`);
	if (role === 'step-item' || role === 'task') {
		const state = str(node, 'state') || 'todo';
		const mark = state === 'done' ? '✓' : state === 'current' ? '●' : '○';
		return (
			<div
				className={`${cls} ${pv(`pv-state-${state}`)}`.trim()}
				data-kind="text"
				{...at(cx, node)}
				aria-current={state === 'current' ? 'step' : undefined}
			>
				<span className={pv('pv-mark')} aria-hidden="true">
					{mark}
				</span>
				{text(cx, str(node, 'text'))}
			</div>
		);
	}
	let content = str(node, 'text') ? fill(str(node, 'text'), cx.vars) : '';
	if (!content && role === 'document' && str(node, 'source')) content = `(${str(node, 'source')})`;
	if (str(node, 'key'))
		return (
			<div className={cls} data-kind="text" {...at(cx, node)}>
				<span className={pv('pv-keycap')}>{str(node, 'key')}</span> {content}
			</div>
		);
	if (role === 'heading' && node.props?.icon)
		return (
			<div className={`${cls} ${pv('pv-has-icon')}`.trim()} data-kind="text" {...at(cx, node)}>
				<ArtSvg art={node.props.icon} />
				<span>{content}</span>
			</div>
		);
	if (role === 'error' && !content) return null;
	if (role === 'document' || role === 'summary')
		return (
			<pre
				className={cls}
				data-kind="text"
				{...at(cx, node)}
				tabIndex={role === 'document' ? -1 : undefined}
			>
				{content}
			</pre>
		);
	return (
		<div className={cls} data-kind="text" {...at(cx, node)}>
			{content}
		</div>
	);
}

function Billboard({ cx, node, label }: { cx: Ctx; node: LayoutNode; label: string }): ReactNode {
	const p = props(node);
	type Cap = { title: string; text: string; art?: unknown };
	let caps: Cap[] = [];
	if (Array.isArray(p.captions))
		caps = (p.captions as Record<string, unknown>[]).map((c) => ({
			title: String(c.title ?? ''),
			text: String(c.text ?? ''),
			art: c.art,
		}));
	else if (Array.isArray(p.items))
		caps = (p.items as unknown[]).map((c) => ({ title: '', text: String(c) }));
	else if (str(node, 'text')) caps = [{ title: '', text: str(node, 'text') }];
	const shown = cx.live?.slide ?? lookup(cx, node.bind);
	const i = caps.length ? (typeof shown === 'number' ? shown : 0) % caps.length : 0;
	const cap = caps[i];
	const art = cap?.art ?? p.art;
	return (
		<div
			className={pv('pv-billboard')}
			data-kind="billboard"
			role="group"
			aria-label={label}
			{...at(cx, node)}
		>
			{art ? (
				<div className={pv('pv-bb-art')}>
					<ArtSvg art={art} />
				</div>
			) : null}
			<div className={pv('pv-bb-copy')}>
				<div className={pv('pv-bb-title')}>{text(cx, cap?.title ?? '')}</div>
				<div className={pv('pv-bb-text')}>{text(cx, cap?.text ?? label)}</div>
			</div>
			<div className={pv('pv-bb-dots')} aria-hidden="true">
				{caps.map((_, d) => (
					<i key={d} className={pv(d === i && 'pv-on')} />
				))}
			</div>
		</div>
	);
}

function Meter({ cx, node, label }: { cx: Ctx; node: LayoutNode; label: string }): ReactNode {
	const p = props(node);
	if (node.kind === 'progress' && str(node, 'role') === 'steps') {
		const max = num(node, 'max', 8);
		const value = num(node, 'value', 1);
		return (
			<div
				className={pv('pv-steps')}
				data-kind="progress"
				{...at(cx, node)}
				role="progressbar"
				aria-valuemin={0}
				aria-valuemax={max}
				aria-valuenow={value}
				aria-label={label}
			>
				<div className={pv('pv-steps-text')}>
					{text(cx, str(node, 'text') || t('preview.stepOf', { value, max }))}
				</div>
				<div className={pv('pv-steps-bar')}>
					{Array.from({ length: max }, (_, i) => (
						<i key={i} className={pv(i < value && 'pv-on')} />
					))}
				</div>
			</div>
		);
	}
	const pct = Math.round(fraction(cx, node) * 100);
	const source = meterSource(node);
	return (
		<div
			className={pv('pv-meter')}
			data-kind={node.kind}
			{...at(cx, node)}
			data-source={source}
			role="meter"
			aria-label={label}
			aria-valuemin={0}
			aria-valuemax={100}
			aria-valuenow={pct}
		>
			<span className={pv('pv-meter-label')}>{label}</span>
			<Bar cx={cx} node={node} pct={pct} />
			{typeof p.caption === 'string' ? (
				<span className={pv('pv-meter-caption')}>{text(cx, p.caption)}</span>
			) : null}
		</div>
	);
}

function Choices({ cx, node, label }: { cx: Ctx; node: LayoutNode; label: string }): ReactNode {
	const check = node.kind === 'checklist';
	const items = options(cx, node);
	const v = lookup(cx, node.bind);
	const on = (o: Option): boolean => {
		if (check)
			return Array.isArray(v)
				? v.includes(o.id) || o.required === true
				: o.checked === true || o.required === true;
		const dflt =
			typeof props(node).default === 'string' ? (props(node).default as string) : items[0]?.id;
		return (typeof v === 'string' ? v : dflt) === o.id;
	};
	const cards = props(node).cards === true;
	return (
		<div
			className={pv('pv-choices', cards && 'pv-cards')}
			data-kind={node.kind}
			{...at(cx, node)}
			role={check ? 'group' : 'radiogroup'}
			aria-label={label}
		>
			<div className={pv('pv-caption')}>{label}</div>
			<div className={pv('pv-choice-list', props(node).sunken === true && 'pv-sunken')}>
				{items.map((o) => {
					const sel = on(o);
					const glyph =
						cx.look === 'dos' ? (
							check ? (
								sel ? (
									'[x]'
								) : (
									'[ ]'
								)
							) : sel ? (
								'(•)'
							) : (
								'( )'
							)
						) : (
							<span
								className={`${pv(check ? 'pv-box' : 'pv-dot')} ${sel ? styles.on : ''}`.trim()}
								aria-hidden="true"
							/>
						);
					const pick = () => {
						const key = keyOf(cx, node);
						if (!running(cx) || !key) return;
						if (!check) return cx.interact?.onChange?.(key, o.id);
						if (o.required) return;
						const now = items.filter(on).map((x) => x.id);
						cx.interact?.onChange?.(key, sel ? now.filter((id) => id !== o.id) : [...now, o.id]);
					};
					return (
						<div
							key={o.id}
							className={pv('pv-choice', sel && 'pv-sel')}
							role={check ? 'checkbox' : 'radio'}
							aria-checked={sel}
							tabIndex={running(cx) ? 0 : undefined}
							onClick={running(cx) ? pick : undefined}
							onKeyDown={
								running(cx)
									? (e) => {
											if (e.key === ' ' || e.key === 'Enter') {
												e.preventDefault();
												pick();
											}
										}
									: undefined
							}
						>
							<span className={pv('pv-glyph')}>{glyph}</span>
							<span className={pv('pv-choice-body')}>
								<span className={pv('pv-choice-label')}>{text(cx, o.label)}</span>
								{o.required && cards ? (
									<>
										{' '}
										<span className={pv('pv-req')}>· {t('preview.required')}</span>
									</>
								) : null}
								{o.description ? (
									<span className={pv('pv-desc')}>{text(cx, o.description)}</span>
								) : null}
								{o.badges?.length ? (
									<span className={pv('pv-badges')}>
										{o.badges.map((b, i) => (
											<span key={i} className={pv('pv-badge')}>
												{text(cx, b.text)}
											</span>
										))}
									</span>
								) : null}
							</span>
						</div>
					);
				})}
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------------------------
// Frames: the screen around the content

interface Parts {
	backdrop: LayoutNode | null;
	frame: LayoutNode;
	side: LayoutNode[];
	footer: LayoutNode[];
	hints: LayoutNode[];
	status: LayoutNode[];
	body: LayoutNode[];
}

function split(root: LayoutNode): Parts {
	const backdrop = root.kind === 'backdrop' ? root : null;
	const inner = backdrop ? (backdrop.children ?? []) : [root];
	const frame = inner.find((n) => n.kind === 'dialog' || n.kind === 'window') ?? {
		kind: 'window',
		children: inner.filter((n) => n.kind !== 'meter' && !regionOf(n)),
	};
	const behind = backdrop?.children ?? [];
	const all = [...behind, ...(frame.children ?? [])];
	// A meter sits on the backdrop unless its `place` says `dialog`; one in the dialog stays there
	// unless it says `backdrop` or `region = "status"`.
	const onBackdrop = (n: LayoutNode, inBackdrop: boolean): boolean =>
		n.kind === 'meter' &&
		(str(n, 'place') === 'backdrop' ||
			regionOf(n) === 'status' ||
			(inBackdrop && str(n, 'place') !== 'dialog'));
	const status = all.filter((n) => onBackdrop(n, behind.includes(n)));
	const inDialog = behind.filter((n) => n.kind === 'meter' && !status.includes(n) && !regionOf(n));
	return {
		backdrop,
		frame,
		side: all.filter((n) => regionOf(n) === 'side'),
		footer: all.filter((n) => regionOf(n) === 'footer'),
		hints: all.filter((n) => regionOf(n) === 'hints'),
		status,
		body: [
			...(frame.children ?? []).filter((n) => !regionOf(n) && !status.includes(n)),
			...inDialog,
		],
	};
}

function DosFrame({ cx, pt }: { cx: Ctx; pt: Parts }): ReactNode {
	const bd = pt.backdrop ? props(pt.backdrop) : {};
	const fp = props(pt.frame);
	const fillRow = '░'.repeat(120);
	const rows = Array.from({ length: 23 }, () => fillRow).join('\n');
	const title = text(cx, String(fp.title ?? bd.title ?? ''));
	return (
		<div className={pv('pv-stage')} {...at(cx, pt.backdrop)}>
			<div className={pv('pv-dos-bar')}>
				<span>{text(cx, String(bd.title ?? fp.title ?? ''))}</span>
				<span>{text(cx, String(bd.credit ?? t('preview.credit')))}</span>
			</div>
			<div className={pv('pv-dos-fill')} aria-hidden="true">
				{rows}
			</div>
			{pt.side.length ? (
				<div className={pv('pv-dos-side')}>
					<List cx={cx} nodes={pt.side} />
				</div>
			) : null}
			<div className={pv('pv-dos-shadow')} />
			<div className={pv('pv-dos-dialog')} role="dialog" aria-label={title} {...at(cx, pt.frame)}>
				<div className={pv('pv-dos-dtitle')}>[ {title} ]</div>
				<div className={pv('pv-dos-content')}>
					<List cx={cx} nodes={pt.body} />
					{pt.status.length ? (
						<div className={pv('pv-stack', 'pv-col', 'pv-g-meters')}>
							<List cx={cx} nodes={pt.status} />
						</div>
					) : null}
				</div>
				{pt.footer.length ? (
					<div className={pv('pv-dos-footer')}>
						<List cx={cx} nodes={pt.footer} />
					</div>
				) : null}
			</div>
			<div className={pv('pv-dos-hints')}>
				<List cx={cx} nodes={pt.hints} />
			</div>
		</div>
	);
}

const SVG_PIXEL = { shapeRendering: 'crispEdges', 'aria-hidden': true } as const;

const FileIcons = (): ReactNode => (
	<>
		<svg {...SVG_PIXEL} width="34" height="28" viewBox="0 0 34 28">
			<path d="M1.5 5.5 h11 l3 3 h17 v18 h-31 z" fill="#ffff80" stroke="#808000" />
			<rect x="2" y="10" width="30" height="1" fill="#ffffff" />
		</svg>
		<Arrow />
		<svg {...SVG_PIXEL} width="34" height="34" viewBox="0 0 34 34">
			<path d="M9.5 3.5 h11 l5 5 v22 h-16 z" fill="#ffffff" stroke="#404040" />
			<path d="M20.5 3.5 v5 h5" fill="#dfdfdf" stroke="#404040" />
			<rect x="4.5" y="18.5" width="10" height="10" fill="#c0c0c0" stroke="#404040" />
			<rect x="7" y="21" width="5" height="3" fill="#ffffff" />
		</svg>
	</>
);
const DriveIcon = (): ReactNode => (
	<svg {...SVG_PIXEL} width="34" height="14" viewBox="0 0 34 14">
		<rect x="0.5" y="2.5" width="33" height="10" fill="#dfdfdf" stroke="#404040" />
		<rect x="1" y="3" width="32" height="2" fill="#ffffff" />
		<rect x="4" y="8" width="18" height="1" fill="#808080" />
		<rect x="26" y="7" width="4" height="2" fill="#00c000" />
	</svg>
);
const Arrow = (): ReactNode => (
	<svg {...SVG_PIXEL} width="14" height="14" viewBox="0 0 14 14">
		<path d="M7 1 L13 7 L10 7 L10 13 L4 13 L4 7 L1 7 Z" fill="#c0c0c0" stroke="#404040" />
	</svg>
);
const TotalIcon = (): ReactNode => (
	<svg aria-hidden="true" width="34" height="34" viewBox="0 0 34 34">
		<circle cx="20" cy="15" r="12" fill="#e0e0e0" stroke="#404040" />
		<circle cx="20" cy="15" r="9" fill="none" stroke="#8fd3ff" strokeWidth="2" />
		<circle cx="20" cy="15" r="6" fill="none" stroke="#ffd36b" strokeWidth="2" />
		<circle cx="20" cy="15" r="3" fill="#ffffff" stroke="#404040" />
		<rect x="2.5" y="20.5" width="11" height="11" fill="#202020" stroke="#000" />
		<rect x="5" y="21" width="6" height="4" fill="#c0c0c0" />
		<rect x="4" y="27" width="8" height="4" fill="#ffffff" />
	</svg>
);
const LowIcon = ({ low, label }: { low: boolean; label: string }): ReactNode => (
	<svg aria-hidden="true" width="34" height="34" viewBox="0 0 34 34">
		<path
			d="M10.5 1.5 h13 l9 9 v13 l-9 9 h-13 l-9 -9 v-13 z"
			fill={low ? '#ff4040' : '#d4d4d4'}
			stroke={low ? '#800000' : '#a0a0a0'}
		/>
		<text
			x="17"
			y="21"
			textAnchor="middle"
			fontFamily="Tahoma, Verdana, sans-serif"
			fontSize="10"
			fontWeight="700"
			fill="#ffffff"
		>
			{label}
		</text>
	</svg>
);

/** The classic Windows setup status group: file, total and free-space gauges as lit-block columns. */
function WinStatus({ cx, meters }: { cx: Ctx; meters: LayoutNode[] }): ReactNode {
	if (meters.length === 0) return null;
	return (
		<div className={pv('pv-status')} role="group" aria-label={t('preview.setupStatus')}>
			{meters.map((m, mi) => {
				const pct = Math.round(fraction(cx, m) * 100);
				const source = meterSource(m);
				const low = isLow(m, pct);
				const lit = Math.round((pct / 100) * 14);
				const colourOf = source === 'free_space' ? (low ? '#c00000' : '#800080') : '#000080';
				return (
					<div
						key={mi}
						className={pv('pv-vmeter')}
						data-kind="meter"
						data-source={source}
						{...at(cx, m)}
						role="meter"
						aria-label={text(cx, str(m, 'label'))}
						aria-valuenow={pct}
						aria-valuemin={0}
						aria-valuemax={100}
					>
						<span className={pv('pv-vgauge', 'pv-sunken')}>
							{Array.from({ length: 14 }, (_, b) => (
								<i key={b} style={{ background: b < lit ? colourOf : 'transparent' }} />
							))}
						</span>
						<span className={pv('pv-vicons')}>
							{source === 'file' ? (
								<FileIcons />
							) : source === 'total' ? (
								<>
									<DriveIcon />
									<Arrow />
									<TotalIcon />
								</>
							) : (
								<>
									<DriveIcon />
									<span style={{ height: 14 }} />
									<LowIcon low={low} label={str(m, 'warn_label') || t('preview.low')} />
								</>
							)}
						</span>
					</div>
				);
			})}
		</div>
	);
}

function WinFrame({ cx, pt }: { cx: Ctx; pt: Parts }): ReactNode {
	const bd = pt.backdrop ? props(pt.backdrop) : {};
	const fp = props(pt.frame);
	const title = text(cx, String(fp.title ?? ''));
	return (
		<div className={pv('pv-stage')} {...at(cx, pt.backdrop)}>
			<div className={pv('pv-w-mark')}>{text(cx, String(bd.title ?? ''))}</div>
			<div className={pv('pv-w-sub')}>{text(cx, String(bd.subtitle ?? ''))}</div>
			<div className={pv('pv-w-credit')}>{text(cx, String(bd.credit ?? ''))}</div>
			<div className={pv('pv-w-dialog')} role="dialog" aria-label={title} {...at(cx, pt.frame)}>
				<div className={pv('pv-w-title')}>
					<span>{title}</span>
					<button
						className={pv('pv-w-close')}
						type="button"
						tabIndex={-1}
						aria-label={t('preview.close')}
					>
						<svg {...SVG_PIXEL} width="8" height="7" viewBox="0 0 8 7">
							<path
								d="M0 0h2v1h1v1h2V1h1V0h2v1H7v1H6v1H5v1h1v1h1v1h1v1H6V6H5V5H3v1H2v1H0V6h1V5h1V4h1V3H2V2H1V1H0z"
								fill="#000"
							/>
						</svg>
					</button>
				</div>
				<div className={pv('pv-w-main')}>
					{pt.side.length ? (
						<div className={pv('pv-w-side')} aria-hidden="true">
							<List cx={cx} nodes={pt.side} />
						</div>
					) : null}
					<div className={pv('pv-w-content')}>
						<List cx={cx} nodes={pt.body} />
					</div>
				</div>
				<div className={pv('pv-w-sep')} />
				<div className={pv('pv-w-footer')}>
					<List cx={cx} nodes={pt.footer.flatMap((f) => f.children ?? [])} />
				</div>
			</div>
			<WinStatus cx={cx} meters={pt.status} />
			<div className={pv('pv-w-task')}>
				<span className={pv('pv-w-start')}>
					<svg {...SVG_PIXEL} width="16" height="14" viewBox="0 0 16 14">
						<rect x="1" y="1" width="6" height="5" fill="#f00" />
						<rect x="8" y="1" width="6" height="5" fill="#0a0" />
						<rect x="1" y="7" width="6" height="5" fill="#00f" />
						<rect x="8" y="7" width="6" height="5" fill="#ff0" />
					</svg>
					{t('preview.start')}
				</span>
				<span className={pv('pv-w-win')}>{text(cx, String(bd.taskbar ?? t('preview.setup')))}</span>
				<span className={pv('pv-w-spacer')} />
				<span className={pv('pv-w-clock')}>{SAMPLE_CLOCK}</span>
			</div>
		</div>
	);
}

function ModernFrame({ cx, pt }: { cx: Ctx; pt: Parts }): ReactNode {
	const fp = props(pt.frame);
	const footerKids = pt.footer.flatMap((f) => f.children ?? []);
	const notes = footerKids.filter((k) => k.kind === 'text');
	const buttons = footerKids.filter((k) => k.kind !== 'text');
	return (
		<div className={pv('pv-stage')} {...at(cx, pt.backdrop)}>
			<div className={pv('pv-m-glow')} aria-hidden="true" />
			<div
				className={pv('pv-m-window')}
				{...at(cx, pt.frame)}
				role="dialog"
				aria-label={text(cx, String(fp.label ?? fp.title ?? ''))}
			>
				<div className={pv('pv-m-title')}>
					<span className={pv('pv-m-name')}>
						{fp.icon ? <ArtSvg art={fp.icon} /> : null}
						<span>{text(cx, String(fp.title ?? ''))}</span>
					</span>
					<span className={pv('pv-m-close')} aria-hidden="true">
						<svg
							width="16"
							height="16"
							viewBox="0 0 16 16"
							fill="none"
							stroke="currentColor"
							strokeWidth="1.5"
						>
							<path d="M4.5 4.5l7 7M11.5 4.5l-7 7" />
						</svg>
					</span>
				</div>
				<div className={pv('pv-m-main')}>
					{pt.side.length ? (
						<div className={pv('pv-m-side')}>
							<List cx={cx} nodes={pt.side} />
						</div>
					) : null}
					<div className={pv('pv-m-content')}>
						<List cx={cx} nodes={pt.body} />
					</div>
				</div>
				<div className={pv('pv-m-footer')}>
					<span className={pv('pv-m-note')}>
						<List cx={cx} nodes={notes} />
					</span>
					<span className={pv('pv-m-buttons')}>
						<List cx={cx} nodes={buttons} />
					</span>
				</div>
			</div>
			{typeof fp.caption === 'string' ? (
				<div className={pv('pv-m-caption')}>{text(cx, fp.caption)}</div>
			) : null}
		</div>
	);
}

// ---------------------------------------------------------------------------------------------
// The stage and its scaling

/** The element of a click that has a node behind it, and the key of that node. */
const nodeAt = (target: EventTarget | null): HTMLElement | null =>
	target instanceof Element ? target.closest<HTMLElement>('[data-node-path]') : null;

/** Draws a prepared tree (overrides already applied) in the look the options pick. */
export function PreviewTree({ root, options: o }: { root: LayoutNode; options: RenderOptions }) {
	const look = lookFor(o.renderer, o.theme, str(root, 'style'));
	const cx: Ctx = {
		look,
		renderer: o.renderer,
		vars: { ...SAMPLE, ...o.vars },
		state: o.state ?? {},
		components: o.components ?? [],
		interact: o.interact,
		live: o.live,
		pathOf: o.pathOf,
	};
	const pt = split(root);
	const { w, h } = STAGE[look];
	const editing = o.interact?.mode === 'edit';
	const hover = useRef<HTMLElement | null>(null);
	const setHover = (el: HTMLElement | null) => {
		if (hover.current === el) return;
		hover.current?.removeAttribute('data-hover');
		el?.setAttribute('data-hover', 'true');
		hover.current = el;
	};
	return (
		<StageBox look={look} w={w} h={h} contain={o.fit === 'contain'}>
			<div
				className={pv(
					'pv',
					`pv-${look}`,
					o.renderer === 'tui' && 'pv-tui',
					editing && 'pv-editing',
				)}
				data-renderer={o.renderer}
				data-look={look}
				data-mode={o.interact?.mode}
				onClick={
					editing
						? (e) => {
								const el = nodeAt(e.target);
								if (!el) return;
								e.preventDefault();
								o.interact?.onSelect?.(el.dataset.nodePath ?? '');
							}
						: undefined
				}
				onMouseOver={editing ? (e) => setHover(nodeAt(e.target)) : undefined}
				onMouseLeave={editing ? () => setHover(null) : undefined}
			>
				{look === 'dos' ? (
					<DosFrame cx={cx} pt={pt} />
				) : look === 'win95' ? (
					<WinFrame cx={cx} pt={pt} />
				) : (
					<ModernFrame cx={cx} pt={pt} />
				)}
			</div>
		</StageBox>
	);
}

/**
 * The layout as a screen, drawn for `renderer`: the page's layout inside the theme's `stage` when it
 * has one, with `@renderer` overrides applied first. A click selects the node it hit by the key
 * `s:` (stage) or `p:` (page) and its path, so an editor can map it back.
 */
export function LayoutPreview({
	layout,
	stage = null,
	step = { id: '', kind: 'page' },
	renderer,
	options: o = {},
}: {
	layout: Layout;
	stage?: Layout | null;
	step?: StepInfo;
	renderer: string;
	options?: Partial<RenderOptions>;
}) {
	const screen = useMemo(
		() =>
			isStageId(layout.id)
				? composeScreen(resolveForRenderer(layout.root, renderer), null, step)
				: composeScreen(
						stage ? resolveForRenderer(stage.root, renderer) : null,
						resolveForRenderer(layout.root, renderer),
						step,
					),
		[layout, stage, renderer, step],
	);
	const pathOf = useMemo(
		() => (node: LayoutNode) => {
			const ref = screen.refs.get(node);
			return ref ? refKey(ref) : undefined;
		},
		[screen],
	);
	return <PreviewTree root={screen.root} options={{ ...o, renderer, pathOf }} />;
}

/**
 * The box that scales its first child, the stage. By default it fits the box's width (never above
 * natural size) and takes the stage's height. With `contain` it fills its parent and fits the whole
 * stage inside it, larger than natural size when there is room, centred.
 */
function StageBox({
	look,
	w,
	h,
	contain = false,
	children,
}: {
	look: Look;
	w: number;
	h: number;
	contain?: boolean;
	children: ReactNode;
}) {
	const box = useRef<HTMLDivElement>(null);
	useLayoutEffect(() => {
		const el = box.current;
		if (!el) return;
		const fit = () => {
			const stage = el.firstElementChild as HTMLElement | null;
			if (contain) {
				const scale = Math.min(el.clientWidth / w, el.clientHeight / h) || 1;
				const dx = Math.max(0, (el.clientWidth - w * scale) / 2);
				const dy = Math.max(0, (el.clientHeight - h * scale) / 2);
				if (stage) stage.style.transform = `translate(${dx}px, ${dy}px) scale(${scale})`;
				return;
			}
			const scale = Math.min(1, el.clientWidth / w) || 1;
			if (stage) stage.style.transform = `scale(${scale})`;
			el.style.height = `${h * scale}px`;
		};
		fit();
		if (typeof ResizeObserver === 'undefined') return;
		const observer = new ResizeObserver(fit);
		observer.observe(el);
		return () => observer.disconnect();
	}, [w, h, look, contain]);
	return (
		<div ref={box} className={pv('pv-fit', contain && 'pv-contain')} data-w={w} data-h={h}>
			{children}
		</div>
	);
}
