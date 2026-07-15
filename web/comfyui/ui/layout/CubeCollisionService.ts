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
 * Own the SugarCubes layout orchestration layer in `web/comfyui/ui/layout/CubeCollisionService.js`.
 */

import { CubeInstanceIndex } from './CubeInstanceIndex.js';
import { moveInstanceByDelta } from './CubeMover.js';
import type { RectBounds } from '../types/common.js';
import type { ComfyGraph } from '../types/graph.js';
import type { MovableCubeInstance } from './CubeMover.js';

const DEFAULT_GAP = 2;
const DEFAULT_MAX_ITERATIONS = 3;
const DEFAULT_BUCKET_THRESHOLD = 100;
const DEFAULT_BUCKET_SIZE = 800;

interface MovementDelta {
  dx: number;
  dy: number;
}

export interface CollisionInstance extends MovableCubeInstance {
  instanceId: string;
  bounds: RectBounds | null;
}

export interface CollisionIndex {
  instances: CollisionInstance[];
  instanceById?: Map<string, CollisionInstance>;
}
type IndexFactory = (graph: ComfyGraph | null | undefined) => CollisionIndex;

interface CollisionOptions {
  indexFactory?: IndexFactory | null;
  gap?: number;
  maxIterations?: number;
  bucketThreshold?: number;
  bucketSize?: number;
}

interface CollisionRequest {
  graph?: ComfyGraph | null;
  activeInstanceId?: string | null;
  activeInstance?: CollisionInstance | null;
  index?: CollisionIndex | null;
}

function boundsOverlap(a: RectBounds, b: RectBounds): boolean {
  return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
}

function computeMTV(active: RectBounds, other: RectBounds, gap: number): MovementDelta | null {
  if (!boundsOverlap(active, other)) {
    return null;
  }
  const overlapX = Math.min(active.x + active.w - other.x, other.x + other.w - active.x);
  const overlapY = Math.min(active.y + active.h - other.y, other.y + other.h - active.y);
  if (!Number.isFinite(overlapX) || !Number.isFinite(overlapY)) {
    return null;
  }
  if (overlapX <= 0 || overlapY <= 0) {
    return null;
  }
  const centerAx = active.x + active.w / 2;
  const centerAy = active.y + active.h / 2;
  const centerBx = other.x + other.w / 2;
  const centerBy = other.y + other.h / 2;
  if (overlapX < overlapY) {
    const direction = centerAx < centerBx ? -1 : 1;
    return { dx: direction * (overlapX + gap), dy: 0 };
  }
  const direction = centerAy < centerBy ? -1 : 1;
  return { dx: 0, dy: direction * (overlapY + gap) };
}

function buildBucketKey(x: number, y: number): string {
  return `${x},${y}`;
}

function bucketizeInstances(
  instances: readonly CollisionInstance[],
  cellSize: number,
): Map<string, CollisionInstance[]> {
  const buckets = new Map<string, CollisionInstance[]>();
  for (const entry of instances) {
    const bounds = entry?.bounds;
    if (!bounds) {
      continue;
    }
    const minX = Math.floor(bounds.x / cellSize);
    const maxX = Math.floor((bounds.x + bounds.w) / cellSize);
    const minY = Math.floor(bounds.y / cellSize);
    const maxY = Math.floor((bounds.y + bounds.h) / cellSize);
    for (let gx = minX; gx <= maxX; gx += 1) {
      for (let gy = minY; gy <= maxY; gy += 1) {
        const key = buildBucketKey(gx, gy);
        const bucket = buckets.get(key) || [];
        bucket.push(entry);
        buckets.set(key, bucket);
      }
    }
  }
  return buckets;
}

