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
 * Own the SugarCubes layout orchestration layer in `frontend/comfyui/ui/layout/CubeBoundsReconciler.js`.
 */

import {
  normalizeBoundsPayload,
  resolveChromeBoundsFromContent,
  resolveInstanceBounds,
  resolveCanonicalPadding,
  writeCanonicalBounds,
} from '../graph/CubeBounds.js';
import { readGroupBounds } from '../graph/Bounds.js';
import { CubeInstanceIndex } from './CubeInstanceIndex.js';
import type { CubeBoundsPayload } from '../graph/CubeBounds.js';
import type { CubeGroupMetadataRecord } from '../graph/GroupMetadata.js';
import type { RectBounds } from '../types/common.js';
import type { Bounds } from '../types/common.js';
import type { ComfyGraph, ComfyGroup, ComfyNode } from '../types/graph.js';

const EPSILON = 0.001;

type BoundsLike = RectBounds | CubeBoundsPayload;
export interface BoundsReconciliationInstance {
  instanceId: string;
  group: ComfyGroup | null;
  metadata: CubeGroupMetadataRecord | null;
  bounds: RectBounds | null;
  nodes: ComfyNode[];
  markers: ComfyNode[];
}

export interface BoundsReconciliationIndex {
  instances: BoundsReconciliationInstance[];
}

type IndexFactory = (graph: ComfyGraph | null | undefined) => BoundsReconciliationIndex;

interface ReconcileBoundsOptions {
  graph?: ComfyGraph | null;
  index?: BoundsReconciliationIndex | null;
}

function nearlyEqual(a: number, b: number, epsilon = EPSILON): boolean {
  return Math.abs(Number(a) - Number(b)) <= epsilon;
}

function payloadEqual(a: BoundsLike | null | undefined, b: unknown): boolean {
  if (!a || !b) {
    return false;
  }
  const candidate = b as Partial<CubeBoundsPayload>;
  const source = a as Partial<CubeBoundsPayload>;
  return (
    a.x === candidate.x &&
    a.y === candidate.y &&
    a.w === candidate.w &&
    a.h === candidate.h &&
    (source.padding?.x ?? null) === (candidate.padding?.x ?? null) &&
    (source.padding?.y ?? null) === (candidate.padding?.y ?? null) &&
    (source.padding?.top_extra ?? null) === (candidate.padding?.top_extra ?? null) &&
    (source.header?.height ?? null) === (candidate.header?.height ?? null)
  );
}

function boundsEqual(
  a: BoundsLike | null | undefined,
  b: BoundsLike | null | undefined,
  epsilon = EPSILON,
): boolean {
  if (!a || !b) {
    return false;
  }
  return (
    nearlyEqual(a.x, b.x, epsilon) &&
    nearlyEqual(a.y, b.y, epsilon) &&
    nearlyEqual(a.w, b.w, epsilon) &&
    nearlyEqual(a.h, b.h, epsilon)
  );
}

function groupMatchesBounds(
  group: ComfyGroup | null | undefined,
  bounds: BoundsLike | null | undefined,
): boolean {
  if (!bounds) {
    return false;
  }
  const pos = group?.pos;
  const size = group?.size;
  const hasPos = (Array.isArray(pos) || ArrayBuffer.isView(pos)) && (pos?.length || 0) >= 2;
  const hasSize = (Array.isArray(size) || ArrayBuffer.isView(size)) && (size?.length || 0) >= 2;
  if (hasPos && hasSize) {
    return (
      pos[0] === bounds.x && pos[1] === bounds.y && size[0] === bounds.w && size[1] === bounds.h
    );
  }
  const groupBounds = readGroupBounds(group);
  if (!groupBounds) {
    return false;
  }
  return (
    groupBounds[0] === bounds.x &&
    groupBounds[1] === bounds.y &&
    groupBounds[2] === bounds.w &&
    groupBounds[3] === bounds.h
  );
}

function normalizeGroupBounds(
  groupBounds: Bounds | null | undefined,
  canonical: CubeBoundsPayload | null | undefined,
): CubeBoundsPayload | null {
  if (!groupBounds || !canonical) {
    return null;
  }
  return normalizeBoundsPayload(
    {
      x: groupBounds[0],
      y: groupBounds[1],
      w: groupBounds[2],
      h: groupBounds[3],
      padding: canonical.padding,
      header: canonical.header,
    },
    { bounds: canonical },
  );
}

