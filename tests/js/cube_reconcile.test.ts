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
  type CubeBoundsPayload,
  CUBE_INSTANCE_HEADER_HEIGHT,
  CUBE_INSTANCE_PADDING,
  CUBE_INSTANCE_TOP_EXTRA,
} from '../../frontend/comfyui/ui/graph/CubeBounds.js';
import { CubeBoundsReconciler } from '../../frontend/comfyui/ui/layout/CubeBoundsReconciler.js';
import type { CubeGroupMetadataRecord } from '../../frontend/comfyui/ui/graph/GroupMetadata.js';

describe('CubeBoundsReconciler', () => {
  test('treats canonical metadata bounds as source of truth', () => {
    const metadata: CubeGroupMetadataRecord & {
      bounds: { x: number; y: number; w: number; h: number; padding: { x: number } };
    } = {
      managed: true,
      instance_id: 'inst-1',
      cube_id: 'cube-1',
      bounds: { x: 100, y: 200, w: 300, h: 400, padding: { x: 1 } },
      markers: { inputs: [], outputs: [] },
      nodes: [],
    };
    const group = { pos: [5, 6], size: [7, 8], properties: { sugarcubes: metadata } };
    const instance = {
      instanceId: 'inst-1',
      group,
      metadata,
      nodes: [],
      markers: [],
      bounds: { ...metadata.bounds },
    };

    const reconciler = new CubeBoundsReconciler();
    const result = reconciler.reconcileAll({ index: { instances: [instance] } });

    const padX = 1;
    const expected = {
      x: 100,
      y: 200,
      w: 300,
      h: 400,
      padding: {
        x: 1,
        y: CUBE_INSTANCE_PADDING.y,
        top_extra: CUBE_INSTANCE_TOP_EXTRA,
      },
      header: {
        height: CUBE_INSTANCE_HEADER_HEIGHT,
      },
    };

    expect(result.changed.has('inst-1')).toBe(true);
    expect(metadata.bounds).toEqual(expected);
    expect(group.pos).toEqual([expected.x, expected.y]);
    expect(group.size).toEqual([expected.w, expected.h]);
  });

  test('falls back to node-derived bounds when canonical bounds are missing', () => {
    const metadata: CubeGroupMetadataRecord & { bounds?: CubeBoundsPayload } = {
      managed: true,
      instance_id: 'inst-2',
      cube_id: 'cube-2',
      markers: { inputs: [], outputs: [] },
      nodes: ['1'],
    };
    const node = { id: 1, pos: [10, 20], size: [30, 40] };
    const instance = {
      instanceId: 'inst-2',
      group: null,
      metadata,
      nodes: [node],
      markers: [],
      bounds: null,
    };

    const reconciler = new CubeBoundsReconciler();
    const result = reconciler.reconcileAll({ index: { instances: [instance] } });

    const expected = {
      x: 0,
      y: -40,
      w: 50,
      h: 110,
      padding: {
        x: CUBE_INSTANCE_PADDING.x,
        y: CUBE_INSTANCE_PADDING.y,
        top_extra: CUBE_INSTANCE_TOP_EXTRA,
      },
      header: {
        height: CUBE_INSTANCE_HEADER_HEIGHT,
      },
    };

    expect(result.changed.has('inst-2')).toBe(true);
    expect(metadata.bounds).toEqual(expected);
    expect(instance.bounds).toEqual(expected);
  });

  test('adopts group translation when nodes move with the group', () => {
    const metadata = {
      managed: true,
      instance_id: 'inst-3',
      cube_id: 'cube-3',
      bounds: {
        x: 0,
        y: -28,
        w: 60,
        h: 100,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 32 },
      },
      markers: { inputs: [10], outputs: [] },
      nodes: [1],
    };
    const group = {
      pos: [15, -18],
      size: [60, 100],
      properties: { sugarcubes: metadata },
    };
    const node = { id: 1, pos: [25, 42], size: [20, 20] };
    const marker = { id: 10, type: 'SugarCubes.CubeInput', pos: [50, 52], size: [15, 20] };
    const instance = {
      instanceId: 'inst-3',
      group,
      metadata,
      nodes: [node],
      markers: [marker],
      bounds: { ...metadata.bounds },
    };

    const reconciler = new CubeBoundsReconciler();
    const result = reconciler.reconcileAll({ index: { instances: [instance] } });

    const expected = {
      x: 15,
      y: -18,
      w: 60,
      h: 100,
      padding: {
        x: 2,
        y: 2,
        top_extra: 0,
      },
      header: {
        height: 32,
      },
    };

    expect(result.changed.has('inst-3')).toBe(true);
    expect(metadata.bounds).toEqual(expected);
    expect(group.pos).toEqual([15, -18]);
    expect(group.size).toEqual([60, 100]);
    expect(instance.bounds).toEqual(expected);
  });
});
