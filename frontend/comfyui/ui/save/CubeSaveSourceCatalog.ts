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
/** Project live graph and catalog state into save-source metadata. */

import {
  defaultSupportedModelsForTarget,
  deriveRouteFromCubeId,
  deriveTargetModelFromCubeId,
  normalizeSupportedModels,
  normalizeTargetModel,
} from '../core/ModelTargets.js';
import {
  buildCubeDefinitionKey,
  isCurrentRevisionRef,
  normalizeCubeVersion,
  normalizeRevisionRef,
} from '../core/CubeDefinitionKey.js';
import { InstanceBuilder } from '../graph/InstanceBuilder.js';
import { getGraphGroups } from '../graph/GraphQuery.js';
import { getGroupSugarcubes } from '../graph/GroupMetadata.js';
import type { CubeInstance } from '../graph/InstanceBuilder.js';
import type { CubeGroupMetadataRecord } from '../graph/GroupMetadata.js';
import type {
  CubeBrowserEntry,
  CubeNodeSavePort,
  SaveAdapter,
  SaveEntryMetadata,
  SaveRefreshCoordinator,
  SaveSourceEntry,
  SaveSourceSelection,
} from './CubeSaveContracts.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type { ComfyGraph, GraphId } from '../types/graph.js';

interface CubeSaveSourceCatalogOptions {
  adapter: SaveAdapter;
  instanceManager: SaveRefreshCoordinator | null;
  cubeNodeSave: CubeNodeSavePort | null;
}

/** Own save-source discovery and metadata projection. */
export class CubeSaveSourceCatalog {
  constructor(private readonly options: CubeSaveSourceCatalogOptions) {}

  /** Index native and legacy Cube instances by persisted identity. */
  buildIndex(graph: ComfyGraph): Map<string, SaveSourceEntry[]> {
    const byCubeId = new Map<string, SaveSourceEntry[]>();
    const addEntry = (entry: SaveSourceEntry): void => {
      const cubeId = typeof entry.cubeId === 'string' ? entry.cubeId.trim() : '';
      if (!cubeId) return;
      const list = byCubeId.get(cubeId) || [];
      const dedupeKey = `${entry.instanceId}|${entry.sourceDefinitionKey}|${entry.markerIds.join(',')}`;
      if (
        dedupeKey.trim() &&
        list.some(
          (existing) =>
            `${existing.instanceId}|${existing.sourceDefinitionKey}|${existing.markerIds.join(',')}` ===
            dedupeKey,
        )
      ) {
        return;
      }
      list.push(entry);
      byCubeId.set(cubeId, list);
    };

    for (const instance of this.options.cubeNodeSave?.listInstances() ?? []) {
      const sourceRevisionRef = normalizeRevisionRef(instance.cubeRevisionRef);
      const sourceVersion = normalizeCubeVersion(instance.cubeVersion);
      addEntry({
        cubeId: instance.cubeId,
        defaultAlias: instance.defaultAlias,
        sourceVersion,
        sourceRevisionRef,
        sourceDefinitionKey:
          instance.cubeDefinitionKey || buildCubeDefinitionKey(instance.cubeId, sourceVersion),
        staleRevision: !isCurrentRevisionRef(sourceRevisionRef),
        targetModel: instance.targetModel,
        supportedModels: instance.supportedModels,
        description: instance.description,
        markerIds: [],
        group: null,
        instanceId: instance.instanceId,
        definitionId: instance.definitionId,
        cubeNodeInstanceId: instance.instanceId,
        cubeNodeId: instance.nodeId,
        surfaceSize: instance.surfaceSize,
        surfaceState: instance.surfaceState,
      });
    }

    for (const group of getGraphGroups(graph)) {
      const metadata = getGroupSugarcubes(group);
      if (!metadata?.managed) continue;
      const cubeId = typeof metadata.cube_id === 'string' ? metadata.cube_id.trim() : '';
      if (!cubeId) continue;
      const sourceVersion = normalizeCubeVersion(metadata.cube_version);
      const sourceRevisionRef = normalizeRevisionRef(metadata.cube_revision_ref);
      const sourceDefinitionKey =
        typeof metadata.cube_definition_key === 'string' && metadata.cube_definition_key.trim()
          ? metadata.cube_definition_key.trim()
          : buildCubeDefinitionKey(cubeId, sourceVersion);
      addEntry({
        cubeId,
        defaultAlias:
          typeof metadata.default_alias === 'string' && metadata.default_alias.trim()
            ? metadata.default_alias.trim()
            : cubeId,
        sourceVersion,
        sourceRevisionRef,
        sourceDefinitionKey,
        staleRevision: !isCurrentRevisionRef(sourceRevisionRef),
        targetModel:
          typeof metadata.target_model === 'string' && metadata.target_model.trim()
            ? metadata.target_model.trim()
            : '',
        supportedModels: Array.isArray(metadata.supported_models)
          ? metadata.supported_models.filter((value) => typeof value === 'string')
          : [],
        description: null,
        markerIds: this.extractMarkerIds(metadata),
        group,
        instanceId:
          typeof metadata.instance_id === 'string' && metadata.instance_id.trim()
            ? metadata.instance_id.trim()
            : '',
        definitionId: '',
        cubeNodeInstanceId: null,
        cubeNodeId: null,
        surfaceSize: null,
        surfaceState: null,
      });
    }

    const builder =
      this.options.instanceManager?.instanceBuilder ||
      new InstanceBuilder({ logger: this.options.adapter.getConsole?.() ?? null });
    for (const instance of builder.build(graph)) {
      if (!instance.cubeId) continue;
      const sourceRevisionRef = normalizeRevisionRef(instance.cubeRevisionRef);
      const sourceVersion = normalizeCubeVersion(instance.cubeVersion);
      addEntry({
        cubeId: instance.cubeId,
        defaultAlias: instance.defaultAlias || instance.cubeId,
        sourceVersion,
        sourceRevisionRef,
        sourceDefinitionKey:
          instance.cubeDefinitionKey || buildCubeDefinitionKey(instance.cubeId, sourceVersion),
        staleRevision: !isCurrentRevisionRef(sourceRevisionRef),
        targetModel: instance.targetModel || '',
        supportedModels: readInstanceSupportedModels(instance),
        description: null,
        markerIds: Array.isArray(instance.markerIds) ? instance.markerIds : [],
        group: null,
        instanceId: instance.instanceId || '',
        definitionId: '',
        cubeNodeInstanceId: null,
        cubeNodeId: null,
        surfaceSize: null,
        surfaceState: null,
      });
    }
    return byCubeId;
  }

