// The shared renderer: draws an installer's layout trees as themed React elements.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export { ArtSvg, LayoutPreview, PreviewTree } from './PreviewTree';
export {
	SAMPLE,
	SCHEMES,
	STAGE,
	fill,
	hasMovingParts,
	hasSchemes,
	isModern,
	lookFor,
	type Interact,
	type Live,
	type Look,
	type RenderOptions,
	type Scheme,
} from './model';
export { resolveForRenderer } from './resolve';
export {
	STAGE_NAME,
	attachStage,
	composeScreen,
	detachStage,
	isStageId,
	parseRef,
	refKey,
	screenRows,
	shownOn,
	stageIdFor,
	type Ref,
	type Row,
	type Scope,
	type Screen,
	type StepInfo,
} from './stage';
export { rendererMessages, setRendererCatalogue, type RendererMessageId } from './i18n/messages';
export type { Layout, LayoutNode } from './types';
