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
import { describe, expect, test, jest } from '@jest/globals';
import { CubeLayoutService } from '../../frontend/comfyui/ui/layout/CubeLayoutService.js';
import type { LayoutEntry } from '../../frontend/comfyui/ui/layout/CubeLayoutEngine.js';
import type { LayoutServiceIndex } from '../../frontend/comfyui/ui/layout/CubeLayoutService.js';
import type { RectBounds } from '../../frontend/comfyui/ui/types/common.js';

type TestLayoutEntry = LayoutEntry & { bounds: RectBounds };

function buildIndex(instances: TestLayoutEntry[]): LayoutServiceIndex {
  const instanceById = new Map(instances.map((entry) => [entry.instanceId, entry]));
  return {
    instances,
    instanceById,
    instanceByMarkerId: new Map(),
  };
}

describe('CubeLayoutService', () => {
  test('insertBetween applies moves and refreshes managers', () => {
    const graph = {
      afterChange: jest.fn(),
      setDirtyCanvas: jest.fn(),
    };
    const instances = [
      { instanceId: 'A', bounds: { x: 0, y: 0, w: 10, h: 10 } },
      { instanceId: 'B', bounds: { x: 100, y: 0, w: 10, h: 10 } },
    ];
    const index = buildIndex(instances);
    const mover = { applyMoves: jest.fn() };
    const instanceManager = { scheduleRefresh: jest.fn() };
    const dirtyManager = { requestRefresh: jest.fn() };
    const service = new CubeLayoutService({
      instanceManager,
      dirtyManager,
      mover,
      indexFactory: () => index,
    });

    const result = service.insertBetween({
      graph,
      leftId: 'A',
      rightId: 'B',
      newBounds: { w: 20 },
      gap: 10,
    });

    const moves = result.moves;
    expect(moves.get('B')).toEqual({ dx: 30, dy: 0 });
    expect(mover.applyMoves).toHaveBeenCalledWith(graph, index, moves, {
      recomputeBounds: false,
    });
    expect(graph.afterChange).toHaveBeenCalled();
    expect(graph.setDirtyCanvas).toHaveBeenCalledWith(true, true);
    expect(instanceManager.scheduleRefresh).toHaveBeenCalled();
    expect(dirtyManager.requestRefresh).toHaveBeenCalled();
  });

  test('swapOrder converts placements into move deltas', () => {
    const graph = {
      afterChange: jest.fn(),
      setDirtyCanvas: jest.fn(),
    };
    const instances = [
      { instanceId: 'A', bounds: { x: 0, y: 0, w: 10, h: 10 } },
      { instanceId: 'B', bounds: { x: 20, y: 0, w: 10, h: 10 } },
    ];
    const index = buildIndex(instances);
    const mover = { applyMoves: jest.fn() };
    const instanceManager = { scheduleRefresh: jest.fn() };
    const dirtyManager = { requestRefresh: jest.fn() };
    const service = new CubeLayoutService({
      instanceManager,
      dirtyManager,
      mover,
      indexFactory: () => index,
    });

    const result = service.swapOrder({
      graph,
      aId: 'A',
      bId: 'B',
      layout: { origin: [0, 0], gap: 5 },
    });

    expect(result.moves.get('A')).toEqual({ dx: 15, dy: 0 });
    expect(result.moves.get('B')).toEqual({ dx: -20, dy: 0 });
    expect(mover.applyMoves).toHaveBeenCalledWith(graph, index, result.moves, {
      recomputeBounds: false,
    });
  });

  test('swapOrder uses provided scoped order without re-deriving global order', () => {
    const graph = {
      afterChange: jest.fn(),
      setDirtyCanvas: jest.fn(),
    };
    const instances = [
      { instanceId: 'A', bounds: { x: 0, y: 0, w: 10, h: 10 } },
      { instanceId: 'B', bounds: { x: 20, y: 0, w: 10, h: 10 } },
      { instanceId: 'C', bounds: { x: 40, y: 0, w: 10, h: 10 } },
    ];
    const index = buildIndex(instances);
    const mover = { applyMoves: jest.fn() };
    const instanceManager = { scheduleRefresh: jest.fn() };
    const dirtyManager = { requestRefresh: jest.fn() };
    const service = new CubeLayoutService({
      instanceManager,
      dirtyManager,
      mover,
      indexFactory: () => index,
    });
    const deriveOrderSpy = jest.spyOn(service, 'deriveOrder');
    const scopedOrder = [instances[0], instances[1]];

    const result = service.swapOrder({
      graph,
      aId: 'A',
      bId: 'B',
      order: scopedOrder,
      layout: { origin: [0, 0], gap: 5 },
    });

    expect(deriveOrderSpy).not.toHaveBeenCalled();
    expect(result.order).toEqual(scopedOrder);
    expect(result.moves.get('A')).toEqual({ dx: 15, dy: 0 });
    expect(result.moves.get('B')).toEqual({ dx: -20, dy: 0 });
    expect(result.moves.has('C')).toBe(false);
    expect(mover.applyMoves).toHaveBeenCalledWith(graph, index, result.moves, {
      recomputeBounds: false,
    });
  });

  test('deriveOrder passes anchor strategy to connected component ordering', () => {
    const instanceA = {
      instanceId: 'A',
      bounds: { x: 0, y: 0, w: 10, h: 10 },
      markers: [{ id: 1, type: 'SugarCubes.CubeOutput' }],
    };
    const instanceB = {
      instanceId: 'B',
      bounds: { x: 20, y: 0, w: 10, h: 10 },
      markers: [{ id: 2, type: 'SugarCubes.CubeInput' }],
    };
    const instanceC = {
      instanceId: 'C',
      bounds: { x: 10, y: 0, w: 10, h: 10 },
      markers: [{ id: 3, type: 'SugarCubes.CubeInput' }],
    };
    const graph = {
      links: [{ origin_id: 1, target_id: 2 }],
    };
    const index = {
      instances: [instanceA, instanceC, instanceB],
      instanceById: new Map([
        ['A', instanceA],
        ['B', instanceB],
        ['C', instanceC],
      ]),
      instanceByMarkerId: new Map([
        ['1', instanceA],
        ['2', instanceB],
        ['3', instanceC],
      ]),
      graph,
    };
    const service = new CubeLayoutService();

    const order = service.deriveOrder(index, {
      graph,
      strategy: { anchorInstanceId: 'A' },
    });

    expect(order.map((entry) => entry.instanceId)).toEqual(['A', 'B']);
  });
});
