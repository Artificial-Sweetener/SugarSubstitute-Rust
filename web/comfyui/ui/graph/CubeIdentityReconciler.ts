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
 * Reconcile live graph identity references after a durable cube move.
 */

import { updateMarkersForCubeId } from './CubeMarkers.js';
import { getGraphGroups } from './GraphQuery.js';
import { getGroupSugarcubes, setGroupSugarcubes } from './GroupMetadata.js';
import type { ComfyGraph } from '../types/graph.js';

interface IdentityAdapter {
  getApp?(): { graph?: ComfyGraph } | null;
}

interface InstanceRefreshService {
  scheduleRefresh?(options: { graph: ComfyGraph; reason: string }): void;
}

interface DirtyRefreshService {
  requestRefresh?(options: { graph: ComfyGraph; reason: string }): void;
}

interface DefinitionInvalidator {
  invalidateCube?(cubeId: string): void;
}

interface CubeIdentityReconcilerOptions {
  adapter?: IdentityAdapter | null;
  instanceManager?: InstanceRefreshService | null;
  dirtyManager?: DirtyRefreshService | null;
  definitionStore?: DefinitionInvalidator | null;
}

interface CubeIdentityChange {
  previousCubeId?: string;
  cubeId?: string;
  defaultAlias?: string;
}

export interface CubeIdentityReconcileResult {
  markers: number;
  groups: number;
}

/** Own graph-local marker and managed-group identity retargeting. */
export class CubeIdentityReconciler {
  private readonly adapter: IdentityAdapter | null;
  private readonly instanceManager: InstanceRefreshService | null;
  private readonly dirtyManager: DirtyRefreshService | null;
  private readonly definitionStore: DefinitionInvalidator | null;

  constructor({
    adapter,
    instanceManager,
    dirtyManager,
    definitionStore,
  }: CubeIdentityReconcilerOptions = {}) {
    this.adapter = adapter ?? null;
    this.instanceManager = instanceManager ?? null;
    this.dirtyManager = dirtyManager ?? null;
    this.definitionStore = definitionStore ?? null;
  }

  /** Retarget all live references after the backend has committed the move. */
  reconcile({
    previousCubeId,
    cubeId,
    defaultAlias,
  }: CubeIdentityChange = {}): CubeIdentityReconcileResult {
    const graph = this.adapter?.getApp?.()?.graph;
    if (!graph || !previousCubeId || !cubeId) {
      return { markers: 0, groups: 0 };
    }
    const markers = updateMarkersForCubeId(graph, previousCubeId, {
      cubeId,
      ...(defaultAlias ? { defaultAlias } : {}),
    });
    let groups = 0;
    for (const group of getGraphGroups(graph)) {
      const metadata = getGroupSugarcubes(group);
      if (metadata?.cube_id !== previousCubeId) {
        continue;
      }
      setGroupSugarcubes(group, {
        ...metadata,
        cube_id: cubeId,
        ...(defaultAlias ? { default_alias: defaultAlias } : {}),
      });
      groups += 1;
    }
    this.definitionStore?.invalidateCube?.(previousCubeId);
    this.instanceManager?.scheduleRefresh?.({ graph, reason: 'cube-promoted' });
    this.dirtyManager?.requestRefresh?.({ graph, reason: 'cube-promoted' });
    graph.setDirtyCanvas?.(true, true);
    return { markers, groups };
  }
}
