// The types the UI shares with the Rust side, re-exported from the generated protocol package.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Everything here comes from `@liminal-hq/installer-protocol`, which ts-rs writes from the Rust
// types (`bash scripts/generate-protocol-bindings.sh`). Edit the Rust type, not a copy of it.
// A few names keep the spelling the UI already used.
export type { Branch } from '@liminal-hq/installer-protocol/generated/Branch';
export type { CellGrid } from '@liminal-hq/installer-protocol/generated/CellGrid';
export type { Cell } from '@liminal-hq/installer-protocol/generated/Cell';
export type { ChangeEvent } from '@liminal-hq/installer-protocol/generated/ChangeEvent';
export type { ComponentInfo } from '@liminal-hq/installer-protocol/generated/ComponentInfo';
export type { DocKey } from '@liminal-hq/installer-protocol/generated/DocKey';
export type { ErrorKind } from '@liminal-hq/installer-protocol/generated/ErrorKind';
export type { ErrorPayload } from '@liminal-hq/installer-protocol/generated/ErrorPayload';
export type { Flow } from '@liminal-hq/installer-protocol/generated/Flow';
export type { FlowInfo } from '@liminal-hq/installer-protocol/generated/FlowInfo';
export type { FlowView } from '@liminal-hq/installer-protocol/generated/FlowView';
export type { HistoryView } from '@liminal-hq/installer-protocol/generated/HistoryView';
export type { HookInfo } from '@liminal-hq/installer-protocol/generated/HookInfo';
export type { HookRun } from '@liminal-hq/installer-protocol/generated/HookRun';
export type { Layout } from '@liminal-hq/installer-protocol/generated/Layout';
export type { McpInfo } from '@liminal-hq/installer-protocol/generated/McpInfo';
export type { Operation } from '@liminal-hq/installer-protocol/generated/Operation';
export type { Plan } from '@liminal-hq/installer-protocol/generated/Plan';
export type { Preview } from '@liminal-hq/installer-protocol/generated/Preview';
export type { PreviewProduct } from '@liminal-hq/installer-protocol/generated/PreviewProduct';
export type { PreviewStep } from '@liminal-hq/installer-protocol/generated/PreviewStep';
export type { Problem } from '@liminal-hq/installer-protocol/generated/Problem';
export type { ProjectSummary } from '@liminal-hq/installer-protocol/generated/ProjectSummary';
export type { Screenshot } from '@liminal-hq/installer-protocol/generated/Screenshot';
export type { ScriptDiagnostic } from '@liminal-hq/installer-protocol/generated/ScriptDiagnostic';
export type { ScriptReport } from '@liminal-hq/installer-protocol/generated/ScriptReport';
export type { Severity } from '@liminal-hq/installer-protocol/generated/Severity';
export type { Source } from '@liminal-hq/installer-protocol/generated/Source';
export type { Step } from '@liminal-hq/installer-protocol/generated/Step';
export type { StepType } from '@liminal-hq/installer-protocol/generated/StepType';
export type { TargetInfo } from '@liminal-hq/installer-protocol/generated/TargetInfo';
export type { ThemeInfo } from '@liminal-hq/installer-protocol/generated/ThemeInfo';
export type { InstallMode } from '@liminal-hq/installer-protocol/generated/InstallMode';
export type { TauriDrift } from '@liminal-hq/installer-protocol/generated/TauriDrift';
export type { TauriFieldChange } from '@liminal-hq/installer-protocol/generated/TauriFieldChange';
export type { TauriIdentityRow } from '@liminal-hq/installer-protocol/generated/TauriIdentityRow';
export type { TauriImportResult } from '@liminal-hq/installer-protocol/generated/TauriImportResult';
export type { TauriModeInfo } from '@liminal-hq/installer-protocol/generated/TauriModeInfo';
export type { TauriSettingsChange } from '@liminal-hq/installer-protocol/generated/TauriSettingsChange';
export type { TauriUpdates } from '@liminal-hq/installer-protocol/generated/TauriUpdates';
export type { TauriView } from '@liminal-hq/installer-protocol/generated/TauriView';
export type { TauriWebview2 } from '@liminal-hq/installer-protocol/generated/TauriWebview2';
export type { UpdateMode } from '@liminal-hq/installer-protocol/generated/UpdateMode';
export type { Updates } from '@liminal-hq/installer-protocol/generated/Updates';
export type { Webview2Mode } from '@liminal-hq/installer-protocol/generated/Webview2Mode';
export type { Webview2Override } from '@liminal-hq/installer-protocol/generated/Webview2Override';
export type { Webview2Setting } from '@liminal-hq/installer-protocol/generated/Webview2Setting';
export type { TuiFrame } from '@liminal-hq/installer-protocol/generated/TuiFrame';
export type { TuiKey } from '@liminal-hq/installer-protocol/generated/TuiKey';
export type { TuiKeyResult } from '@liminal-hq/installer-protocol/generated/TuiKeyResult';
export type { TuiOpened } from '@liminal-hq/installer-protocol/generated/TuiOpened';
export type { TuiOutcome } from '@liminal-hq/installer-protocol/generated/TuiOutcome';
export type { Node as LayoutNode } from '@liminal-hq/installer-protocol/generated/Node';
export type { EntrySummary as HistoryEntry } from '@liminal-hq/installer-protocol/generated/EntrySummary';
export type { CommonMode } from '@liminal-hq/installer-protocol/generated/CommonMode';
export type { ComponentSize } from '@liminal-hq/installer-protocol/generated/ComponentSize';
export type { DiskEntry } from '@liminal-hq/installer-protocol/generated/DiskEntry';
export type { EntryDest } from '@liminal-hq/installer-protocol/generated/EntryDest';
export type { EntryRole } from '@liminal-hq/installer-protocol/generated/EntryRole';
export type { FileSize } from '@liminal-hq/installer-protocol/generated/FileSize';
export type { MediaEdit } from '@liminal-hq/installer-protocol/generated/MediaEdit';
export type { MediaInfo } from '@liminal-hq/installer-protocol/generated/MediaInfo';
export type { MediaKind } from '@liminal-hq/installer-protocol/generated/MediaKind';
export type { MediaPlan } from '@liminal-hq/installer-protocol/generated/MediaPlan';
export type { MediaSeverity } from '@liminal-hq/installer-protocol/generated/MediaSeverity';
export type { MediaTarget } from '@liminal-hq/installer-protocol/generated/MediaTarget';
export type { MediaTargetPlan } from '@liminal-hq/installer-protocol/generated/MediaTargetPlan';
export type { MediaWarning } from '@liminal-hq/installer-protocol/generated/MediaWarning';
export type { MediaWarningCode } from '@liminal-hq/installer-protocol/generated/MediaWarningCode';
export type { Overflow } from '@liminal-hq/installer-protocol/generated/Overflow';
export type { PackEntryPlan } from '@liminal-hq/installer-protocol/generated/PackEntryPlan';
export type { PackSummary } from '@liminal-hq/installer-protocol/generated/PackSummary';
export type { PackWhich } from '@liminal-hq/installer-protocol/generated/PackWhich';
export type { ReadmePlan } from '@liminal-hq/installer-protocol/generated/ReadmePlan';
export type { VolumePlan } from '@liminal-hq/installer-protocol/generated/VolumePlan';
