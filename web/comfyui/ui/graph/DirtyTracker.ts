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
 * Own the SugarCubes graph integration layer in `web/comfyui/ui/graph/DirtyTracker.js`.
 */

import { getGraphGroups, getGraphNodes } from './GraphQuery.js';
import { buildIdLookup, resolveInstanceAnchorFromNodes } from './DirtySnapshotter.js';
import {
  computeCosmeticHash,
  computeImplementationHash,
  computeSurfaceHash,
  computeSurfaceValuesHash,
} from './DirtyHasher.js';
import { DirtyStateService } from './DirtyStateService.js';
import { DirtyStateApplier } from './DirtyStateApplier.js';
import { getGroupSugarcubes } from './GroupMetadata.js';
import { buildCubeDefinitionKey } from '../core/CubeDefinitionKey.js';
import { isRecord } from '../types/common.js';
import type { BaselineRequest, BaselineResolution } from './BaselineResolver.js';
import type {
  ApplyDirtyInstanceStateOptions,
  TrackedDirtyInstanceState,
} from './DirtyStateApplier.js';
import type { DirtyStateRequest, DirtyStateResult } from './DirtyStateService.js';
import type { CubeGroupMetadataRecord } from './GroupMetadata.js';
import type { ComfyGraph, ComfyGroup, CubeSurface, GraphId } from '../types/graph.js';

interface DirtyLogger {
  warn(...values: unknown[]): void;
}

interface DirtyBaselineStore {
  getDefinitionHash(key: string | undefined): string | null | undefined;
  getDefinitionStatus(key: string | undefined): string | null | undefined;
  getLocalBaselineHash(instanceId: string | undefined): string | null | undefined;
  setLocalImplementationHash(instanceId: string, hash: unknown): void;
  getLocalImplementationHash(instanceId: string): string | null;
  setLocalCosmeticHash(instanceId: string, hash: unknown): void;
  getLocalCosmeticHash(instanceId: string): string | null;
  pruneLocalBaselines(activeInstanceIds: Set<string>): void;
}

interface DirtyBaselineResolver {
  resolve(request: BaselineRequest): BaselineResolution;
}

interface DirtyEvaluator {
  evaluate(request: DirtyStateRequest): DirtyStateResult;
}

interface DirtyStateWriter {
  applyInstanceState(options: ApplyDirtyInstanceStateOptions): void;
  finalize(): void;
}

interface DirtyTrackerOptions {
  logger?: DirtyLogger | null;
  baselineStore?: DirtyBaselineStore | null;
  baselineResolver: DirtyBaselineResolver;
  evaluator?: DirtyEvaluator | null;
  stateApplier?: DirtyStateWriter | null;
}

interface RefreshDirtyOptions {
  graph?: ComfyGraph | null;
  knownCubeIds?: ReadonlySet<string> | readonly string[] | null;
}

interface MarkDirtyOptions {
  graph?: ComfyGraph | null | undefined;
  cubeIds?: readonly string[] | null | undefined;
}

type DirtyChangeListener = (cubeIds: Set<string>) => void;

/**
 * Coordinate dirty tracker behavior for the SugarCubes UI.
 */
export class DirtyTracker {
  readonly instances: Map<string, TrackedDirtyInstanceState>;
  private dirtyCubeIds: Set<string>;
  private saveableCubeIds: Set<string>;
  private readonly listeners: Set<DirtyChangeListener>;
  private readonly logger: DirtyLogger | null;
  private readonly baselineStore: DirtyBaselineStore | null;
  private readonly baselineResolver: DirtyBaselineResolver;
  private readonly evaluator: DirtyEvaluator;
  private readonly stateApplier: DirtyStateWriter;
  private readonly warnedMissingSymbols: Set<string>;

  constructor({
    logger = null,
    baselineStore = null,
    baselineResolver,
    evaluator = null,
    stateApplier = null,
  }: DirtyTrackerOptions) {
    this.instances = new Map<string, TrackedDirtyInstanceState>();
    this.dirtyCubeIds = new Set<string>();
    this.saveableCubeIds = new Set<string>();
    this.listeners = new Set<DirtyChangeListener>();
    this.logger = logger;
    this.baselineStore = baselineStore;
    this.baselineResolver = baselineResolver;
    this.evaluator = evaluator || new DirtyStateService();
    this.stateApplier =
      stateApplier ||
      new DirtyStateApplier({
        tracker: this,
      });
    this.warnedMissingSymbols = new Set<string>();
  }

