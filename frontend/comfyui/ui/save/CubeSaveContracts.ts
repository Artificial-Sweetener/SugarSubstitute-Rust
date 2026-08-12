//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Define the collaboration contracts for the Cube save workflow. */

import type { CubeNodeInstance } from '../cube/node/CubeNodeInstanceCatalog.js';
import type { CubeNodeIdentityUpdates } from '../cube/node/CubeNodeIdentityWriter.js';
import type { ApiJsonResult } from '../core/CubeLibraryApi.js';
import type { CubeInstance } from '../graph/InstanceBuilder.js';
import type { SavedCubeResult, SaveReconciliationResult } from './CubeSaveReconciler.js';
import type { CubeDefaultReviewService } from './CubeDefaultReviewService.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type { VersionSuggestion } from '../dialogs/VersionDialog.js';
import type { ComfyApplication, ComfyGraph, ComfyGroup, GraphId } from '../types/graph.js';

/** Identify a historical save that should update the latest definition. */
export const STALE_SAVE_MODE_LATEST = 'latest';

export type CubeSaveStatus = 'saved' | 'no_changes' | 'cancelled' | 'failed';

export interface CubeSaveOutcome {
  status: CubeSaveStatus;
  savedCubeIds: string[];
  message?: string;
}

export interface CubeBrowserEntry extends UnknownRecord {
  cube_id?: string;
  name?: string;
  version?: string;
  author?: string;
  author_url?: string;
  target_model?: string;
  supported_models?: string[];
  is_writable?: boolean;
}

export interface SaveSourceEntry {
  cubeId: string;
  defaultAlias: string;
  sourceVersion: string;
  sourceRevisionRef: string;
  sourceDefinitionKey: string;
  staleRevision: boolean;
  targetModel: string;
  supportedModels: string[];
  description: string | null;
  markerIds: GraphId[];
  group: ComfyGroup | null;
  instanceId: string;
  definitionId: string;
  cubeNodeInstanceId: string | null;
  cubeNodeId: string | null;
  surfaceSize: Vec2 | null;
  surfaceState: UnknownRecord | null;
}

export interface SaveSourceSelection {
  sourceEntries: SaveSourceEntry[];
  sourceRevisionRef: string;
  sourceVersion: string;
  sourceDefinitionKey: string;
  staleRevision: boolean;
  staleSaveMode: string;
  selectedSourceEntry: SaveSourceEntry | null;
  defaultAlias: string;
  targetModel: string;
  supportedModels: string[];
}

export interface SaveEntryMetadata extends UnknownRecord {
  default_alias?: string;
  target_model: string;
  supported_models: string[];
  surface_size?: Vec2;
  surface_state?: UnknownRecord;
}

export interface SavePlanEntry extends SaveSourceSelection {
  cubeId: string;
  forked: boolean;
  lineage: UnknownRecord | null;
  metadata: SaveEntryMetadata | null;
  previousCubeId: string;
  latestVersion: string;
  reconciliationMarkerIds?: GraphId[];
  reconciliationCubeNodeInstanceIds?: string[];
}

export interface SaveButton {
  enabled?: boolean;
  element?: HTMLElement | null;
}

export interface SaveRequest {
  cubeIds?: readonly string[] | null;
  button?: SaveButton | null;
}

export interface SaveAdapter {
  getApp?(): ComfyApplication | null;
  getConsole?(): Console | null;
}

export interface SaveApi {
  saveImplementation(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
}

export interface SaveToast {
  push?(severity: string, summary: string, detail?: string): unknown;
}

export interface SaveRefreshCoordinator {
  instanceBuilder?: { build(graph: ComfyGraph): CubeInstance[] };
  scheduleRefresh?(options: { graph: ComfyGraph; reason: string }): unknown;
}

export interface SaveDirtyCoordinator {
  scheduleRefresh?(options: { graph: ComfyGraph; reason: string }): unknown;
  getImplementationDirtyCubeIds?(): Iterable<string>;
  getDirtyCubeIds?(): Iterable<string>;
}

export interface SaveBrowser {
  getCubes?(): CubeBrowserEntry[];
  refresh?(options: { force: boolean }): Promise<unknown>;
}

export interface SaveVersionDialog {
  open?(suggestions: VersionSuggestion[] | null | undefined): Promise<unknown>;
}

export interface SaveDialogs {
  chooseHistoricalVersionSaveAction?(options: {
    entries: Array<{
      cubeId: string;
      defaultAlias: string;
      sourceVersion: string;
      sourceRevisionRef: string;
    }>;
  }): Promise<unknown>;
}

export interface SaveReconciler {
  reconcile(options: {
    graph: ComfyGraph;
    saved: SavedCubeResult[];
    fallbackCubeIds: string[];
    markerIdsByCubeId: Record<string, string[]>;
    cubeNodeInstanceIdsByCubeId?: Record<string, string[]>;
    reason: string;
  }): Promise<SaveReconciliationResult>;
}

export interface CubeNodeSavePort {
  listInstances(): readonly CubeNodeInstance[];
  updateIdentities(instanceIds: readonly string[], updates: CubeNodeIdentityUpdates): number;
}

export interface SaveCatalogInvalidator {
  invalidate(): Promise<void>;
}

export interface CubeSaveDependencies {
  adapter: SaveAdapter;
  api: SaveApi;
  toast?: SaveToast | null;
  instanceManager?: SaveRefreshCoordinator | null;
  dirtyManager?: SaveDirtyCoordinator | null;
  cubeBrowser?: SaveBrowser | null;
  versionDialog?: SaveVersionDialog | null;
  dialogs?: SaveDialogs | null;
  saveReconciler?: SaveReconciler | null;
  cubeNodeSave?: CubeNodeSavePort | null;
  defaultReview?: CubeDefaultReviewService | null;
  catalogInvalidator?: SaveCatalogInvalidator | null;
}

export interface AssignedCubeId {
  cubeId: string;
  instanceId: string | null;
}
