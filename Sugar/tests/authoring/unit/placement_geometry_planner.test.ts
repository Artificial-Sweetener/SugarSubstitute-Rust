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
import { planPlacementGeometry } from '../../../frontend/comfyui/ui/geometry/PlacementGeometryPlanner.js';
import { resolveRendererGeometryPolicy } from '../../../frontend/comfyui/ui/geometry/RendererGeometryPolicy.js';
import type { ImportPayload } from '../../../frontend/comfyui/ui/import/PlacementPayload.js';

function payloadWithNodes(secondX = 160): ImportPayload {
  return {
    nodes: [
      { symbol: 'first', layout: { id: 1, pos: [0, 0], size: [140, 80] } },
      { symbol: 'second', layout: { id: 2, pos: [secondX, 0], size: [140, 80] } },
    ],
    markers: [],
    layout: {
      origin: [0, 0],
      groups: [
        {
          bounding: [-10, -40, secondX + 160, 130],
          sugarcubes: { managed: true, nodes: ['first', 'second'] },
        },
      ],
    },
  };
}

describe('placement geometry planner', () => {
  test('LiteGraph placement preserves canonical geometry and source data', () => {
    const source = payloadWithNodes();
    const planned = planPlacementGeometry(source, resolveRendererGeometryPolicy({}));

    expect(planned.nodes?.map((entry) => entry.layout)).toEqual(
      source.nodes?.map((entry) => entry.layout),
    );
    expect(planned).not.toBe(source);
    expect(planned.nodes?.[0]).not.toBe(source.nodes?.[0]);
  });

  test('Nodes 2 expands narrow nodes and preserves the authored gap', () => {
    const planned = planPlacementGeometry(
      payloadWithNodes(),
      resolveRendererGeometryPolicy({ vueNodesMode: true }),
    );

    expect(planned.nodes?.[0]?.layout?.size).toEqual([225, 80]);
    expect(planned.nodes?.[1]?.layout?.pos).toEqual([245, 30]);
    expect(planned.nodes?.[1]?.layout?.size).toEqual([225, 80]);
  });

  test('intentional authored overlaps are retained', () => {
    const planned = planPlacementGeometry(
      payloadWithNodes(100),
      resolveRendererGeometryPolicy({ vueNodesMode: true }),
    );

    expect(planned.nodes?.[1]?.layout?.pos).toEqual([100, 30]);
  });

  test('managed group chrome grows around renderer-safe nodes', () => {
    const planned = planPlacementGeometry(
      payloadWithNodes(),
      resolveRendererGeometryPolicy({ vueNodesMode: true }),
    );

    expect(planned.layout?.groups?.[0]?.bounding).toEqual([-10, -40, 490, 160]);
    expect(planned.layout?.groups?.[0]?.sugarcubes?.bounds).toEqual({
      x: -10,
      y: -40,
      w: 490,
      h: 160,
    });
  });

  test('does not replace unrelated authored groups with managed cube chrome', () => {
    const source = payloadWithNodes();
    source.layout?.groups?.push({ title: 'Inner note', bounding: [20, 10, 80, 60] });

    const planned = planPlacementGeometry(
      source,
      resolveRendererGeometryPolicy({ vueNodesMode: true }),
    );

    expect(planned.layout?.groups?.[1]?.bounding).toEqual([20, 10, 80, 60]);
  });

  test('planning is idempotent after geometry has stabilized', () => {
    const policy = resolveRendererGeometryPolicy({ vueNodesMode: true });
    const first = planPlacementGeometry(payloadWithNodes(), policy);
    const second = planPlacementGeometry(first, policy);

    expect(second).toEqual(first);
  });

  test('collapsed nodes are not forced to the expanded minimum width', () => {
    const source = payloadWithNodes();
    const first = source.nodes?.[0];
    if (first?.layout) first.layout.flags = { collapsed: true };

    const planned = planPlacementGeometry(
      source,
      resolveRendererGeometryPolicy({ vueNodesMode: true }),
    );

    expect(planned.nodes?.[0]?.layout?.size).toEqual([140, 80]);
  });
});
