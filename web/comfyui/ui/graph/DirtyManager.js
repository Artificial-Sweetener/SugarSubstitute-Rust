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
 * Own the SugarCubes graph integration layer in `web/comfyui/ui/graph/DirtyManager.js`.
 */

import { DirtyTracker } from './DirtyTracker.js';
import { BaselineStore } from './BaselineStore.js';
import { BaselineResolver } from './BaselineResolver.js';
import { getGraphGroups } from './GraphQuery.js';
import { getGroupSugarcubes } from './GroupMetadata.js';
import { DirtyRefreshScheduler } from './DirtyRefreshScheduler.js';
import { buildCubeDefinitionKey, normalizeRevisionRef } from '../core/CubeDefinitionKey.js';

/**
 * Coordinate dirty manager behavior for the SugarCubes UI.
 */
export class DirtyManager {
  constructor({ adapter, events, scheduler, cubeBrowser, definitionStore } = {}) {
    this.adapter = adapter;
    this.events = events;
    this.scheduler = scheduler;
    this.cubeBrowser = cubeBrowser;
    this.savedIds = new Set();
    this.unsubscribe = null;
    this.lastGraph = null;
    this.baselineStore = new BaselineStore();
    this.baselineResolver = new BaselineResolver({ baselineStore: this.baselineStore });
    this.tracker = new DirtyTracker({
      logger: adapter?.getConsole?.(),
      baselineStore: this.baselineStore,
      baselineResolver: this.baselineResolver,
    });
    this.definitionStore = definitionStore;
    this.refreshScheduler = new DirtyRefreshScheduler({
      scheduler: this.scheduler,
      onRefresh: (options) => this.refresh(options),
    });
  }

  setup() {
    if (this.unsubscribe) {
      return;
    }
    this.unsubscribe = this.tracker.onChange((dirtyIds) => {
      this.cubeBrowser?.setDirtyCubeIds?.(dirtyIds);
      this.events?.emit?.('cube:dirty:changed', { dirtyIds });
    });
    this.events?.on?.('cube:instances:updated', (payload) => {
      const graph = payload?.graph;
      this.requestRefresh({ graph, reason: 'instances-updated' });
    });
  }

  dispose() {
    if (this.unsubscribe) {
      this.unsubscribe();
      this.unsubscribe = null;
    }
  }

  addSavedIds(cubeIds) {
    for (const cubeId of cubeIds || []) {
      if (cubeId) {
        this.savedIds.add(cubeId);
      }
    }
  }

  updateKnownCubes(cubes) {
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

  buildKnownCubeIdSet() {
    const known = new Set();
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

  getDirtyCubeIds() {
    return this.tracker.getDirtyCubeIds();
  }

  getImplementationDirtyCubeIds() {
    return this.tracker.getImplementationDirtyCubeIds();
  }

  getSaveableCubeIds() {
    return this.tracker.getSaveableCubeIds();
  }

  requestRefresh({ graph, reason } = {}) {
    this.refreshScheduler.requestRefresh({ graph, reason });
  }

  scheduleRefresh({ graph, reason } = {}) {
    this.requestRefresh({ graph, reason });
  }

  refresh({ graph } = {}) {
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
      if (!cubeId) {
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

  markClean({ graph, cubeIds } = {}) {
    this.tracker.markClean({ graph, cubeIds });
  }

  /** Install authoritative persisted definition baselines after a save. */
  acceptFinalizedDefinitions({ entries } = {}) {
    for (const result of Array.isArray(entries) ? entries : []) {
      if (result?.definitionKey && result?.entry) {
        this.baselineStore.setDefinition(result.definitionKey, result.entry);
      }
    }
  }

  markLocalBaseline({ graph, cubeIds } = {}) {
    this.tracker.markLocalBaseline({ graph, cubeIds });
  }
}

function buildDefinitionRequest(metadata) {
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
