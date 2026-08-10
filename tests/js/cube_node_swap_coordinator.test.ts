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
/** Verify first-class Cube node swapping does not use legacy group markers. */

import { jest } from '@jest/globals';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeNodeSwapCoordinator } from '../../frontend/comfyui/ui/cube/node/CubeNodeSwapCoordinator.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { ComfyGraph } from '../../frontend/comfyui/ui/types/graph.js';

describe('CubeNodeSwapCoordinator', () => {
  test('swaps real Cube node bounds through history and dirty graph state', () => {
    const first = cubeNode('first', [20, 40], { size: [220, 160] });
    const second = cubeNode('second', [300, 80], { size: [360, 240] });
    const graph: ComfyGraph = {
      _nodes: [first, second],
      afterChange: jest.fn(),
      setDirtyCanvas: jest.fn(),
    };
    const catalog = new CubeNodeCatalog();
    catalog.add(first);
    catalog.add(second);
    const changed: string[] = [];
    catalog.subscribe(() => changed.push('changed'));
    const beforeChange = jest.fn();
    const afterChange = jest.fn();
    const setDirtyCanvas = jest.fn();
    const coordinator = new CubeNodeSwapCoordinator({
      graph,
      nodes: catalog,
      history: { beforeChange, afterChange },
      setDirtyCanvas,
    });

    expect(coordinator.canSwap({ instance_id: 'first' }, 'right')).toBe(true);
    coordinator.swap({ instance_id: 'first' }, 'right');

    expect([...first.pos]).toEqual([440, 80]);
    expect([...second.pos]).toEqual([20, 40]);
    expect(beforeChange).toHaveBeenCalledTimes(1);
    expect(afterChange).toHaveBeenCalledTimes(1);
    expect(graph.afterChange).toHaveBeenCalledTimes(1);
    expect(graph.setDirtyCanvas).toHaveBeenCalledWith(true, true);
    expect(setDirtyCanvas).toHaveBeenCalledWith(true, true);
    expect(changed).toHaveLength(2);
  });

  test('uses Cube widths read at click time so resized neighbors remain separated', () => {
    const first = cubeNode('first', [20, 40], { size: [220, 160] });
    const second = cubeNode('second', [300, 40], { size: [220, 160] });
    const catalog = new CubeNodeCatalog();
    catalog.add(first);
    catalog.add(second);
    const coordinator = new CubeNodeSwapCoordinator({
      graph: {},
      nodes: catalog,
      history: {},
    });

    expect(coordinator.canSwap({ instance_id: 'first' }, 'right')).toBe(true);
    first.size[0] = 480;
    second.size[0] = 340;
    coordinator.swap({ instance_id: 'first' }, 'right');

    expect([...first.pos]).toEqual([384, 40]);
    expect([...second.pos]).toEqual([20, 40]);
    expect(second.pos[0] + second.size[0]).toBeLessThanOrEqual(first.pos[0]);
  });

  test('keeps outputless Cubes eligible when they still expose an input', () => {
    const first = cubeNode('first', [20, 40], { outputs: [] });
    const second = cubeNode('second', [300, 40]);
    const catalog = new CubeNodeCatalog();
    catalog.add(first);
    catalog.add(second);
    const coordinator = new CubeNodeSwapCoordinator({
      graph: {},
      nodes: catalog,
      history: {},
    });

    expect(coordinator.canSwap({ instance_id: 'first' }, 'right')).toBe(true);
  });

  test('does not offer arrows on inputless Cubes or allow another Cube to move them', () => {
    const source = cubeNode('source', [20, 40], { inputs: [] });
    const middle = cubeNode('middle', [300, 40]);
    const catalog = new CubeNodeCatalog();
    catalog.add(source);
    catalog.add(middle);
    const coordinator = new CubeNodeSwapCoordinator({
      graph: {},
      nodes: catalog,
      history: {},
    });

    expect(coordinator.canSwap({ instance_id: 'source' }, 'right')).toBe(false);
    expect(coordinator.canSwap({ instance_id: 'middle' }, 'left')).toBe(false);

    coordinator.swap({ instance_id: 'source' }, 'right');
    coordinator.swap({ instance_id: 'middle' }, 'left');

    expect([...source.pos]).toEqual([20, 40]);
    expect([...middle.pos]).toEqual([300, 40]);
  });
});

/** Build a first-class Cube node with graph-owned boundary slots. */
function cubeNode(
  instanceId: string,
  position: [number, number],
  options: { inputs?: unknown[]; outputs?: unknown[]; size?: [number, number] } = {},
): CubeNode {
  return {
    id: instanceId,
    type: `cube-${instanceId}`,
    title: instanceId,
    pos: [...position],
    size: [...(options.size ?? [220, 160])],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: instanceId },
      sugarcubes_surface: {},
    },
    inputs: (options.inputs ?? [{ name: 'input.value' }]) as CubeNode['inputs'],
    outputs: (options.outputs ?? [{ name: 'output.image' }]) as CubeNode['outputs'],
    subgraph: {
      id: `definition-${instanceId}`,
      name: instanceId,
      _nodes: [],
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
  };
}
