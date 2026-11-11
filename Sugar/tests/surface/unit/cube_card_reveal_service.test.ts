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
import { CubeFaceCardStateService } from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js';
import { parseCubeSurfaceState } from '../../../frontend/comfyui/ui/surface/CubeSurfaceState.js';
import { readCubeFaceCardState } from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateService.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';

test.each(['cube', 'workflow'])(
  'retains independent reveal and activation choices on a %s',
  (kind) => {
    const cube = cubeNode();
    const nodes = new CubeNodeCatalog();
    nodes.add(cube);
    const graph: ComfyGraph = { _nodes: cube.subgraph._nodes };
    const target = kind === 'cube' ? cube : graph;
    const node = cube.subgraph._nodes[0]!;
    const service = new CubeFaceCardStateService({ nodes, history: {} });
    const listener = jest.fn();
    const unsubscribe = service.subscribe(listener);
    service.setRevealed(target, 'patch', true);
    expect(node.mode).toBe(4);
    service.setEnabled(target, 'patch', true);
    expect(node.mode).toBe(0);
    service.setRevealed(target, 'patch', false);
    expect(node.mode).toBe(4);
    service.setRevealed(target, 'patch', true);
    expect(node.mode).toBe(0);
    service.setEnabled(target, 'patch', false);
    expect(service.list(target)[0]?.revealed).toBe(true);
    expect(readCubeFaceCardState(target).cards.patch).toMatchObject({
      authoredBypass: true,
      revealed: true,
      enabledOverride: false,
    });

    if (kind === 'workflow') {
      expect(
        parseCubeSurfaceState(
          graph.extra && typeof graph.extra === 'object'
            ? Reflect.get(graph.extra, 'sugarcubes_surface')
            : undefined,
        ).cards.patch,
      ).toEqual(readCubeFaceCardState(target).cards.patch);
      const savedExtra: unknown = JSON.parse(JSON.stringify(graph.extra));
      expect(
        parseCubeSurfaceState(
          typeof savedExtra === 'object' && savedExtra !== null
            ? Reflect.get(savedExtra, 'sugarcubes_surface')
            : undefined,
        ).cards.patch,
      ).toEqual(readCubeFaceCardState(target).cards.patch);
    }
    expect(listener).toHaveBeenCalledTimes(5);
    unsubscribe();
    service.setEnabled(target, 'patch', true);
    expect(listener).toHaveBeenCalledTimes(5);
  },
);

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
  const service = new CubeFaceCardStateService({ nodes, history });

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
  const service = new CubeFaceCardStateService({ nodes, history });

  service.setRevealed(node, 'removed-node', true);

  expect(history.beforeChange).not.toHaveBeenCalled();
  expect(history.afterChange).not.toHaveBeenCalled();
});

test('copies linked execution choices through persisted card state while preserving follower reveal', () => {
  const cube = cubeNode();
  const nodes = new CubeNodeCatalog();
  nodes.add(cube);
  const service = new CubeFaceCardStateService({ nodes, history: {} });
  service.setExecutionMode(cube, 'patch', 2);
  expect(cube.subgraph._nodes[0]?.mode).toBe(4);
  expect(readCubeFaceCardState(cube).cards.patch).toMatchObject({
    revealed: false,
    enabledOverride: true,
    activeMode: 2,
  });
  service.setRevealed(cube, 'patch', true);
  expect(cube.subgraph._nodes[0]?.mode).toBe(2);
  service.setExecutionMode(cube, 'patch', 4);
  expect(readCubeFaceCardState(cube).cards.patch).toMatchObject({
    revealed: true,
    enabledOverride: false,
    activeMode: 2,
  });
  service.setExecutionMode(cube, 'patch', 0);
  expect(cube.subgraph._nodes[0]?.mode).toBe(0);
  expect(readCubeFaceCardState(cube).cards.patch?.revealed).toBe(true);
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
