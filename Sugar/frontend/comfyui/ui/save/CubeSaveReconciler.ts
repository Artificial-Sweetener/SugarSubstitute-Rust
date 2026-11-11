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
/**
 * Reconcile the live graph to authoritative definitions returned by cube saves.
 */

import { buildCubeDefinitionKey } from '../core/CubeDefinitionKey.js';
import { updateMarkersForIds } from '../graph/CubeMarkers.js';
import { getGraphGroups } from '../graph/GraphQuery.js';
import {
  flattenCubeGroupMetadata,
  getGroupSugarcubes,
  setGroupSugarcubes,
  writeCubeDefinitionMetadata,
} from '../graph/GroupMetadata.js';
import { isRecord } from '../types/common.js';
import type { CubeNodeIdentityUpdates } from '../cube/node/CubeNodeIdentityWriter.js';
import type { FinalizedDefinition } from '../core/FinalizedDefinition.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyGraph, GraphId } from '../types/graph.js';
import {
  parseCubeDefinitionDocument,
  type CubeDefinitionDocument,
} from '../workflow/CubeDefinitionDocumentWriter.js';

const WORKTREE_REVISION = 'WORKTREE';

interface DefinitionStore {
  publishFinalized(request: DefinitionRequest, payload: UnknownRecord): unknown;
}

interface InstanceRefresher {
  refresh?(options: { graph: ComfyGraph; reason: string }): unknown;
}

interface FlavorHydrator {
  hydrateFromDefinition?(options: {
    cubeId: string;
    definitionKey: string;
    entry: unknown;
    graph: ComfyGraph;
    forceApply: boolean;
  }): Promise<unknown> | unknown;
}

interface DirtyStateCoordinator {
  acceptFinalizedDefinitions?(options: {
    graph: ComfyGraph;
    entries: readonly FinalizedDefinition[];
  }): unknown;
  addSavedIds?(cubeIds: readonly string[]): unknown;
  markClean?(options: { graph: ComfyGraph; cubeIds: readonly string[] }): unknown;
}

interface CubeNodeIdentityPort {
  updateIdentities(instanceIds: readonly string[], updates: CubeNodeIdentityUpdates): number;
  updateDocuments(
    instanceIds: readonly string[],
    document: CubeDefinitionDocument,
    identity: { cubeId: string; cubeVersion: string },
  ): number;
}

interface CubeSaveReconcilerDependencies {
  definitionStore: DefinitionStore;
  instanceManager?: InstanceRefresher | null;
  flavorService?: FlavorHydrator | null;
  dirtyManager?: DirtyStateCoordinator | null;
  cubeNodeSave?: CubeNodeIdentityPort | null;
}

interface DefinitionRequest {
  cubeId: string;
  cubeVersion: string;
  revisionRef: string;
  definitionKey: string;
}

export interface SavedCubeResult extends UnknownRecord {
  cube_id?: unknown;
  version?: unknown;
  definition?: unknown;
}

export interface SaveReconciliationOptions {
  graph: ComfyGraph;
  saved?: readonly SavedCubeResult[];
  fallbackCubeIds?: readonly unknown[];
  markerIdsByCubeId?: Readonly<Record<string, readonly GraphId[] | undefined>>;
  cubeNodeInstanceIdsByCubeId?: Readonly<Record<string, readonly string[] | undefined>>;
  reason?: string;
}

export interface SaveReconciliationResult {
  cubeIds: string[];
  entries: FinalizedDefinition[];
}

interface AlignIdentityOptions {
  graph: ComfyGraph;
  cubeId: string;
  cubeVersion: string;
  definitionKey: string;
  markerIds: readonly GraphId[];
}

interface PreparedDefinitionResponse extends UnknownRecord {
  cube: UnknownRecord;
  document: CubeDefinitionDocument;
}

interface PreparedSavedDefinition {
  cubeId: string;
  cubeVersion: string;
  definition: PreparedDefinitionResponse;
  document: CubeDefinitionDocument;
}

/** Coordinate post-save definition, instance, preset, and dirty-state updates. */
export class CubeSaveReconciler {
  private readonly definitionStore: DefinitionStore;
  private readonly instanceManager: InstanceRefresher | null;
  private readonly flavorService: FlavorHydrator | null;
  private readonly dirtyManager: DirtyStateCoordinator | null;
  private readonly cubeNodeSave: CubeNodeIdentityPort | null;

  constructor({
    definitionStore,
    instanceManager = null,
    flavorService = null,
    dirtyManager = null,
    cubeNodeSave = null,
  }: CubeSaveReconcilerDependencies) {
    this.definitionStore = definitionStore;
    this.instanceManager = instanceManager;
    this.flavorService = flavorService;
    this.dirtyManager = dirtyManager;
    this.cubeNodeSave = cubeNodeSave;
  }

  /** Reconcile successful save results before control returns to the caller. */
  async reconcile({
    graph,
    saved,
    fallbackCubeIds = [],
    markerIdsByCubeId = {},
    cubeNodeInstanceIdsByCubeId = {},
    reason = 'cube-save',
  }: SaveReconciliationOptions): Promise<SaveReconciliationResult> {
    const finalized = this.publishDefinitions(
      graph,
      saved,
      markerIdsByCubeId,
      cubeNodeInstanceIdsByCubeId,
    );
    const savedCubeIds = finalized.map(({ cubeId }) => cubeId);
    const cubeIds = savedCubeIds.length ? savedCubeIds : normalizeCubeIds(fallbackCubeIds);
    if (!finalized.length && cubeIds.length) {
      throw new Error('Cube save response is missing finalized definitions');
    }

    this.instanceManager?.refresh?.({ graph, reason });
    for (const result of finalized) {
      await this.flavorService?.hydrateFromDefinition?.({
        cubeId: result.cubeId,
        definitionKey: result.definitionKey,
        entry: result.entry,
        graph,
        forceApply: true,
      });
    }
    this.dirtyManager?.acceptFinalizedDefinitions?.({ graph, entries: finalized });
    this.dirtyManager?.addSavedIds?.(cubeIds);
    this.dirtyManager?.markClean?.({ graph, cubeIds });
    return { cubeIds, entries: finalized };
  }