  onChange(listener: DirtyChangeListener): () => boolean | void {
    if (typeof listener !== 'function') {
      return () => {};
    }
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  getDirtyCubeIds(): Set<string> {
    return new Set(this.dirtyCubeIds);
  }

  getImplementationDirtyCubeIds(): Set<string> {
    return new Set(this.dirtyCubeIds);
  }

  getSaveableCubeIds(): Set<string> {
    return new Set(this.saveableCubeIds);
  }

  hasDefinitionBaseline(definitionKey: string): boolean {
    const hash = this.baselineStore?.getDefinitionHash(definitionKey) || null;
    if (!hash) {
      return false;
    }
    const status = this.baselineStore?.getDefinitionStatus(definitionKey) || null;
    return status == null || status === 'ready';
  }

  resolveGroupSnapshotForLocalHash(group: ComfyGroup, definitionKey: string): ComfyGroup | null {
    return this.hasDefinitionBaseline(definitionKey) ? null : group;
  }

  resolveSurfaceBaselineHash(metadata: CubeGroupMetadataRecord): string | null {
    const surface = isRecord(metadata.surface) ? (metadata.surface as CubeSurface) : null;
    const activeValues = isRecord(metadata.active_flavor_values)
      ? metadata.active_flavor_values
      : {};
    if (!Array.isArray(surface?.controls) || !surface.controls.length) {
      return null;
    }
    return computeSurfaceValuesHash(surface, activeValues);
  }

  refresh({ graph, knownCubeIds }: RefreshDirtyOptions = {}): { dirtyCubeIds: Set<string> } {
    if (!graph) {
      return { dirtyCubeIds: new Set() };
    }
    const now = Date.now();
    const hasKnown = knownCubeIds != null;
    const knownSet =
      knownCubeIds instanceof Set
        ? knownCubeIds
        : new Set(Array.isArray(knownCubeIds) ? knownCubeIds : []);
    const groups = getGraphGroups(graph);
    const activeInstanceIds = new Set<string>();

    for (const group of groups) {
      const metadata = getGroupSugarcubes(group);
      const instanceId = readMetadataString(metadata, 'instance_id');
      const cubeId = readMetadataString(metadata, 'cube_id');
      if (!metadata?.managed || !instanceId || !cubeId) {
        continue;
      }
      const nodeIds = readGraphIds(metadata.nodes);
      const markerIds = readMarkerIds(metadata.markers);
      const definitionKey = resolveDefinitionKey(metadata);
      const localGroup = this.resolveGroupSnapshotForLocalHash(group, definitionKey);
      const nodeById = new Map(
        getGraphNodes(graph)
          .filter((node) => node?.id != null)
          .map((node) => [String(node.id), node]),
      );
      const ids = new Set([...nodeIds.map(String), ...markerIds.map(String)]);
      const anchor = resolveInstanceAnchorFromNodes(nodeById, ids);
      const { lookup: idLookup, missing: missingSymbols } = buildIdLookup(nodeById, ids);
      const isKnown = !hasKnown || knownSet.has(cubeId);
      const resolution = this.baselineResolver.resolve({
        cubeId,
        definitionKey,
        instanceId,
        missingSymbols,
      });
      const useDefinition = resolution.useDefinition;
      const implementationCurrentHash = computeImplementationHash(
        graph,
        nodeIds,
        markerIds,
        anchor,
        readSurface(metadata.surface),
        useDefinition
          ? {
              idLookup,
              useSymbols: true,
              useInputNames: true,
              stripSugarcubesProperties: true,
            }
          : {
              useSymbols: false,
              useInputNames: false,
            },
      );
      const cosmeticCurrentHash = computeCosmeticHash(
        graph,
        nodeIds,
        markerIds,
        anchor,
        localGroup || group,
      );
      const surfaceCurrentHash = computeSurfaceHash(graph, nodeIds, readSurface(metadata.surface));
      const previous = this.instances.get(instanceId);
      let implementationBaselineHash = resolution.baselineHash;
      if (!implementationBaselineHash && !useDefinition) {
        this.baselineStore?.setLocalImplementationHash(instanceId, implementationCurrentHash);
        implementationBaselineHash = implementationCurrentHash;
      }
      let cosmeticBaselineHash = this.baselineStore?.getLocalCosmeticHash(instanceId) || null;
      if (!cosmeticBaselineHash) {
        this.baselineStore?.setLocalCosmeticHash(instanceId, cosmeticCurrentHash);
        cosmeticBaselineHash = cosmeticCurrentHash;
      }
      let surfaceBaselineHash = this.resolveSurfaceBaselineHash(metadata);
      if (!surfaceBaselineHash) {
        surfaceBaselineHash = surfaceCurrentHash;
      }
      if (
        !useDefinition &&
        !this.baselineStore?.getLocalImplementationHash(instanceId) &&
        metadata.dirty &&
        metadata.dirty_at
      ) {
        this.baselineStore?.setLocalImplementationHash(instanceId, implementationCurrentHash);
        implementationBaselineHash = implementationCurrentHash;
      }
      const evaluation = this.evaluator.evaluate({
        implementationCurrentHash,
        implementationBaselineHash,
        cosmeticCurrentHash,
        cosmeticBaselineHash,
        surfaceCurrentHash,
        surfaceBaselineHash,
        isKnown,
        missingSymbols: useDefinition ? missingSymbols : false,
        previousDirtyAt: previous?.dirtyAt || null,
      });
      const initializedAt = previous?.initializedAt ?? now;

      this.stateApplier.applyInstanceState({
        group,
        metadata,
        instanceId,
        cubeId,
        definitionKey,
        implementationBaselineHash,
        implementationCurrentHash,
        cosmeticBaselineHash,
        cosmeticCurrentHash,
        surfaceBaselineHash,
        surfaceCurrentHash,
        dirty: evaluation.implementationDirty,
        dirtyAt: evaluation.dirtyAt,
        implementationDirty: evaluation.implementationDirty,
        implementationReasons: evaluation.implementationReasons,
        cosmeticDirty: evaluation.cosmeticDirty,
        surfaceValuesChanged: evaluation.surfaceValuesChanged,
        hasSaveableChanges: evaluation.implementationDirty,
        initializedAt,
      });
      activeInstanceIds.add(instanceId);

      if (
        evaluation.implementationReasons.includes('missing-symbols') &&
        this.logger &&
        !this.warnedMissingSymbols.has(instanceId)
      ) {
        this.warnedMissingSymbols.add(instanceId);
        this.logger.warn('SugarCubes: symbol metadata missing; instance marked dirty', cubeId);
      }
    }

    for (const instanceId of Array.from(this.instances.keys())) {
      if (!activeInstanceIds.has(instanceId)) {
        this.instances.delete(instanceId);
        this.warnedMissingSymbols.delete(instanceId);
      }
    }
    this.baselineStore?.pruneLocalBaselines(activeInstanceIds);

    this.stateApplier.finalize();
    return { dirtyCubeIds: new Set(this.dirtyCubeIds) };
  }

  markClean({ graph, cubeIds }: MarkDirtyOptions = {}): void {
    if (!graph || !Array.isArray(cubeIds) || !cubeIds.length) {
      return;
    }
    const cubeIdSet = new Set(cubeIds);
    const groups = getGraphGroups(graph);
    for (const group of groups) {
      const metadata = getGroupSugarcubes(group);
      const instanceId = readMetadataString(metadata, 'instance_id');
      const cubeId = readMetadataString(metadata, 'cube_id');
      if (!metadata?.managed || !instanceId || !cubeId) {
        continue;
      }
      if (!cubeIdSet.has(cubeId)) {
        continue;
      }
      const nodeIds = readGraphIds(metadata.nodes);
      const markerIds = readMarkerIds(metadata.markers);
      const ids = new Set([...nodeIds.map(String), ...markerIds.map(String)]);
      const nodeById = new Map(
        getGraphNodes(graph)
          .filter((node) => node?.id != null)
          .map((node) => [String(node.id), node]),
      );
      const anchor = resolveInstanceAnchorFromNodes(nodeById, ids);
      const implementationCurrentHash = computeImplementationHash(
        graph,
        nodeIds,
        markerIds,
        anchor,
        readSurface(metadata.surface),
        {
          useSymbols: false,
          useInputNames: false,
        },
      );
      const cosmeticCurrentHash = computeCosmeticHash(graph, nodeIds, markerIds, anchor, group);
      this.baselineStore?.setLocalImplementationHash(instanceId, implementationCurrentHash);
      this.baselineStore?.setLocalCosmeticHash(instanceId, cosmeticCurrentHash);
      this.stateApplier.applyInstanceState({
        group,
        metadata,
        instanceId,
        cubeId,
        implementationBaselineHash: implementationCurrentHash,
        implementationCurrentHash,
        cosmeticBaselineHash: cosmeticCurrentHash,
        cosmeticCurrentHash,
        surfaceBaselineHash: this.resolveSurfaceBaselineHash(metadata),
        surfaceCurrentHash: computeSurfaceHash(graph, nodeIds, readSurface(metadata.surface)),
        dirty: false,
        dirtyAt: null,
        implementationDirty: false,
        implementationReasons: [],
        cosmeticDirty: false,
        surfaceValuesChanged: false,
        hasSaveableChanges: false,
        initializedAt: Date.now(),
      });
    }
    this.stateApplier.finalize();
  }

  markLocalBaseline({ graph, cubeIds }: MarkDirtyOptions = {}): void {
    if (!graph || !Array.isArray(cubeIds) || !cubeIds.length) {
      return;
    }
    const cubeIdSet = new Set(cubeIds);
    const groups = getGraphGroups(graph);
    for (const group of groups) {
      const metadata = getGroupSugarcubes(group);
      const instanceId = readMetadataString(metadata, 'instance_id');
      const cubeId = readMetadataString(metadata, 'cube_id');
      if (!metadata?.managed || !instanceId || !cubeId) {
        continue;
      }
      if (!cubeIdSet.has(cubeId)) {
        continue;
      }
      const nodeIds = readGraphIds(metadata.nodes);
      const markerIds = readMarkerIds(metadata.markers);
      const ids = new Set([...nodeIds.map(String), ...markerIds.map(String)]);
      const nodeById = new Map(
        getGraphNodes(graph)
          .filter((node) => node?.id != null)
          .map((node) => [String(node.id), node]),
      );
      const anchor = resolveInstanceAnchorFromNodes(nodeById, ids);
      const implementationCurrentHash = computeImplementationHash(
        graph,
        nodeIds,
        markerIds,
        anchor,
        readSurface(metadata.surface),
        {
          useSymbols: false,
          useInputNames: false,
        },
      );
      const cosmeticCurrentHash = computeCosmeticHash(graph, nodeIds, markerIds, anchor, group);
      this.baselineStore?.setLocalImplementationHash(instanceId, implementationCurrentHash);
      this.baselineStore?.setLocalCosmeticHash(instanceId, cosmeticCurrentHash);
      const existing = this.instances.get(instanceId);
      if (existing) {
        this.instances.set(instanceId, {
          ...existing,
          baselineHash: implementationCurrentHash,
          currentHash: implementationCurrentHash,
          implementationBaselineHash: implementationCurrentHash,
          implementationCurrentHash,
          cosmeticBaselineHash: cosmeticCurrentHash,
          cosmeticCurrentHash,
          surfaceBaselineHash: this.resolveSurfaceBaselineHash(metadata),
          surfaceCurrentHash: computeSurfaceHash(graph, nodeIds, readSurface(metadata.surface)),
        });
      }
    }
  }

  updateDirtyCubeIds(): void {
    const nextDirty = new Set<string>();
    const nextSaveable = new Set<string>();
    for (const entry of this.instances.values()) {
      if (entry?.hasSaveableChanges && entry.cubeId) {
        nextSaveable.add(entry.cubeId);
      }
      if (entry?.implementationDirty && entry.cubeId) {
        nextDirty.add(entry.cubeId);
      }
    }
    this.saveableCubeIds = nextSaveable;
    const previous = this.dirtyCubeIds;
    let changed = nextDirty.size !== previous.size;
    if (!changed) {
      for (const value of nextDirty) {
        if (!previous.has(value)) {
          changed = true;
          break;
        }
      }
    }
    if (changed) {
      this.dirtyCubeIds = nextDirty;
      for (const listener of this.listeners) {
        try {
          listener(new Set(this.dirtyCubeIds));
        } catch (_error) {
          // ignore listener errors
        }
      }
    }
  }
}

function resolveDefinitionKey(metadata: CubeGroupMetadataRecord): string {
  const existing =
    typeof metadata?.cube_definition_key === 'string' ? metadata.cube_definition_key.trim() : '';
  if (existing) {
    return existing;
  }
  return buildCubeDefinitionKey(metadata?.cube_id, metadata?.cube_version);
}

function readMetadataString(metadata: CubeGroupMetadataRecord | null, key: string): string {
  const value = metadata?.[key];
  return typeof value === 'string' ? value.trim() : '';
}

function readGraphIds(value: unknown): GraphId[] {
  return Array.isArray(value)
    ? value.filter(
        (entry): entry is GraphId => typeof entry === 'string' || typeof entry === 'number',
      )
    : [];
}

function readMarkerIds(value: unknown): GraphId[] {
  if (Array.isArray(value)) {
    return readGraphIds(value);
  }
  return isRecord(value) ? Object.values(value).flatMap(readGraphIds) : [];
}

function readSurface(value: unknown): CubeSurface | null {
  return isRecord(value) ? (value as CubeSurface) : null;
}
