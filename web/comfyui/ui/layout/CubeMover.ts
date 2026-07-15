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
 * Own the SugarCubes layout orchestration layer in `web/comfyui/ui/layout/CubeMover.js`.
 */

import { readGroupBounds } from '../graph/Bounds.js';
import {
  computeInstanceBounds,
  inflateInstanceBounds,
  resolveCanonicalPadding,
  writeCanonicalBounds,
} from '../graph/CubeBounds.js';
import { isRecord } from '../types/common.js';
import type { Vec2 } from '../types/common.js';
import type { ComfyGraph, ComfyGroup, ComfyNode } from '../types/graph.js';
import type { CubeGroupMetadataRecord } from '../graph/GroupMetadata.js';

export interface MoveDelta {
  dx: number;
  dy: number;
}

interface MovableBounds {
  x: number;
  y: number;
  w: number;
  h: number;
  padding?: unknown;
  header?: unknown;
}

export interface MovableCubeInstance {
  instanceId: string;
  nodes?: ComfyNode[];
  markers?: ComfyNode[];
  group?: ComfyGroup | null;
  metadata?: CubeGroupMetadataRecord | null;
  bounds?: MovableBounds | null;
}

interface CubeMoveOptions {
  recomputeBounds?: boolean;
}

interface MovableInstanceIndex {
  instances?: MovableCubeInstance[];
}

function readNumber(value: unknown, fallback = 0): number {
  const num = Number(value);
  return Number.isFinite(num) ? num : fallback;
}

function moveNode(node: ComfyNode | null | undefined, delta: Vec2): void {
  if (!node || !node.pos || typeof node.pos[0] === 'undefined') {
    return;
  }
  node.pos[0] = readNumber(node.pos[0], 0) + delta[0];
  node.pos[1] = readNumber(node.pos[1], 0) + delta[1];
}

function moveGroup(group: ComfyGroup | null | undefined, delta: Vec2): void {
  if (!group || !group.pos) {
    return;
  }
  group.pos[0] = readNumber(group.pos[0], 0) + delta[0];
  group.pos[1] = readNumber(group.pos[1], 0) + delta[1];
}

/**
 * Move instance by delta.
 */
export function moveInstanceByDelta(
  instance: MovableCubeInstance | null | undefined,
  delta: MoveDelta | null | undefined,
  options: CubeMoveOptions = {},
): void {
  if (!instance || !delta) {
    return;
  }
  const dx = readNumber(delta.dx, 0);
  const dy = readNumber(delta.dy, 0);
  if (!dx && !dy) {
    return;
  }
  const deltaVec: Vec2 = [dx, dy];
  const nodes = Array.isArray(instance.nodes) ? instance.nodes : [];
  const markers = Array.isArray(instance.markers) ? instance.markers : [];
  nodes.forEach((node) => moveNode(node, deltaVec));
  markers.forEach((marker) => moveNode(marker, deltaVec));
  moveGroup(instance.group, deltaVec);

  const shouldRecompute = options.recomputeBounds !== false;
  if (!shouldRecompute) {
    const metadataBounds = instance.metadata?.bounds;
    const baseBounds = isRecord(metadataBounds) ? metadataBounds : instance.bounds;
    if (baseBounds && isRecord(baseBounds)) {
      const shifted = {
        x: readNumber(baseBounds.x, 0) + dx,
        y: readNumber(baseBounds.y, 0) + dy,
        w: readNumber(baseBounds.w, 0),
        h: readNumber(baseBounds.h, 0),
        ...resolveCanonicalPadding(instance.metadata, baseBounds),
      };
      instance.bounds = { ...shifted };
      writeCanonicalBounds({ group: instance.group, metadata: instance.metadata, bounds: shifted });
    }
    return;
  }

  const computed = computeInstanceBounds(nodes, markers);
  if (computed) {
    const padding = resolveCanonicalPadding(instance.metadata, undefined);
    const inflated = inflateInstanceBounds(computed, {
      ...padding.padding,
      header: { ...padding.header },
    });
    if (inflated) {
      instance.bounds = { ...inflated };
      writeCanonicalBounds({
        group: instance.group,
        metadata: instance.metadata,
        bounds: inflated,
      });
    }
    return;
  }

  const groupBounds = readGroupBounds(instance.group);
  if (groupBounds) {
    const bounds = {
      x: groupBounds[0],
      y: groupBounds[1],
      w: groupBounds[2],
      h: groupBounds[3],
    };
    instance.bounds = bounds;
    writeCanonicalBounds({ group: instance.group, metadata: instance.metadata, bounds });
  }
}

/**
 * Apply moves.
 */
export function applyMoves(
  _graph: ComfyGraph | null | undefined,
  index: MovableInstanceIndex | null | undefined,
  moves: ReadonlyMap<string, MoveDelta> | null | undefined,
  options: CubeMoveOptions = {},
): void {
  const instances = Array.isArray(index?.instances) ? index.instances : [];
  if (!instances.length || !(moves instanceof Map)) {
    return;
  }
  for (const instance of instances) {
    const delta = moves.get(instance.instanceId);
    if (!delta) {
      continue;
    }
    moveInstanceByDelta(instance, delta, options);
  }
}