function deriveContentChromeBounds(
  instance: BoundsReconciliationInstance,
  canonical: unknown,
): CubeBoundsPayload | null {
  const nodes = Array.isArray(instance?.nodes) ? instance.nodes : [];
  const markers = Array.isArray(instance?.markers) ? instance.markers : [];
  if (!nodes.length && !markers.length) {
    return null;
  }
  const canonicalPadding = resolveCanonicalPadding(instance?.metadata, canonical);
  return resolveChromeBoundsFromContent({
    nodes,
    markers,
    padding: canonicalPadding.padding,
    header: canonicalPadding.header,
  });
}

function resolveGroupMoveCanonical(
  instance: BoundsReconciliationInstance,
  canonical: CubeBoundsPayload,
): CubeBoundsPayload | null {
  const groupBounds = readGroupBounds(instance?.group);
  if (!groupBounds || !canonical) {
    return null;
  }
  const movedGroup = normalizeGroupBounds(groupBounds, canonical);
  if (!movedGroup || boundsEqual(movedGroup, canonical)) {
    return null;
  }
  if (!nearlyEqual(movedGroup.w, canonical.w) || !nearlyEqual(movedGroup.h, canonical.h)) {
    return null;
  }
  const contentChrome = deriveContentChromeBounds(instance, canonical);
  if (!contentChrome || !boundsEqual(contentChrome, movedGroup)) {
    return null;
  }
  return movedGroup;
}

/**
 * Coordinate cube bounds reconciler behavior for the SugarCubes UI.
 */
export class CubeBoundsReconciler {
  private readonly indexFactory: IndexFactory | null;

  constructor({ indexFactory }: { indexFactory?: IndexFactory | null } = {}) {
    this.indexFactory = indexFactory ?? null;
  }

  buildIndex(graph: ComfyGraph | null | undefined): BoundsReconciliationIndex {
    if (this.indexFactory) {
      return this.indexFactory(graph);
    }
    return new CubeInstanceIndex(graph === undefined ? {} : { graph });
  }

  reconcileAll({ graph, index }: ReconcileBoundsOptions = {}): {
    changed: Set<string>;
    index: BoundsReconciliationIndex;
  } {
    const indexRef = index || this.buildIndex(graph);
    const instances = Array.isArray(indexRef?.instances) ? indexRef.instances : [];
    const changed = new Set<string>();
    for (const instance of instances) {
      const metadataBounds = instance.metadata?.bounds;
      const canonicalSource = isBoundsLike(metadataBounds) ? metadataBounds : instance.bounds;
      const canonicalRaw =
        canonicalSource && instance.metadata
          ? normalizeBoundsPayload(canonicalSource, instance.metadata)
          : deriveContentChromeBounds(instance, instance.metadata || null) ||
            resolveInstanceBounds({
              group: instance.group,
              metadata: instance.metadata,
              nodes: instance.nodes,
              markers: instance.markers,
            });
      if (!canonicalRaw) {
        continue;
      }
      const canonical = normalizeBoundsPayload(
        canonicalRaw,
        instance.metadata || { bounds: canonicalRaw },
      );
      const resolvedCanonical = resolveGroupMoveCanonical(instance, canonical) || canonical;
      const current = instance.metadata?.bounds || instance.bounds || null;
      const needsCanonicalWrite = !payloadEqual(resolvedCanonical, current);
      const needsGroupSync = !groupMatchesBounds(instance.group, resolvedCanonical);
      if (needsCanonicalWrite || needsGroupSync) {
        writeCanonicalBounds({
          group: instance.group,
          metadata: instance.metadata,
          bounds: resolvedCanonical,
        });
        instance.bounds = { ...resolvedCanonical };
        changed.add(instance.instanceId);
      } else if (!payloadEqual(instance.bounds, resolvedCanonical)) {
        instance.bounds = { ...resolvedCanonical };
      }
    }
    return { changed, index: indexRef };
  }
}

function isBoundsLike(value: unknown): value is RectBounds {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const bounds = value as Partial<RectBounds>;
  return [bounds.x, bounds.y, bounds.w, bounds.h].every((entry) => Number.isFinite(Number(entry)));
}