function collectBucketNeighbors(
  bounds: RectBounds,
  buckets: ReadonlyMap<string, readonly CollisionInstance[]>,
  cellSize: number,
): CollisionInstance[] {
  const neighbors = new Set<CollisionInstance>();
  const minX = Math.floor(bounds.x / cellSize);
  const maxX = Math.floor((bounds.x + bounds.w) / cellSize);
  const minY = Math.floor(bounds.y / cellSize);
  const maxY = Math.floor((bounds.y + bounds.h) / cellSize);
  for (let gx = minX - 1; gx <= maxX + 1; gx += 1) {
    for (let gy = minY - 1; gy <= maxY + 1; gy += 1) {
      const key = buildBucketKey(gx, gy);
      const bucket = buckets.get(key);
      if (!bucket) {
        continue;
      }
      for (const entry of bucket) {
        neighbors.add(entry);
      }
    }
  }
  return Array.from(neighbors);
}

/**
 * Coordinate cube collision service behavior for the SugarCubes UI.
 */
export class CubeCollisionService {
  private readonly indexFactory: IndexFactory | null;
  private readonly gap: number;
  private readonly maxIterations: number;
  private readonly bucketThreshold: number;
  private readonly bucketSize: number;

  constructor({
    indexFactory,
    gap = DEFAULT_GAP,
    maxIterations = DEFAULT_MAX_ITERATIONS,
    bucketThreshold = DEFAULT_BUCKET_THRESHOLD,
    bucketSize = DEFAULT_BUCKET_SIZE,
  }: CollisionOptions = {}) {
    this.indexFactory = typeof indexFactory === 'function' ? indexFactory : null;
    this.gap = Number.isFinite(Number(gap)) ? Number(gap) : DEFAULT_GAP;
    this.maxIterations = Number.isFinite(Number(maxIterations))
      ? Number(maxIterations)
      : DEFAULT_MAX_ITERATIONS;
    this.bucketThreshold = Number.isFinite(Number(bucketThreshold))
      ? Number(bucketThreshold)
      : DEFAULT_BUCKET_THRESHOLD;
    this.bucketSize = Number.isFinite(Number(bucketSize))
      ? Number(bucketSize)
      : DEFAULT_BUCKET_SIZE;
  }

  buildIndex(graph: ComfyGraph | null | undefined): CollisionIndex {
    if (this.indexFactory) {
      return this.indexFactory(graph);
    }
    return new CubeInstanceIndex(graph === undefined ? {} : { graph });
  }

  resolveCollisions({ graph, activeInstanceId, activeInstance, index }: CollisionRequest = {}): {
    moved: boolean;
    iterations: number;
  } {
    const indexRef = index || this.buildIndex(graph);
    const instances = Array.isArray(indexRef?.instances) ? indexRef.instances : [];
    if (!instances.length) {
      return { moved: false, iterations: 0 };
    }
    const active =
      activeInstance ||
      (activeInstanceId
        ? indexRef?.instanceById?.get?.(String(activeInstanceId)) ||
          instances.find((entry) => entry.instanceId === String(activeInstanceId)) ||
          null
        : null);
    if (!active || !active.bounds) {
      return { moved: false, iterations: 0 };
    }

    const buckets =
      instances.length >= this.bucketThreshold
        ? bucketizeInstances(instances, this.bucketSize)
        : null;

    let iterations = 0;
    let moved = false;

    while (iterations < this.maxIterations) {
      const activeBounds = active.bounds;
      const candidates = buckets
        ? collectBucketNeighbors(activeBounds, buckets, this.bucketSize)
        : instances;

      let bestMove: MovementDelta | null = null;
      let bestMagnitude: number | null = null;

      for (const entry of candidates) {
        if (!entry || entry.instanceId === active.instanceId) {
          continue;
        }
        const bounds = entry.bounds;
        if (!bounds) {
          continue;
        }
        const mtv = computeMTV(activeBounds, bounds, this.gap);
        if (!mtv) {
          continue;
        }
        const magnitude = Math.abs(mtv.dx) + Math.abs(mtv.dy);
        if (bestMagnitude == null || magnitude < bestMagnitude) {
          bestMagnitude = magnitude;
          bestMove = mtv;
        }
      }

      if (!bestMove) {
        break;
      }

      moveInstanceByDelta(active, bestMove, { recomputeBounds: false });
      moved = true;
      iterations += 1;
    }

    return { moved, iterations };
  }
}