  /** Publish each persisted definition and align marker definition identity. */
  publishDefinitions(
    graph: ComfyGraph,
    saved: readonly SavedCubeResult[] | undefined,
    markerIdsByCubeId: Readonly<Record<string, readonly GraphId[] | undefined>>,
    cubeNodeInstanceIdsByCubeId: Readonly<Record<string, readonly string[] | undefined>> = {},
  ): FinalizedDefinition[] {
    const results: FinalizedDefinition[] = [];
    const preparedDefinitions = prepareSavedDefinitions(saved);
    for (const { cubeId, cubeVersion, definition, document } of preparedDefinitions) {
      const definitionKey = buildCubeDefinitionKey(cubeId, cubeVersion);
      const markerIds = markerIdsByCubeId[cubeId] ?? [];
      const cubeNodeInstanceIds = cubeNodeInstanceIdsByCubeId[cubeId] ?? [];
      if (!markerIds.length && !cubeNodeInstanceIds.length) {
        throw new Error(`Cube save reconciliation targets are missing for '${cubeId}'`);
      }
      updateMarkersForIds(graph, markerIds, {
        cubeVersion,
        cubeRevisionRef: WORKTREE_REVISION,
      });
      if (cubeNodeInstanceIds.length) {
        this.cubeNodeSave?.updateIdentities(cubeNodeInstanceIds, {
          cubeVersion,
          cubeRevisionRef: WORKTREE_REVISION,
          cubeDefinitionKey: definitionKey,
        });
        this.cubeNodeSave?.updateDocuments(cubeNodeInstanceIds, document, {
          cubeId,
          cubeVersion,
        });
      }
      this.alignTargetGroupIdentity({
        graph,
        cubeId,
        cubeVersion,
        definitionKey,
        markerIds,
      });
      const entry = this.definitionStore.publishFinalized(
        {
          cubeId,
          cubeVersion,
          revisionRef: WORKTREE_REVISION,
          definitionKey,
        },
        definition,
      );
      results.push({ cubeId, cubeVersion, definitionKey, entry });
    }
    return results;
  }

  /** Align only groups whose markers supplied this save. */
  alignTargetGroupIdentity({
    graph,
    cubeId,
    cubeVersion,
    definitionKey,
    markerIds,
  }: AlignIdentityOptions): void {
    const targetMarkerIds = new Set(markerIds.map(String));
    for (const group of getGraphGroups(graph)) {
      const metadata = getGroupSugarcubes(group);
      if (!metadata?.managed || metadata.cube_id !== cubeId) {
        continue;
      }
      const groupMarkerIds = readMetadataMarkerIds(metadata);
      if (!groupMarkerIds.some((markerId) => targetMarkerIds.has(markerId))) {
        continue;
      }
      const definitionMetadata = writeCubeDefinitionMetadata(metadata, {
        cube_version: cubeVersion,
        cube_revision_ref: WORKTREE_REVISION,
        cube_definition_key: definitionKey,
      });
      setGroupSugarcubes(group, flattenCubeGroupMetadata(definitionMetadata, metadata));
    }
  }
}

/** Validate every finalized backend definition before mutating the live graph. */
function prepareSavedDefinitions(
  saved: readonly SavedCubeResult[] | undefined,
): PreparedSavedDefinition[] {
  const prepared: PreparedSavedDefinition[] = [];
  for (const savedEntry of Array.isArray(saved) ? saved : []) {
    if (!isRecord(savedEntry.definition)) {
      continue;
    }
    const definition = savedEntry.definition;
    if (!isRecord(definition.cube)) {
      throw new TypeError('Cube save response definition.cube must be an object.');
    }
    const document = parseCubeDefinitionDocument(definition.document);
    const cubeId = document.cube_id;
    const cubeVersion = document.version;
    requireMatchingIdentity(savedEntry.cube_id, cubeId, 'saved cube_id');
    requireMatchingIdentity(savedEntry.version, cubeVersion, 'saved version');
    requireMatchingIdentity(definition.cube.cube_id, cubeId, 'definition.cube.cube_id');
    requireMatchingIdentity(definition.cube.version, cubeVersion, 'definition.cube.version');
    prepared.push({
      cubeId,
      cubeVersion,
      definition: { ...definition, cube: definition.cube, document },
      document,
    });
  }
  return prepared;
}

/** Reject contradictory response identity before any graph reconciliation. */
function requireMatchingIdentity(value: unknown, canonical: string, path: string): void {
  const supplied = readString(value);
  if (supplied && supplied !== canonical) {
    throw new TypeError(`Cube save response ${path} disagrees with definition.document.`);
  }
}

function normalizeCubeIds(values: readonly unknown[]): string[] {
  return Array.from(new Set(values.map(readString).filter(Boolean)));
}

function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function readMetadataMarkerIds(metadata: UnknownRecord): string[] {
  if (Array.isArray(metadata.markers)) {
    return metadata.markers.map(String);
  }
  const markers = metadata.markers;
  if (!isRecord(markers)) {
    return [];
  }
  return Object.values(markers).flatMap((values) =>
    Array.isArray(values) ? values.map(String) : [],
  );
}
