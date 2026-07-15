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
import { describe, expect, test } from '@jest/globals';
import { CubeCollisionService } from '../../frontend/comfyui/ui/layout/CubeCollisionService.js';
import type { RectBounds } from '../../frontend/comfyui/ui/types/common.js';
import type { IndexedCubeInstance } from '../../frontend/comfyui/ui/layout/CubeInstanceIndex.js';
import type { ComfyGroup } from '../../frontend/comfyui/ui/types/graph.js';

type TestIndexedCubeInstance = IndexedCubeInstance & {
  bounds: RectBounds;
  group: ComfyGroup & { pos: number[]; size: number[] };
};

function buildInstance({
  id,
  bounds,
}: {
  id: string;
  bounds: RectBounds;
}): TestIndexedCubeInstance {
  const metadata = {
    instance_id: id,
    bounds: { ...bounds, padding: { x: 4, y: 4, top_extra: 0 }, header: { height: 12 } },
  };
  const group = {
    pos: [bounds.x, bounds.y],
    size: [bounds.w, bounds.h],
    properties: { sugarcubes: metadata },
  };
  return {
    instanceId: id,
    bounds: metadata.bounds,
    metadata,
    group,
    nodes: [],
    markers: [],
  } as unknown as TestIndexedCubeInstance;
}

describe('CubeCollisionService', () => {
  test('uses tighter default gap for overlap resolution', () => {
    const active = buildInstance({ id: 'A', bounds: { x: 0, y: 0, w: 50, h: 50 } });
    const other = buildInstance({ id: 'B', bounds: { x: 40, y: 0, w: 50, h: 50 } });
    const index = {
      instances: [active, other],
      instanceById: new Map([
        ['A', active],
        ['B', other],
      ]),
    };
    const service = new CubeCollisionService({ maxIterations: 1, bucketThreshold: 1000 });
    const result = service.resolveCollisions({ activeInstanceId: 'A', index });

    expect(result.moved).toBe(true);
    expect(active.bounds.x).toBe(-12);
    expect(active.bounds.y).toBe(0);
  });

  test('resolves overlap with gap', () => {
    const active = buildInstance({ id: 'A', bounds: { x: 0, y: 0, w: 50, h: 50 } });
    const other = buildInstance({ id: 'B', bounds: { x: 40, y: 0, w: 50, h: 50 } });
    const index = {
      instances: [active, other],
      instanceById: new Map([
        ['A', active],
        ['B', other],
      ]),
    };
    const service = new CubeCollisionService({ gap: 8, maxIterations: 1, bucketThreshold: 1000 });
    const result = service.resolveCollisions({ activeInstanceId: 'A', index });

    expect(result.moved).toBe(true);
    expect(active.bounds.x).toBe(-18);
    expect(active.bounds.y).toBe(0);
    expect(active.group.pos).toEqual([-18, 0]);
    expect(active.group.size).toEqual([50, 50]);
  });

  test('does not move when no overlap', () => {
    const active = buildInstance({ id: 'A', bounds: { x: 0, y: 0, w: 50, h: 50 } });
    const other = buildInstance({ id: 'B', bounds: { x: 70, y: 0, w: 50, h: 50 } });
    const index = {
      instances: [active, other],
      instanceById: new Map([
        ['A', active],
        ['B', other],
      ]),
    };
    const service = new CubeCollisionService({ gap: 8, maxIterations: 2, bucketThreshold: 1000 });
    const result = service.resolveCollisions({ activeInstanceId: 'A', index });

    expect(result.moved).toBe(false);
    expect(active.bounds.x).toBe(0);
    expect(active.group.pos).toEqual([0, 0]);
  });
});
