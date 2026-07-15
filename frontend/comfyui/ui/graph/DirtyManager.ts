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
 * Own the SugarCubes graph integration layer in `frontend/comfyui/ui/graph/DirtyManager.js`.
 */

import { DirtyTracker } from './DirtyTracker.js';
import { BaselineStore } from './BaselineStore.js';
import { BaselineResolver } from './BaselineResolver.js';
import { getGraphGroups } from './GraphQuery.js';
import { getGroupSugarcubes } from './GroupMetadata.js';
import { DirtyRefreshScheduler } from './DirtyRefreshScheduler.js';
import { buildCubeDefinitionKey, normalizeRevisionRef } from '../core/CubeDefinitionKey.js';
import type { CubeDefinitionEntry, CubeDefinitionRequest } from './CubeDefinitionStore.js';
import type { DirtyRefreshOptions } from './DirtyRefreshScheduler.js';
import type { FinalizedDefinition } from '../save/CubeSaveReconciler.js';
import type { CubeGroupMetadataRecord } from './GroupMetadata.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyGraph } from '../types/graph.js';
import type { RefreshScheduler } from './DirtyRefreshScheduler.js';
import { isRecord } from '../types/common.js';

interface DirtyManagerAdapter {
  getConsole?(): { warn(...values: unknown[]): void } | null;
}

interface DirtyEventBus {
  emit?(event: string, payload: unknown): unknown;
  on?(event: string, handler: (payload: UnknownRecord) => void): (() => void) | void;
}

interface DirtyBrowser {
  getCubes?(): UnknownRecord[];
  setDirtyCubeIds?(ids: Set<string>): void;
}

interface DirtyDefinitionStore {
  ensure(request: CubeDefinitionRequest): CubeDefinitionEntry | null;
  getEntry(request: CubeDefinitionRequest): CubeDefinitionEntry | null;
}

interface DirtyManagerOptions {
  adapter: DirtyManagerAdapter;
  events?: DirtyEventBus | null;
  scheduler?: RefreshScheduler | null;
  cubeBrowser?: DirtyBrowser | null;
  definitionStore: DirtyDefinitionStore;
}

interface DirtyMarkOptions {
  graph?: ComfyGraph | null | undefined;
  cubeIds?: readonly string[] | null | undefined;
}

/**
 * Coordinate dirty manager behavior for the SugarCubes UI.
 */
export class DirtyManager {
  private readonly events: DirtyEventBus | null;
  private readonly scheduler: RefreshScheduler | null | undefined;
  private readonly cubeBrowser: DirtyBrowser | null;
  private readonly savedIds: Set<string>;
  private unsubscribe: (() => boolean | void) | null;
  private lastGraph: ComfyGraph | null;
  private readonly baselineStore: BaselineStore;
  private readonly baselineResolver: BaselineResolver;
  private readonly tracker: DirtyTracker;
  private readonly definitionStore: DirtyDefinitionStore;
  private readonly refreshScheduler: DirtyRefreshScheduler;

  constructor({
    adapter,
    events = null,
    scheduler,
    cubeBrowser = null,
    definitionStore,
  }: DirtyManagerOptions) {
    this.events = events;
    this.scheduler = scheduler;
    this.cubeBrowser = cubeBrowser;
    this.savedIds = new Set<string>();
    this.unsubscribe = null;
    this.lastGraph = null;
    this.baselineStore = new BaselineStore();
    this.baselineResolver = new BaselineResolver({ baselineStore: this.baselineStore });
    this.tracker = new DirtyTracker({
      logger: adapter.getConsole?.() ?? null,
      baselineStore: this.baselineStore,
      baselineResolver: this.baselineResolver,
    });
    this.definitionStore = definitionStore;
    this.refreshScheduler = new DirtyRefreshScheduler({
      scheduler: this.scheduler,
      onRefresh: (options) => this.refresh(options),
    });
  }

  setup(): void {
    if (this.unsubscribe) {
      return;
    }
    this.unsubscribe = this.tracker.onChange((dirtyIds) => {
      this.cubeBrowser?.setDirtyCubeIds?.(dirtyIds);
      this.events?.emit?.('cube:dirty:changed', { dirtyIds });
    });
    this.events?.on?.('cube:instances:updated', (payload) => {
      const graph = isRecord(payload.graph) ? (payload.graph as ComfyGraph) : null;
      this.requestRefresh({ graph, reason: 'instances-updated' });
    });
  }

  dispose(): void {
    if (this.unsubscribe) {
      this.unsubscribe();
      this.unsubscribe = null;
    }
  }

  addSavedIds(cubeIds: readonly string[]): void {
    for (const cubeId of cubeIds || []) {
      if (cubeId) {
        this.savedIds.add(cubeId);
      }
    }
  }

  updateKnownCubes(cubes: readonly UnknownRecord[] | null | undefined): void {
    const knownIds = new Set(
      (Array.isArray(cubes) ? cubes : [])
        .map((entry) => (typeof entry?.cube_id === 'string' ? entry.cube_id.trim() : ''))
        .filter(Boolean),
    );
    for (const cubeId of Array.from(this.savedIds)) {
      if (knownIds.has(cubeId)) {
        this.savedIds.delete(cubeId);
      }
    }
    if (this.lastGraph) {
      this.requestRefresh({ graph: this.lastGraph, reason: 'library-update' });
    }
  }

