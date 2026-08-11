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
/** Verify renderer-neutral Cube card visibility persistence and invalidation. */

import { jest } from '@jest/globals';
import {
  requireCubeSurface,
  type CubeNode,
} from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeCardRevealService } from '../../../frontend/comfyui/ui/surface/CubeCardRevealService.js';
import { parseCubeSurfaceState } from '../../../frontend/comfyui/ui/surface/CubeSurfaceState.js';

test('persists visibility through graph history and invalidates both renderers', () => {
  const node = cubeNode();
  const nodes = new CubeNodeCatalog();
  nodes.add(node);
  const changed = jest.fn();
  nodes.subscribe(changed);
  const history = {
    beforeChange: jest.fn(),
    afterChange: jest.fn(),
    setDirtyCanvas: jest.fn(),
  };
  const service = new CubeCardRevealService({ nodes, history });

  expect(service.list(node)).toEqual([{ id: 'patch', label: 'Model patch', revealed: false }]);

  service.setRevealed(node, 'patch', true);

  expect(parseCubeSurfaceState(requireCubeSurface(node)).cards.patch).toEqual({
    authoredBypass: true,
    revealed: true,
    enabledOverride: false,
    activeMode: 0,
  });
  expect(node.subgraph._nodes[0]?.mode).toBe(4);
  expect(history.beforeChange).toHaveBeenCalledTimes(1);
  expect(history.afterChange).toHaveBeenCalledTimes(1);
  expect(history.setDirtyCanvas).toHaveBeenCalledWith(true, true);
  expect(changed).toHaveBeenCalledTimes(1);
});

test('ignores stale menu entries without creating empty history changes', () => {
  const node = cubeNode();
  const nodes = new CubeNodeCatalog();
  nodes.add(node);
  const history = { beforeChange: jest.fn(), afterChange: jest.fn() };
  const service = new CubeCardRevealService({ nodes, history });

  service.setRevealed(node, 'removed-node', true);

  expect(history.beforeChange).not.toHaveBeenCalled();
  expect(history.afterChange).not.toHaveBeenCalled();
});

/** Build one persisted Cube with a bypass-authored optional card. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-node',
    type: 'cube-definition',
    pos: [100, 140],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-instance' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'cube-definition',
      name: 'Optional cards',
      _nodes: [
        {
          id: 'patch',
          type: 'Model patch',
          mode: 4,
          pos: [0, 0],
          size: [240, 100],
          widgets: [],
          inputs: [{ type: 'MODEL' }],
          outputs: [{ type: 'MODEL' }],
          properties: {},
          connect() {},
        },
      ],
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add() {},
      remove() {},
      addInput() {
        throw new Error('not used');
      },
      addOutput() {
        throw new Error('not used');
      },
      configure() {},
    },
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
    setSize(size) {
      this.size[0] = size[0] ?? 0;
      this.size[1] = size[1] ?? 0;
    },
  };
}