  /** Select the source metadata that governs one save-plan entry. */
  resolve(cubeId: string, index: ReadonlyMap<string, SaveSourceEntry[]>): SaveSourceSelection {
    const entries = index.get(cubeId) ?? [];
    const staleEntries = entries.filter((entry) => entry.staleRevision);
    const selected = staleEntries[0] || entries[0] || null;
    return {
      sourceEntries: entries,
      sourceRevisionRef: selected?.sourceRevisionRef || '',
      sourceVersion: selected?.sourceVersion || '',
      sourceDefinitionKey: selected?.sourceDefinitionKey || '',
      staleRevision: Boolean(staleEntries.length),
      staleSaveMode: '',
      selectedSourceEntry: selected,
      defaultAlias: selected?.defaultAlias || '',
      targetModel: selected?.targetModel || '',
      supportedModels: selected?.supportedModels || [],
    };
  }

  /** Build backend metadata from the authoritative source and catalog views. */
  buildMetadata(
    cubeId: string,
    browserEntry: CubeBrowserEntry | null,
    sourceMetadata: SaveSourceSelection | null = null,
  ): SaveEntryMetadata | null {
    let defaultAlias = sourceMetadata?.defaultAlias?.trim() || '';
    if (!defaultAlias) {
      try {
        defaultAlias = deriveRouteFromCubeId(cubeId);
      } catch (_error) {
        defaultAlias = '';
      }
    }
    const targetModel =
      normalizeTargetModel(browserEntry?.target_model) ||
      normalizeTargetModel(sourceMetadata?.targetModel) ||
      this.deriveTargetModel(cubeId);
    if (!defaultAlias && !targetModel) return null;
    const supportedSource = Array.isArray(browserEntry?.supported_models)
      ? browserEntry.supported_models
      : Array.isArray(sourceMetadata?.supportedModels)
        ? sourceMetadata.supportedModels
        : defaultSupportedModelsForTarget(targetModel);
    return {
      ...(defaultAlias ? { default_alias: defaultAlias } : {}),
      target_model: targetModel,
      supported_models: normalizeSupportedModels(supportedSource, { targetModel }),
      ...(sourceMetadata?.selectedSourceEntry?.surfaceSize
        ? { surface_size: [...sourceMetadata.selectedSourceEntry.surfaceSize] as Vec2 }
        : {}),
      ...(sourceMetadata?.selectedSourceEntry?.surfaceState
        ? { surface_state: sourceMetadata.selectedSourceEntry.surfaceState }
        : {}),
    };
  }

  /** Index browser entries by canonical Cube id. */
  indexBrowserEntries(entries: readonly CubeBrowserEntry[]): Map<string, CubeBrowserEntry> {
    const map = new Map<string, CubeBrowserEntry>();
    for (const entry of Array.isArray(entries) ? entries : []) {
      const cubeId = typeof entry.cube_id === 'string' ? entry.cube_id.trim() : '';
      if (cubeId) map.set(cubeId, entry);
    }
    return map;
  }

  private extractMarkerIds(metadata: CubeGroupMetadataRecord): GraphId[] {
    const rawMarkers = metadata.markers;
    if (Array.isArray(rawMarkers)) return rawMarkers.filter((value) => value != null);
    if (!rawMarkers || typeof rawMarkers !== 'object') return [];
    return Object.values(rawMarkers).flatMap((value) =>
      Array.isArray(value) ? value.filter((entry) => entry != null) : [],
    );
  }

  private deriveTargetModel(cubeId: string): string {
    try {
      return normalizeTargetModel(deriveTargetModelFromCubeId(cubeId));
    } catch (_error) {
      return '';
    }
  }
}

function readInstanceSupportedModels(instance: CubeInstance): string[] {
  const value = (instance as unknown as UnknownRecord).supportedModels;
  return Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === 'string')
    : [];
}