  buildKnownCubeIdSet(): Set<string> {
    const known = new Set<string>();
    const entries = Array.isArray(this.cubeBrowser?.getCubes?.())
      ? this.cubeBrowser.getCubes()
      : [];
    for (const entry of entries) {
      const cubeId = typeof entry?.cube_id === 'string' ? entry.cube_id.trim() : '';
      if (cubeId) {
        known.add(cubeId);
      }
    }
    for (const cubeId of this.savedIds) {
      known.add(cubeId);
    }
    return known;
  }

  getDirtyCubeIds(): Set<string> {
    return this.tracker.getDirtyCubeIds();
  }

  getImplementationDirtyCubeIds(): Set<string> {
    return this.tracker.getImplementationDirtyCubeIds();
  }

  getSaveableCubeIds(): Set<string> {
    return this.tracker.getSaveableCubeIds();
  }

  /** Return a diagnostic snapshot without exposing mutable tracker ownership. */
  getDebugState(instanceId: unknown): UnknownRecord | null {
    const key = typeof instanceId === 'string' ? instanceId.trim() : '';
    if (!key) return null;
    const entry = this.tracker.instances.get(key);
    if (!entry) return null;
    const definitionHash = this.baselineStore.getDefinitionHash(entry.cubeId);
    const localBaselineHash = this.baselineStore.getLocalBaselineHash(key);
    const baselineSource =
      localBaselineHash && entry.baselineHash === localBaselineHash
        ? 'local'
        : definitionHash && entry.baselineHash === definitionHash
          ? 'definition'
          : null;
    return {
      instanceId: key,
      cubeId: entry.cubeId || null,
      baselineHash: entry.baselineHash || null,
      currentHash: entry.currentHash || null,
      baselineSource,
      reasons: Array.isArray(entry.reasons) ? entry.reasons : [],
      dirty: Boolean(entry.dirty),
      dirtyAt: entry.dirtyAt || null,
      initializedAt: entry.initializedAt || null,
    };
  }

  requestRefresh({ graph, reason }: DirtyRefreshOptions = {}): void {
    this.refreshScheduler.requestRefresh({ graph, reason });
  }

  scheduleRefresh({ graph, reason }: DirtyRefreshOptions = {}): void {
    this.requestRefresh({ graph, reason });
  }

  refresh({ graph }: DirtyRefreshOptions = {}): { dirtyCubeIds: Set<string> } {
    if (graph) {
      this.lastGraph = graph;
    }
    if (!graph) {
      return { dirtyCubeIds: new Set() };
    }
    const groups = getGraphGroups(graph);
    if (!groups.length) {
      this.tracker.refresh({ graph, knownCubeIds: null });
      this.cubeBrowser?.setDirtyCubeIds?.(new Set());
      return { dirtyCubeIds: new Set() };
    }
    const hasSugarcubes = groups.some((group) => {
      const metadata = getGroupSugarcubes(group);
      return Boolean(metadata?.managed && metadata.cube_id);
    });
    if (!hasSugarcubes) {
      this.tracker.refresh({ graph, knownCubeIds: null });
      this.cubeBrowser?.setDirtyCubeIds?.(new Set());
      return { dirtyCubeIds: new Set() };
    }
    for (const group of groups) {
      const metadata = getGroupSugarcubes(group);
      const cubeId = typeof metadata?.cube_id === 'string' ? metadata.cube_id : '';
      if (!metadata || !cubeId) {
        continue;
      }
      const definitionRequest = buildDefinitionRequest(metadata);
      this.definitionStore.ensure(definitionRequest);
      const entry = this.definitionStore.getEntry(definitionRequest);
      this.baselineStore.setDefinition(definitionRequest.definitionKey, entry);
    }
    const knownCubeIds = this.buildKnownCubeIdSet();
    const result = this.tracker.refresh({
      graph,
      knownCubeIds: knownCubeIds.size ? knownCubeIds : null,
    });
    this.cubeBrowser?.setDirtyCubeIds?.(result?.dirtyCubeIds || new Set());
    return result;
  }

  markClean({ graph, cubeIds }: DirtyMarkOptions = {}): void {
    this.tracker.markClean({ graph, cubeIds });
  }

  /** Install authoritative persisted definition baselines after a save. */
  acceptFinalizedDefinitions({
    entries,
  }: {
    entries?: readonly FinalizedDefinition[];
  } = {}): void {
    for (const result of Array.isArray(entries) ? entries : []) {
      if (result?.definitionKey && result?.entry) {
        this.baselineStore.setDefinition(result.definitionKey, result.entry);
      }
    }
  }

  markLocalBaseline({ graph, cubeIds }: DirtyMarkOptions = {}): void {
    this.tracker.markLocalBaseline({ graph, cubeIds });
  }
}

function buildDefinitionRequest(metadata: CubeGroupMetadataRecord): CubeDefinitionRequest & {
  cubeId: string;
  cubeVersion: string;
  revisionRef: string;
  definitionKey: string;
} {
  const cubeId = typeof metadata?.cube_id === 'string' ? metadata.cube_id.trim() : '';
  const cubeVersion =
    typeof metadata?.cube_version === 'string' ? metadata.cube_version.trim() : '';
  const revisionRef = normalizeRevisionRef(metadata?.cube_revision_ref);
  const definitionKey =
    typeof metadata?.cube_definition_key === 'string' && metadata.cube_definition_key.trim()
      ? metadata.cube_definition_key.trim()
      : buildCubeDefinitionKey(cubeId, cubeVersion);
  return {
    cubeId,
    cubeVersion,
    revisionRef,
    definitionKey,
  };
}
