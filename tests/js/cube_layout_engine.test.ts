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
  appendAfter,
  deriveChainOrder,
  insertBefore,
  insertBetween,
  layoutFromOrder,
  replaceCube,
  swapOrder,
} from '../../frontend/comfyui/ui/layout/CubeLayoutEngine.js';
import type {
  LayoutEntry,
  LayoutIndex,
  LayoutPlacement,
} from '../../frontend/comfyui/ui/layout/CubeLayoutEngine.js';
import type { RectBounds } from '../../frontend/comfyui/ui/types/common.js';
import type { ComfyGraph, ComfyNode, GraphId } from '../../frontend/comfyui/ui/types/graph.js';

interface TestLayoutEntry extends LayoutEntry {
  bounds: RectBounds;
  markers: ComfyNode[];
}

function makeMarker(id: GraphId, type: string): ComfyNode {
  return { id, type };
}

function makeInstance({
  instanceId,
  bounds,
  markers = [],
}: {
  instanceId: string;
  bounds: RectBounds;
  markers?: ComfyNode[];
}): TestLayoutEntry {
  return {
    instanceId,
    bounds,
    markers,
  };
}

function buildIndex({
  instances,
  markerMap,
  graph,
}: {
  instances: TestLayoutEntry[];
  markerMap: ReadonlyMap<string, TestLayoutEntry>;
  graph: ComfyGraph;
}): LayoutIndex {
  return {
    instances,
    instanceByMarkerId: markerMap,
    graph,
  };
}

