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
import {
  CUBE_INSTANCE_HEADER_HEIGHT,
  CUBE_INSTANCE_PADDING,
  CUBE_INSTANCE_TOP_EXTRA,
  computeInstanceBounds,
  inflateInstanceBounds,
} from '../../web/comfyui/ui/graph/CubeBounds.js';
import { applyMoves } from '../../web/comfyui/ui/layout/CubeMover.js';
import type { CubeBoundsPayload } from '../../web/comfyui/ui/graph/CubeBounds.js';
import type { RectBounds } from '../../web/comfyui/ui/types/common.js';

function makeTypedVec2(x: number, y: number): Float32Array {
  return new Float32Array([x, y]);
}

function buildMetadataBounds(
  bounds: RectBounds,
  padding: CubeBoundsPayload['padding'],
  header: CubeBoundsPayload['header'],
): CubeBoundsPayload {
  return {
    x: bounds.x,
    y: bounds.y,
    w: bounds.w,
    h: bounds.h,
    padding: { ...padding },
    header: { ...header },
  };
}

describe('CubeMover', () => {
  test('moves nodes, markers, and group positions together', () => {
    const node = { id: 1, pos: makeTypedVec2(10, 20), size: makeTypedVec2(30, 40) };
    const marker = { id: 2, pos: makeTypedVec2(50, 60), size: makeTypedVec2(10, 10) };
    const computed = computeInstanceBounds(
      [{ pos: [10, 20], size: [30, 40] }],
      [{ pos: [50, 60], size: [10, 10] }],
    );
    const inflated = inflateInstanceBounds(computed, CUBE_INSTANCE_PADDING);
    const metadata = {
      bounds: buildMetadataBounds(
        inflated!,
        { ...CUBE_INSTANCE_PADDING, top_extra: CUBE_INSTANCE_TOP_EXTRA },
        { height: CUBE_INSTANCE_HEADER_HEIGHT },
      ),
    };
    const group = {
      pos: makeTypedVec2(0, 0),
      size: makeTypedVec2(100, 120),
      properties: { sugarcubes: metadata },
    };
    const instance = {
      instanceId: 'inst-1',
      nodes: [node],
      markers: [marker],
      group,
      metadata,
      bounds: { ...inflated! },
    };
    const index = { instances: [instance] };
    const moves = new Map([['inst-1', { dx: 10, dy: -5 }]]);

    applyMoves(null, index, moves);

    expect(Array.from(node.pos)).toEqual([20, 15]);
    expect(Array.from(marker.pos)).toEqual([60, 55]);
    const movedComputed = computeInstanceBounds(
      [{ pos: [20, 15], size: [30, 40] }],
      [{ pos: [60, 55], size: [10, 10] }],
    );
    const movedInflated = inflateInstanceBounds(movedComputed, CUBE_INSTANCE_PADDING);
    expect(Array.from(group.pos)).toEqual([movedInflated!.x, movedInflated!.y]);
    expect(Array.from(group.size)).toEqual([movedInflated!.w, movedInflated!.h]);
  });

  test('updates metadata bounds after moving nodes', () => {
    const node = { id: 1, pos: [5, 5], size: [20, 10] };
    const marker = { id: 2, pos: [30, 15], size: [5, 5] };
    const metadata = {
      bounds: {
        x: 0,
        y: 0,
        w: 0,
        h: 0,
        padding: { x: 8, y: 9, top_extra: 7 },
        header: { height: 24 },
      },
    };
    const group = { pos: [0, 0], size: [0, 0], properties: { sugarcubes: metadata } };
    const instance = {
      instanceId: 'inst-2',
      nodes: [node],
      markers: [marker],
      group,
      metadata,
      bounds: { x: 0, y: 0, w: 0, h: 0 },
    };
    const index = { instances: [instance] };
    const moves = new Map([['inst-2', { dx: 5, dy: 5 }]]);

    applyMoves(null, index, moves);

    const computed = computeInstanceBounds([node], [marker]);
    const inflated = inflateInstanceBounds(computed, {
      x: 8,
      y: 9,
      top_extra: 7,
      header: { height: 24 },
    });
    expect(group.properties.sugarcubes.bounds).toEqual({
      x: inflated!.x,
      y: inflated!.y,
      w: inflated!.w,
      h: inflated!.h,
      padding: { x: 8, y: 9, top_extra: 7 },
      header: { height: 24 },
    });
    expect(instance.bounds).toEqual(inflated);
    expect(group.pos).toEqual([inflated!.x, inflated!.y]);
    expect(group.size).toEqual([inflated!.w, inflated!.h]);
  });

  test('preserves bounds shape during rigid layout moves', () => {
    const node = { id: 1, pos: [10, 20], size: [30, 20] };
    const marker = { id: 2, pos: [50, 40], size: [10, 10] };
    const metadata = {
      bounds: {
        x: -5,
        y: -10,
        w: 220,
        h: 180,
        padding: { x: 6, y: 7, top_extra: 3 },
        header: { height: 24 },
      },
    };
    const group = {
      pos: [metadata.bounds.x, metadata.bounds.y],
      size: [metadata.bounds.w, metadata.bounds.h],
      properties: { sugarcubes: metadata },
    };
    const instance = {
      instanceId: 'inst-rigid',
      nodes: [node],
      markers: [marker],
      group,
      metadata: group.properties.sugarcubes,
      bounds: { ...metadata.bounds },
    };
    const index = { instances: [instance] };
    const moves = new Map([['inst-rigid', { dx: 12, dy: -8 }]]);

    applyMoves(null, index, moves, { recomputeBounds: false });

    expect(node.pos).toEqual([22, 12]);
    expect(marker.pos).toEqual([62, 32]);
    expect(group.pos).toEqual([7, -18]);
    expect(group.size).toEqual([220, 180]);
    expect(group.properties.sugarcubes.bounds).toEqual({
      x: 7,
      y: -18,
      w: 220,
      h: 180,
      padding: { x: 6, y: 7, top_extra: 3 },
      header: { height: 24 },
    });
  });
});
