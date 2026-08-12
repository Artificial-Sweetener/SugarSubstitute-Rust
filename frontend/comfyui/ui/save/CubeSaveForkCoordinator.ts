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
/** Own save-time fork decisions and identity mutation. */

import { suggestPersonalCubeIdentity } from '../create/PersonalCubeIdentity.js';
import { updateMarkersForIds } from '../graph/CubeMarkers.js';
import { getGroupSugarcubes, setGroupSugarcubes } from '../graph/GroupMetadata.js';
import {
  CURRENT_REVISION_REF,
  buildCubeDefinitionKey,
  normalizeCubeVersion,
  normalizeRevisionRef,
} from '../core/CubeDefinitionKey.js';
import {
  STALE_SAVE_MODE_LATEST,
  type CubeBrowserEntry,
  type CubeNodeSavePort,
  type SaveDialogs,
  type SavePlanEntry,
  type SaveSourceEntry,
} from './CubeSaveContracts.js';
import type { CubeSaveIdentityAssigner } from './CubeSaveIdentityAssigner.js';
import type { CubeSaveSourceCatalog } from './CubeSaveSourceCatalog.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyGraph } from '../types/graph.js';

interface CubeSaveForkCoordinatorOptions {
  dialogs: SaveDialogs | null;
  cubeNodeSave: CubeNodeSavePort | null;
  identities: CubeSaveIdentityAssigner;
  sources: CubeSaveSourceCatalog;
}

/** Coordinate explicit historical and read-only Cube forks. */
export class CubeSaveForkCoordinator {
  constructor(private readonly options: CubeSaveForkCoordinatorOptions) {}

  /** Resolve the user's policy for saving historical revisions. */
  async resolveHistoricalChoice(
    savePlan: readonly SavePlanEntry[],
  ): Promise<typeof STALE_SAVE_MODE_LATEST | 'fork' | '' | null> {
    const staleEntries = savePlan.filter((entry) => entry.staleRevision);
    if (!staleEntries.length) return '';
    const choice = await this.options.dialogs?.chooseHistoricalVersionSaveAction?.({
      entries: staleEntries.map((entry) => ({
        cubeId: entry.cubeId,
        defaultAlias: entry.defaultAlias || entry.cubeId,
        sourceVersion: entry.sourceVersion,
        sourceRevisionRef: entry.sourceRevisionRef,
      })),
    });
    return choice === STALE_SAVE_MODE_LATEST || choice === 'fork' ? choice : null;
  }