describe('CubeLayoutEngine', () => {
  test('deriveChainOrder uses marker links for linear chains', () => {
    const aOut = makeMarker(1, 'SugarCubes.CubeOutput');
    const bIn = makeMarker(2, 'SugarCubes.CubeInput');
    const bOut = makeMarker(3, 'SugarCubes.CubeOutput');
    const cIn = makeMarker(4, 'SugarCubes.CubeInput');

    const instanceA = makeInstance({
      instanceId: 'A',
      bounds: { x: 0, y: 0, w: 10, h: 10 },
      markers: [aOut],
    });
    const instanceB = makeInstance({
      instanceId: 'B',
      bounds: { x: 20, y: 0, w: 10, h: 10 },
      markers: [bIn, bOut],
    });
    const instanceC = makeInstance({
      instanceId: 'C',
      bounds: { x: 40, y: 0, w: 10, h: 10 },
      markers: [cIn],
    });

    const markerMap = new Map([
      ['1', instanceA],
      ['2', instanceB],
      ['3', instanceB],
      ['4', instanceC],
    ]);
    const graph = {
      links: [
        { origin_id: 1, target_id: 2 },
        { origin_id: 3, target_id: 4 },
      ],
    };
    const index = buildIndex({
      instances: [instanceA, instanceB, instanceC],
      markerMap,
      graph,
    });

    const order = deriveChainOrder(index);
    expect(order.map((entry) => entry.instanceId)).toEqual(['A', 'B', 'C']);
  });

  test('deriveChainOrder falls back to spatial order when graph is not linear', () => {
    const aOut = makeMarker(10, 'SugarCubes.CubeOutput');
    const bIn = makeMarker(11, 'SugarCubes.CubeInput');
    const cIn = makeMarker(12, 'SugarCubes.CubeInput');
    const instanceA = makeInstance({
      instanceId: 'A',
      bounds: { x: 50, y: 0, w: 10, h: 10 },
      markers: [aOut],
    });
    const instanceB = makeInstance({
      instanceId: 'B',
      bounds: { x: 0, y: 0, w: 10, h: 10 },
      markers: [bIn],
    });
    const instanceC = makeInstance({
      instanceId: 'C',
      bounds: { x: 100, y: 0, w: 10, h: 10 },
      markers: [cIn],
    });
    const markerMap = new Map([
      ['10', instanceA],
      ['11', instanceB],
      ['12', instanceC],
    ]);
    const graph = {
      links: [
        { origin_id: 10, target_id: 11 },
        { origin_id: 10, target_id: 12 },
      ],
    };
    const index = buildIndex({
      instances: [instanceA, instanceB, instanceC],
      markerMap,
      graph,
    });
    const order = deriveChainOrder(index);
    expect(order.map((entry) => entry.instanceId)).toEqual(['B', 'A', 'C']);
  });

  test('deriveChainOrder scopes to connected component for anchored instance', () => {
    const aOut = makeMarker(21, 'SugarCubes.CubeOutput');
    const bIn = makeMarker(22, 'SugarCubes.CubeInput');
    const cOut = makeMarker(23, 'SugarCubes.CubeOutput');
    const cIn = makeMarker(24, 'SugarCubes.CubeInput');
    const instanceA = makeInstance({
      instanceId: 'A',
      bounds: { x: 0, y: 0, w: 10, h: 10 },
      markers: [aOut],
    });
    const instanceB = makeInstance({
      instanceId: 'B',
      bounds: { x: 20, y: 0, w: 10, h: 10 },
      markers: [bIn],
    });
    const instanceC = makeInstance({
      instanceId: 'C',
      bounds: { x: 10, y: 0, w: 10, h: 10 },
      markers: [cOut, cIn],
    });
    const markerMap = new Map([
      ['21', instanceA],
      ['22', instanceB],
      ['23', instanceC],
      ['24', instanceC],
    ]);
    const graph = {
      links: [{ origin_id: 21, target_id: 22 }],
    };
    const index = buildIndex({
      instances: [instanceA, instanceC, instanceB],
      markerMap,
      graph,
    });

    const anchoredOrder = deriveChainOrder(index, { anchorInstanceId: 'A' });

    expect(anchoredOrder.map((entry) => entry.instanceId)).toEqual(['A', 'B']);
  });

  test('deriveChainOrder returns solo anchored instance when it is disconnected', () => {
    const aOut = makeMarker(31, 'SugarCubes.CubeOutput');
    const bIn = makeMarker(32, 'SugarCubes.CubeInput');
    const cIn = makeMarker(33, 'SugarCubes.CubeInput');
    const instanceA = makeInstance({
      instanceId: 'A',
      bounds: { x: 0, y: 0, w: 10, h: 10 },
      markers: [aOut],
    });
    const instanceB = makeInstance({
      instanceId: 'B',
      bounds: { x: 20, y: 0, w: 10, h: 10 },
      markers: [bIn],
    });
    const instanceC = makeInstance({
      instanceId: 'C',
      bounds: { x: 40, y: 0, w: 10, h: 10 },
      markers: [cIn],
    });
    const markerMap = new Map([
      ['31', instanceA],
      ['32', instanceB],
      ['33', instanceC],
    ]);
    const graph = {
      links: [{ origin_id: 31, target_id: 32 }],
    };
    const index = buildIndex({
      instances: [instanceA, instanceB, instanceC],
      markerMap,
      graph,
    });

    const anchoredOrder = deriveChainOrder(index, { anchorInstanceId: 'C' });

    expect(anchoredOrder.map((entry) => entry.instanceId)).toEqual(['C']);
  });

  test('deriveChainOrder includes proximity-linked neighbors for anchored instance', () => {
    const aOut = makeMarker(41, 'SugarCubes.CubeOutput');
    const bIn = makeMarker(42, 'SugarCubes.CubeInput');
    const instanceA = makeInstance({
      instanceId: 'A',
      bounds: { x: 0, y: 0, w: 10, h: 10 },
      markers: [aOut],
    });
    const instanceB = makeInstance({
      instanceId: 'B',
      bounds: { x: 20, y: 0, w: 10, h: 10 },
      markers: [bIn],
    });
    const markerMap = new Map([
      ['41', instanceA],
      ['42', instanceB],
    ]);
    const graph = { links: [] };
    const index = buildIndex({
      instances: [instanceA, instanceB],
      markerMap,
      graph,
    });

    const anchoredOrder = deriveChainOrder(index, {
      anchorInstanceId: 'A',
      proximityMatches: [{ outputId: 41, inputId: 42 }],
    });

    expect(anchoredOrder.map((entry) => entry.instanceId)).toEqual(['A', 'B']);
  });

  test('insertBetween displaces instances to the right of the insertion point', () => {
    const order = [
      makeInstance({ instanceId: 'A', bounds: { x: 0, y: 0, w: 10, h: 10 } }),
      makeInstance({ instanceId: 'B', bounds: { x: 100, y: 0, w: 10, h: 10 } }),
      makeInstance({ instanceId: 'C', bounds: { x: 200, y: 0, w: 10, h: 10 } }),
    ];
    const moves = insertBetween(order, 'A', 'B', { w: 40 }, 10);
    expect(moves.get('A')).toBeUndefined();
    expect(moves.get('B')).toEqual({ dx: 50, dy: 0 });
    expect(moves.get('C')).toEqual({ dx: 50, dy: 0 });
  });

  test('insertBefore mirrors insertBetween for the target', () => {
    const order = [
      makeInstance({ instanceId: 'A', bounds: { x: 0, y: 0, w: 10, h: 10 } }),
      makeInstance({ instanceId: 'B', bounds: { x: 100, y: 0, w: 10, h: 10 } }),
    ];
    const moves = insertBefore(order, 'B', { w: 30 }, 5);
    expect(moves.get('B')).toEqual({ dx: 35, dy: 0 });
  });

  test('swapOrder reflows using layoutFromOrder', () => {
    const order = [
      makeInstance({ instanceId: 'A', bounds: { x: 0, y: 0, w: 10, h: 10 } }),
      makeInstance({ instanceId: 'B', bounds: { x: 0, y: 0, w: 10, h: 10 } }),
      makeInstance({ instanceId: 'C', bounds: { x: 0, y: 0, w: 10, h: 10 } }),
    ];
    const placements = swapOrder(order, 'A', 'C', { origin: [0, 0], gap: 5 });
    expect(placements).toEqual([
      { instanceId: 'C', x: 0, y: 0 },
      { instanceId: 'B', x: 15, y: 0 },
      { instanceId: 'A', x: 30, y: 0 },
    ]);
  });

  test('swapOrder preserves per-gap spacing when provided', () => {
    const order = [
      makeInstance({ instanceId: 'A', bounds: { x: 0, y: 0, w: 10, h: 10 } }),
      makeInstance({ instanceId: 'B', bounds: { x: 20, y: 0, w: 20, h: 10 } }),
      makeInstance({ instanceId: 'C', bounds: { x: 60, y: 0, w: 10, h: 10 } }),
    ];
    const placements = swapOrder(order, 'A', 'B', {
      origin: [0, 0],
      gaps: [5, 15],
      minGap: 0,
    });
    expect(placements).toEqual([
      { instanceId: 'B', x: 0, y: 0 },
      { instanceId: 'A', x: 25, y: 0 },
      { instanceId: 'C', x: 50, y: 0 },
    ]);
  });

  test('swapOrder keeps span stable across repeated swaps', () => {
    const order = [
      makeInstance({ instanceId: 'A', bounds: { x: 0, y: 0, w: 10, h: 10 } }),
      makeInstance({ instanceId: 'B', bounds: { x: 25, y: 0, w: 20, h: 10 } }),
      makeInstance({ instanceId: 'C', bounds: { x: 60, y: 0, w: 15, h: 10 } }),
    ];
    const layout = { origin: [0, 0], gaps: [5, 15], minGap: 0 };
    const spanFromOrder = (entries: TestLayoutEntry[]) => {
      const xs = entries.map((entry) => entry.bounds.x);
      const minX = Math.min(...xs);
      const maxX = Math.max(...entries.map((entry) => entry.bounds.x + entry.bounds.w));
      return { minX, maxX, span: maxX - minX };
    };
    const applyPlacements = (entries: TestLayoutEntry[], placements: LayoutPlacement[]) => {
      const next = entries.map((entry) => ({ ...entry, bounds: { ...entry.bounds } }));
      const byId = new Map(next.map((entry) => [entry.instanceId, entry]));
      for (const placement of placements) {
        const target = byId.get(placement.instanceId);
        if (target) {
          target.bounds.x = placement.x;
          target.bounds.y = placement.y;
        }
      }
      return next;
    };

    let current = order;
    const expectedSpan =
      order.reduce((sum, entry) => sum + entry.bounds.w, 0) +
      layout.gaps.reduce((sum, gap) => sum + gap, 0);
    for (let i = 0; i < 10; i += 1) {
      const placements = swapOrder(current, 'A', 'B', layout);
      current = applyPlacements(current, placements);
      const { span } = spanFromOrder(current);
      expect(span).toBe(expectedSpan);

      const placementsBack = swapOrder(current, 'A', 'B', layout);
      current = applyPlacements(current, placementsBack);
      const { span: roundTripSpan } = spanFromOrder(current);
      expect(roundTripSpan).toBe(expectedSpan);
    }
  });

  test('replaceCube shifts instances based on width delta direction', () => {
    const order = [
      makeInstance({ instanceId: 'A', bounds: { x: 0, y: 0, w: 10, h: 10 } }),
      makeInstance({ instanceId: 'B', bounds: { x: 50, y: 0, w: 10, h: 10 } }),
      makeInstance({ instanceId: 'C', bounds: { x: 100, y: 0, w: 10, h: 10 } }),
    ];
    const growMoves = replaceCube(order, 'B', { w: 20 }, 0);
    expect(growMoves.get('C')).toEqual({ dx: 10, dy: 0 });
    expect(growMoves.get('A')).toBeUndefined();

    const shrinkMoves = replaceCube(order, 'B', { w: 5 }, 0);
    expect(shrinkMoves.get('A')).toEqual({ dx: -5, dy: 0 });
    expect(shrinkMoves.get('C')).toBeUndefined();
  });

  test('appendAfter places a cube after the last bounds', () => {
    const order = [makeInstance({ instanceId: 'A', bounds: { x: 10, y: 5, w: 20, h: 10 } })];
    const placement = appendAfter(order, 'A', { w: 10 }, 6);
    expect(placement).toEqual({ x: 36, y: 5 });
  });

  test('layoutFromOrder positions in a left-to-right flow', () => {
    const order = [
      makeInstance({ instanceId: 'A', bounds: { x: 0, y: 0, w: 10, h: 10 } }),
      makeInstance({ instanceId: 'B', bounds: { x: 0, y: 0, w: 20, h: 10 } }),
    ];
    const placements = layoutFromOrder(order, [5, 7], 3);
    expect(placements).toEqual([
      { instanceId: 'A', x: 5, y: 7 },
      { instanceId: 'B', x: 18, y: 7 },
    ]);
  });
});