  /** Fork every stale source selected by the user. */
  async forkStaleEntries(
    graph: ComfyGraph,
    savePlan: SavePlanEntry[],
    cubeIndex: ReadonlyMap<string, CubeBrowserEntry>,
    usedNames: Set<string>,
  ): Promise<void> {
    for (const entry of savePlan) {
      if (!entry.staleRevision) continue;
      const sourceEntries = entry.sourceEntries.filter((source) => source.staleRevision);
      const markerIds = Array.from(
        new Set(sourceEntries.flatMap((source) => source.markerIds || []).filter(Boolean)),
      );
      const cubeNodeInstanceIds = Array.from(
        new Set(
          sourceEntries
            .map((source) => source.cubeNodeInstanceId)
            .filter((instanceId): instanceId is string => Boolean(instanceId)),
        ),
      );
      if (!markerIds.length && !cubeNodeInstanceIds.length) {
        throw new Error(
          `Unable to fork '${entry.defaultAlias || entry.cubeId}' (instances missing).`,
        );
      }
      const browserEntry = cubeIndex.get(entry.cubeId) || {};
      const forkedName = this.buildForkedName(
        entry.defaultAlias || browserEntry.name || 'SugarCube',
        usedNames,
      );
      usedNames.add(forkedName);
      const reservedIds = [...cubeIndex.keys(), ...savePlan.map((candidate) => candidate.cubeId)];
      const forkedId = suggestPersonalCubeIdentity(
        forkedName,
        this.options.identities.resolveTargetModel(
          entry.targetModel,
          browserEntry.target_model,
          entry.cubeId,
        ),
        reservedIds,
      ).cubeId;
      const updatedMarkers = updateMarkersForIds(graph, markerIds, {
        cubeId: forkedId,
        defaultAlias: forkedName,
        cubeVersion: '',
        cubeRevisionRef: CURRENT_REVISION_REF,
      });
      const updatedCubeNodes =
        this.options.cubeNodeSave?.updateIdentities(cubeNodeInstanceIds, {
          cubeId: forkedId,
          defaultAlias: forkedName,
          cubeRevisionRef: CURRENT_REVISION_REF,
        }) ?? 0;
      if (!updatedMarkers && !updatedCubeNodes) {
        throw new Error(`Unable to fork '${forkedName}' (instances missing).`);
      }
      for (const source of sourceEntries) {
        this.updateSourceGroupIdentity(source, forkedId, forkedName);
      }
      entry.previousCubeId = entry.cubeId;
      entry.cubeId = forkedId;
      entry.forked = true;
      entry.reconciliationMarkerIds = markerIds;
      entry.reconciliationCubeNodeInstanceIds = cubeNodeInstanceIds;
      entry.lineage = this.buildHistoricalLineage(browserEntry, entry);
      entry.metadata = this.options.sources.buildMetadata(forkedId, browserEntry);
      entry.staleRevision = false;
      entry.sourceEntries = [];
      entry.sourceRevisionRef = '';
      entry.sourceVersion = '';
      entry.sourceDefinitionKey = '';
      entry.staleSaveMode = '';
      entry.latestVersion = '';
      entry.defaultAlias = forkedName;
    }
  }

  /** Derive a unique human-readable fork alias. */
  buildForkedName(baseName: unknown, usedNames: ReadonlySet<string>): string {
    const base = typeof baseName === 'string' && baseName.trim() ? baseName.trim() : 'SugarCube';
    const fallback = `${base} (fork)`;
    if (!usedNames.has(fallback)) return fallback;
    let index = 2;
    while (index < 1000) {
      const next = `${base} (fork ${index})`;
      if (!usedNames.has(next)) return next;
      index += 1;
    }
    return `${base} (fork ${Date.now()})`;
  }

  /** Record lineage for a read-only catalog fork. */
  buildLineage(entry: CubeBrowserEntry | null | undefined): UnknownRecord | null {
    if (!entry) return null;
    return {
      id: entry.cube_id || '',
      name: entry.name || '',
      version: entry.version || '',
      author: entry.author || '',
      author_url: entry.author_url || '',
      forked_at: new Date().toISOString(),
    };
  }

  private updateSourceGroupIdentity(
    source: SaveSourceEntry,
    cubeId: string,
    defaultAlias: string,
  ): void {
    const group = source.group;
    const metadata = getGroupSugarcubes(group);
    if (!metadata) return;
    const nextVersion = normalizeCubeVersion('');
    setGroupSugarcubes(group, {
      ...metadata,
      cube_id: cubeId,
      default_alias: defaultAlias || metadata.default_alias || cubeId,
      cube_version: nextVersion,
      cube_revision_ref: normalizeRevisionRef(CURRENT_REVISION_REF),
      cube_definition_key: buildCubeDefinitionKey(cubeId, nextVersion),
    });
  }

  private buildHistoricalLineage(
    browserEntry: CubeBrowserEntry | undefined,
    entry: SavePlanEntry,
  ): UnknownRecord {
    const source = browserEntry ?? {};
    return {
      id: entry.previousCubeId || source.cube_id || '',
      name: entry.defaultAlias || source.name || '',
      version: entry.sourceVersion || source.version || '',
      revision_ref: entry.sourceRevisionRef || '',
      author: source.author || '',
      author_url: source.author_url || '',
      forked_at: new Date().toISOString(),
    };
  }
}
